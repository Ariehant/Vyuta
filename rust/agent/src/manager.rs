//! Companion-agent operations: ROS 2 introspection, `colcon build`, and rsync
//! deploy. Real commands are shelled out when the tools exist; otherwise a
//! synthetic equivalent runs so the panel works on a dev box without ROS.
//!
//! A single `stop` one-shot tears down whichever task is active (real child or
//! synthetic), mirroring the `sim-manager` lifecycle.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::Command as ProcCommand;
use tokio::sync::{broadcast, oneshot, Mutex as AsyncMutex};
use tokio::task::JoinHandle;

use crate::graph::{self, GraphFrame};
use crate::protocol::{AckFrame, EchoFrame, LogFrame, Outbound};
use crate::state::{CompanionState, Phase};

const LOG_CAP: usize = 512;

#[derive(Debug, Clone)]
pub struct Config {
    pub ros2_bin: String,
    pub colcon_bin: String,
    pub rsync_bin: String,
    pub workspace: String,
    pub deploy_target: String,
}

struct Inner {
    stop_tx: Option<oneshot::Sender<()>>,
    run_task: Option<JoinHandle<()>>,
}

pub struct Agent {
    cfg: Config,
    ros_available: bool,
    colcon_available: bool,
    rsync_available: bool,
    state: Arc<Mutex<CompanionState>>,
    logs: broadcast::Sender<String>,
    inner: AsyncMutex<Inner>,
}

impl Agent {
    pub fn new(cfg: Config) -> Arc<Self> {
        let ros_available = which(&cfg.ros2_bin);
        let colcon_available = which(&cfg.colcon_bin);
        let rsync_available = which(&cfg.rsync_bin);
        let state = Arc::new(Mutex::new(CompanionState::new(
            ros_available,
            colcon_available,
            cfg.workspace.clone(),
            cfg.deploy_target.clone(),
        )));
        let (logs, _) = broadcast::channel(LOG_CAP);
        Arc::new(Self {
            cfg,
            ros_available,
            colcon_available,
            rsync_available,
            state,
            logs,
            inner: AsyncMutex::new(Inner {
                stop_tx: None,
                run_task: None,
            }),
        })
    }

    pub fn state(&self) -> Arc<Mutex<CompanionState>> {
        self.state.clone()
    }

    pub fn subscribe_logs(&self) -> broadcast::Receiver<String> {
        self.logs.subscribe()
    }

    // --- introspection ------------------------------------------------------

    pub async fn graph(&self) -> GraphFrame {
        if !self.ros_available {
            return graph::synthetic_graph();
        }
        let nodes = run_capture(&self.cfg.ros2_bin, &["node", "list"], 4000)
            .await
            .map(|s| graph::parse_node_list(&s))
            .unwrap_or_default();
        let topics = run_capture(&self.cfg.ros2_bin, &["topic", "list", "-t"], 4000)
            .await
            .map(|s| graph::topics_from_types(graph::parse_named_with_types(&s)))
            .unwrap_or_default();
        let services = run_capture(&self.cfg.ros2_bin, &["service", "list", "-t"], 4000)
            .await
            .map(|s| graph::services_from_types(graph::parse_named_with_types(&s)))
            .unwrap_or_default();
        GraphFrame {
            ros_available: true,
            synthetic: false,
            nodes,
            topics,
            services,
        }
    }

    pub async fn echo(&self, topic: &str) -> EchoFrame {
        let sample = if self.ros_available {
            run_capture(
                &self.cfg.ros2_bin,
                &["topic", "echo", "--once", topic],
                6000,
            )
            .await
            .unwrap_or_else(|| format!("(no message within timeout on {topic})"))
        } else {
            graph::synthetic_echo(topic)
        };
        EchoFrame {
            topic: topic.to_string(),
            sample,
        }
    }

    // --- build / deploy -----------------------------------------------------

    pub async fn build(
        &self,
        workspace: Option<String>,
        packages: Option<Vec<String>>,
    ) -> AckFrame {
        let mut inner = self.inner.lock().await;
        if self.busy() {
            return ack("build", false, "a task is already running");
        }
        inner.stop_tx.take();
        inner.run_task.take();

        let ws = workspace.unwrap_or_else(|| self.cfg.workspace.clone());
        self.set_phase(Phase::Building, "colcon build");
        let (stop_tx, stop_rx) = oneshot::channel();

        let real = self.colcon_available && Path::new(&ws).is_dir();
        if real {
            let mut cmd = ProcCommand::new(&self.cfg.colcon_bin);
            cmd.arg("build").current_dir(&ws);
            if let Some(pkgs) = &packages {
                if !pkgs.is_empty() {
                    cmd.arg("--packages-select").args(pkgs);
                }
            }
            self.log("agent", format!("▶ colcon build in {ws}"));
            match self.spawn_process(cmd, stop_rx, "build complete") {
                Ok(task) => {
                    inner.stop_tx = Some(stop_tx);
                    inner.run_task = Some(task);
                    ack("build", true, "colcon build started")
                }
                Err(e) => {
                    self.fail(&format!("failed to start colcon: {e}"));
                    ack("build", false, &format!("spawn failed: {e}"))
                }
            }
        } else {
            self.log(
                "agent",
                format!("▶ simulated colcon build (colcon unavailable) in {ws}"),
            );
            let lines = vec![
                "Starting >>> px4_msgs".to_string(),
                "Finished <<< px4_msgs [3.2s]".to_string(),
                "Starting >>> drone_mission".to_string(),
                "Finished <<< drone_mission [1.8s]".to_string(),
                "Summary: 2 packages finished [5.1s]".to_string(),
            ];
            let task = self.spawn_synthetic(lines, stop_rx, "build complete (simulated)");
            inner.stop_tx = Some(stop_tx);
            inner.run_task = Some(task);
            ack("build", true, "simulated build started")
        }
    }

    pub async fn deploy(&self, source: Option<String>, target: Option<String>) -> AckFrame {
        let mut inner = self.inner.lock().await;
        if self.busy() {
            return ack("deploy", false, "a task is already running");
        }
        inner.stop_tx.take();
        inner.run_task.take();

        let src = source.unwrap_or_else(|| self.cfg.workspace.clone());
        let tgt = target.unwrap_or_else(|| self.cfg.deploy_target.clone());
        self.set_phase(Phase::Deploying, "rsync deploy");
        let (stop_tx, stop_rx) = oneshot::channel();

        let real = self.rsync_available && !tgt.is_empty() && Path::new(&src).exists();
        if real {
            let src_slash = if src.ends_with('/') {
                src.clone()
            } else {
                format!("{src}/")
            };
            let mut cmd = ProcCommand::new(&self.cfg.rsync_bin);
            cmd.args(["-az", "--delete", "--info=progress2", &src_slash, &tgt]);
            self.log("agent", format!("▶ rsync {src_slash} → {tgt}"));
            match self.spawn_process(cmd, stop_rx, "deploy complete") {
                Ok(task) => {
                    inner.stop_tx = Some(stop_tx);
                    inner.run_task = Some(task);
                    ack("deploy", true, "deploy started")
                }
                Err(e) => {
                    self.fail(&format!("failed to start rsync: {e}"));
                    ack("deploy", false, &format!("spawn failed: {e}"))
                }
            }
        } else {
            self.log(
                "agent",
                format!(
                    "▶ simulated deploy (rsync/target unavailable) {src} → {}",
                    if tgt.is_empty() { "<unset>" } else { &tgt }
                ),
            );
            let lines = vec![
                "sending incremental file list".to_string(),
                "install/ 1,204 files".to_string(),
                "src/ 86 files".to_string(),
                "sent 4.2M bytes  received 1.1K bytes".to_string(),
                "total size is 41.8M  speedup is 9.95".to_string(),
            ];
            let task = self.spawn_synthetic(lines, stop_rx, "deploy complete (simulated)");
            inner.stop_tx = Some(stop_tx);
            inner.run_task = Some(task);
            ack("deploy", true, "simulated deploy started")
        }
    }

    pub async fn cancel(&self) -> AckFrame {
        let mut inner = self.inner.lock().await;
        if !self.busy() {
            return ack("cancel", true, "nothing running");
        }
        if let Some(tx) = inner.stop_tx.take() {
            let _ = tx.send(());
        }
        if let Some(task) = inner.run_task.take() {
            let _ = task.await;
        }
        self.set_phase(Phase::Idle, "cancelled");
        self.log("agent", "■ task cancelled".into());
        ack("cancel", true, "cancelled")
    }

    fn busy(&self) -> bool {
        matches!(
            self.state.lock().expect("state poisoned").phase,
            Phase::Building | Phase::Deploying
        )
    }

    fn spawn_process(
        &self,
        mut cmd: ProcCommand,
        stop_rx: oneshot::Receiver<()>,
        done_msg: &'static str,
    ) -> std::io::Result<JoinHandle<()>> {
        cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = cmd.spawn()?;
        if let Some(o) = child.stdout.take() {
            self.spawn_reader(o, "stdout");
        }
        if let Some(e) = child.stderr.take() {
            self.spawn_reader(e, "stderr");
        }
        let state = self.state.clone();
        let logs = self.logs.clone();
        Ok(tokio::spawn(async move {
            let mut stop_rx = stop_rx;
            tokio::select! {
                status = child.wait() => {
                    let (phase, msg) = match status {
                        Ok(s) if s.success() => (Phase::Idle, done_msg.to_string()),
                        Ok(s) => (Phase::Error, format!("exited with {s}")),
                        Err(e) => (Phase::Error, format!("wait error: {e}")),
                    };
                    set_phase(&state, phase, &msg);
                    push_log(&state, &logs, "agent", format!("■ {msg}"));
                }
                _ = &mut stop_rx => {
                    let _ = child.start_kill();
                    let _ = child.wait().await;
                    set_phase(&state, Phase::Idle, "cancelled");
                }
            }
        }))
    }

    fn spawn_synthetic(
        &self,
        lines: Vec<String>,
        stop_rx: oneshot::Receiver<()>,
        done_msg: &'static str,
    ) -> JoinHandle<()> {
        let state = self.state.clone();
        let logs = self.logs.clone();
        tokio::spawn(async move {
            let mut stop_rx = stop_rx;
            for line in lines {
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_millis(400)) => {
                        push_log(&state, &logs, "stdout", line);
                    }
                    _ = &mut stop_rx => {
                        set_phase(&state, Phase::Idle, "cancelled");
                        return;
                    }
                }
            }
            set_phase(&state, Phase::Idle, done_msg);
            push_log(&state, &logs, "agent", format!("■ {done_msg}"));
        })
    }

    fn spawn_reader<R>(&self, reader: R, stream: &'static str)
    where
        R: AsyncRead + Unpin + Send + 'static,
    {
        let state = self.state.clone();
        let logs = self.logs.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(reader).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                push_log(&state, &logs, stream, line);
            }
        });
    }

    fn set_phase(&self, phase: Phase, message: &str) {
        set_phase(&self.state, phase, message);
    }

    fn fail(&self, msg: &str) {
        set_phase(&self.state, Phase::Error, msg);
        push_log(&self.state, &self.logs, "agent", format!("✖ {msg}"));
    }

    fn log(&self, stream: &'static str, line: String) {
        push_log(&self.state, &self.logs, stream, line);
    }
}

fn push_log(
    state: &Arc<Mutex<CompanionState>>,
    logs: &broadcast::Sender<String>,
    stream: &'static str,
    line: String,
) {
    let _ = logs.send(
        Outbound::Log(LogFrame {
            stream,
            line: line.clone(),
        })
        .to_json(),
    );
    if let Ok(mut s) = state.lock() {
        s.push_log(stream, line);
    }
}

fn set_phase(state: &Arc<Mutex<CompanionState>>, phase: Phase, message: &str) {
    if let Ok(mut s) = state.lock() {
        s.phase = phase;
        s.message = message.to_string();
        if phase == Phase::Idle || phase == Phase::Error {
            s.task = "idle".to_string();
        }
    }
}

fn ack(cmd: &str, ok: bool, message: &str) -> AckFrame {
    AckFrame {
        cmd: cmd.to_string(),
        ok,
        message: message.to_string(),
    }
}

async fn run_capture(bin: &str, args: &[&str], timeout_ms: u64) -> Option<String> {
    let fut = ProcCommand::new(bin).args(args).output();
    let out = tokio::time::timeout(Duration::from_millis(timeout_ms), fut)
        .await
        .ok()?
        .ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        None
    }
}

fn which(bin: &str) -> bool {
    if bin.contains('/') {
        return Path::new(bin).is_file();
    }
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| {
        let p: PathBuf = dir.join(bin);
        p.is_file()
    })
}
