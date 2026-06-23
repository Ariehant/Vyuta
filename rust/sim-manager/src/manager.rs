//! Simulation lifecycle: toolchain detection, PX4-SITL/Gazebo process control,
//! and the mock flight ticker.
//!
//! The shared [`SimState`] is read by the WebSocket senders; log lines are
//! fanned out over a `broadcast` channel (and ring-buffered in state for late
//! joiners). A single `stop` one-shot tears down whichever run is active —
//! real child processes or the mock ticker — so both paths share one code path.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::Command as ProcCommand;
use tokio::sync::{broadcast, oneshot, Mutex as AsyncMutex};
use tokio::task::JoinHandle;
use tokio::time::{interval, MissedTickBehavior};

use crate::backend::{self, SimEnv};
use crate::protocol::{AckFrame, Command, LogFrame, Outbound};
use crate::state::{Phase, SimState};
use crate::worlds;

const MOCK_DT: f64 = 0.02; // 50 Hz mock integration step
const LOG_BROADCAST_CAP: usize = 512;

/// Static configuration resolved from the environment at startup.
#[derive(Debug, Clone)]
pub struct Config {
    pub px4_dir: Option<PathBuf>,
    pub gz_bin: String,
    pub force_mock: bool,
    pub default_world: String,
    pub default_vehicle: String,
}

struct Inner {
    stop_tx: Option<oneshot::Sender<()>>,
    run_task: Option<JoinHandle<()>>,
}

pub struct SimManager {
    cfg: Config,
    state: Arc<Mutex<SimState>>,
    logs: broadcast::Sender<String>,
    inner: AsyncMutex<Inner>,
}

impl SimManager {
    pub fn new(cfg: Config) -> Arc<Self> {
        let toolchain_ok = !cfg.force_mock && detect_toolchain(&cfg);
        let state = Arc::new(Mutex::new(SimState::new(
            cfg.default_world.clone(),
            cfg.default_vehicle.clone(),
            toolchain_ok,
        )));
        let (logs, _) = broadcast::channel(LOG_BROADCAST_CAP);
        Arc::new(Self {
            cfg,
            state,
            logs,
            inner: AsyncMutex::new(Inner {
                stop_tx: None,
                run_task: None,
            }),
        })
    }

    pub fn state(&self) -> Arc<Mutex<SimState>> {
        self.state.clone()
    }

    pub fn subscribe_logs(&self) -> broadcast::Receiver<String> {
        self.logs.subscribe()
    }

    /// Dispatch a control command, returning an acknowledgement frame.
    pub async fn handle(&self, cmd: Command) -> AckFrame {
        match cmd {
            Command::Start {
                world,
                vehicle,
                simulator,
                headless,
                mock,
            } => self.start(world, vehicle, simulator, headless, mock).await,
            Command::Stop => self.stop("stopped by request").await,
            Command::Reset => self.reset().await,
            Command::SetWind {
                speed_mps,
                direction_deg,
                gust,
            } => self.set_wind(speed_mps, direction_deg, gust),
            Command::Status => ack("status", true, "status follows"),
            Command::SendMavlink { text } => self.mission(text),
        }
    }

    async fn start(
        &self,
        world: Option<String>,
        vehicle: Option<String>,
        simulator: Option<String>,
        headless: Option<bool>,
        mock: Option<bool>,
    ) -> AckFrame {
        let mut inner = self.inner.lock().await;

        // Reject if a run is already active.
        {
            let s = self.state.lock().expect("state poisoned");
            if matches!(s.phase, Phase::Starting | Phase::Running | Phase::Stopping) {
                return ack("start", false, "a simulation is already running");
            }
        }
        // Detach any finished previous run.
        inner.stop_tx.take();
        inner.run_task.take();

        let world = world.unwrap_or_else(|| self.cfg.default_world.clone());
        let vehicle = vehicle.unwrap_or_else(|| self.cfg.default_vehicle.clone());
        let sim_id = simulator.unwrap_or_else(|| worlds::DEFAULT_SIMULATOR.to_string());
        let headless = headless.unwrap_or(true);

        // Pick the simulator backend and check whether a real run is possible.
        let sim = backend::backend(&sim_id);
        let env = SimEnv {
            px4_dir: self.cfg.px4_dir.as_deref(),
            gz_bin: &self.cfg.gz_bin,
        };
        let real_possible = !self.cfg.force_mock && sim.available(&env);

        let use_mock = match mock {
            Some(true) => true,
            Some(false) => {
                if real_possible {
                    false
                } else {
                    self.log(
                        "sim",
                        format!("{} unavailable — falling back to mock", sim.label()),
                    );
                    true
                }
            }
            None => self.cfg.force_mock || !real_possible,
        };

        // Re-seat state for this run.
        {
            let mut s = self.state.lock().expect("state poisoned");
            s.world = world.clone();
            s.vehicle = vehicle.clone();
            s.simulator = sim.id().to_string();
            s.mock = use_mock;
            s.phase = Phase::Starting;
            s.started = Some(Instant::now());
            s.pid = None;
            s.reset_pilot();
        }

        let target = worlds::make_target(&vehicle, &world);
        let (stop_tx, stop_rx) = oneshot::channel();

        if use_mock {
            {
                let mut s = self.state.lock().expect("state poisoned");
                s.pilot.begin();
                s.phase = Phase::Running;
                s.message = format!(
                    "mock flight — {} on {}",
                    worlds::vehicle_label(&vehicle),
                    worlds::world_label(&world)
                );
            }
            self.log(
                "sim",
                format!(
                    "▶ mock simulation started ({} · {}) — target `{}`",
                    worlds::vehicle_label(&vehicle),
                    worlds::world_label(&world),
                    target
                ),
            );
            let task = self.spawn_mock(stop_rx);
            inner.stop_tx = Some(stop_tx);
            inner.run_task = Some(task);
            return ack("start", true, "mock simulation started");
        }

        // --- real simulator via the selected backend -------------------------
        let spec = sim.launch(&vehicle, &world, headless, &env);
        let invocation = format!("{} {}", spec.program, spec.args.join(" "));

        let mut command = ProcCommand::new(&spec.program);
        command
            .args(&spec.args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if let Some(cwd) = &spec.cwd {
            command.current_dir(cwd);
        }
        for (k, v) in &spec.env {
            command.env(k, v);
        }

        let mut child = match command.spawn() {
            Ok(c) => c,
            Err(e) => {
                self.fail("start", &format!("failed to spawn `{invocation}`: {e}"));
                return ack("start", false, &format!("spawn failed: {e}"));
            }
        };
        let pid = child.id();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        if let Some(o) = stdout {
            self.spawn_reader(o, "stdout");
        }
        if let Some(e) = stderr {
            self.spawn_reader(e, "stderr");
        }

        {
            let mut s = self.state.lock().expect("state poisoned");
            s.phase = Phase::Running;
            s.pid = pid;
            s.message = format!("running `{invocation}` (pid {})", pid.unwrap_or(0));
        }
        self.log(
            "sim",
            format!("▶ launched `{invocation}` ({})", sim.label()),
        );

        let state = self.state.clone();
        let logs = self.logs.clone();
        let task = tokio::spawn(async move {
            let mut stop_rx = stop_rx;
            tokio::select! {
                status = child.wait() => {
                    let msg = match status {
                        Ok(s) if s.success() => "simulator exited cleanly".to_string(),
                        Ok(s) => format!("simulator exited with {s}"),
                        Err(e) => format!("error waiting on simulator: {e}"),
                    };
                    set_phase(&state, Phase::Idle, &msg, None);
                    push_log(&state, &logs, "sim", format!("■ {msg}"));
                }
                _ = &mut stop_rx => {
                    let _ = child.start_kill();
                    let _ = child.wait().await;
                    set_phase(&state, Phase::Idle, "stopped by request", None);
                    push_log(&state, &logs, "sim", "■ simulator stopped".into());
                }
            }
        });
        inner.stop_tx = Some(stop_tx);
        inner.run_task = Some(task);
        ack("start", true, &format!("started `{invocation}`"))
    }

    fn spawn_mock(&self, stop_rx: oneshot::Receiver<()>) -> JoinHandle<()> {
        let state = self.state.clone();
        tokio::spawn(async move {
            let mut stop_rx = stop_rx;
            let mut ticker = interval(Duration::from_secs_f64(MOCK_DT));
            ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        let mut s = state.lock().expect("state poisoned");
                        let wind = s.wind;
                        s.pilot.step(MOCK_DT, wind);
                    }
                    _ = &mut stop_rx => break,
                }
            }
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

    async fn stop(&self, reason: &str) -> AckFrame {
        let mut inner = self.inner.lock().await;
        {
            let mut s = self.state.lock().expect("state poisoned");
            if matches!(s.phase, Phase::Idle | Phase::Error) {
                return ack("stop", true, "already stopped");
            }
            s.phase = Phase::Stopping;
            s.message = "stopping…".to_string();
        }
        if let Some(tx) = inner.stop_tx.take() {
            let _ = tx.send(());
        }
        if let Some(task) = inner.run_task.take() {
            let _ = task.await;
        }
        {
            let mut s = self.state.lock().expect("state poisoned");
            s.phase = Phase::Idle;
            s.pid = None;
            s.started = None;
            s.message = reason.to_string();
            s.pilot.halt();
        }
        self.log("sim", format!("■ {reason}"));
        ack("stop", true, reason)
    }

    async fn reset(&self) -> AckFrame {
        self.stop("reset").await;
        {
            let mut s = self.state.lock().expect("state poisoned");
            s.reset_pilot();
            s.wind = crate::protocol::WindFrame {
                speed_mps: 0.0,
                direction_deg: 0.0,
                gust: 0.0,
            };
            s.message = "reset to home".to_string();
        }
        self.log("sim", "↺ reset to home".into());
        ack("reset", true, "reset to home")
    }

    fn set_wind(&self, speed: f64, dir: f64, gust: Option<f64>) -> AckFrame {
        let speed = speed.max(0.0);
        let dir = dir.rem_euclid(360.0);
        let gust = gust.unwrap_or(0.0).clamp(0.0, 1.0);
        {
            let mut s = self.state.lock().expect("state poisoned");
            s.wind = crate::protocol::WindFrame {
                speed_mps: speed,
                direction_deg: dir,
                gust,
            };
        }
        let msg = format!(
            "wind {speed:.1} m/s @ {dir:.0}° (gust {:.0}%)",
            gust * 100.0
        );
        self.log("sim", format!("🌬 {msg}"));
        ack("set_wind", true, &msg)
    }

    fn mission(&self, text: String) -> AckFrame {
        let mut s = self.state.lock().expect("state poisoned");
        if !matches!(s.phase, Phase::Running) {
            return ack("send_mavlink", false, "no simulation running");
        }
        if s.mock {
            let result = s.pilot.handle_command(&text);
            drop(s);
            self.log("sim", format!("» {text}  →  {result}"));
            ack("send_mavlink", true, &result)
        } else {
            drop(s);
            let note = format!("REPL forwarding to live MAVLink not yet wired — ignored `{text}`");
            self.log("sim", format!("» {note}"));
            ack("send_mavlink", true, &note)
        }
    }

    fn log(&self, stream: &'static str, line: String) {
        push_log(&self.state, &self.logs, stream, line);
    }

    fn fail(&self, _cmd: &str, msg: &str) {
        set_phase(&self.state, Phase::Error, msg, None);
        push_log(&self.state, &self.logs, "sim", format!("✖ {msg}"));
    }
}

// --- free helpers (usable from spawned tasks without &self) ------------------

fn push_log(
    state: &Arc<Mutex<SimState>>,
    logs: &broadcast::Sender<String>,
    stream: &'static str,
    line: String,
) {
    let frame = Outbound::Log(LogFrame {
        stream,
        line: line.clone(),
    });
    let _ = logs.send(frame.to_json());
    if let Ok(mut s) = state.lock() {
        s.push_log(stream, line);
    }
}

fn set_phase(state: &Arc<Mutex<SimState>>, phase: Phase, message: &str, pid: Option<u32>) {
    if let Ok(mut s) = state.lock() {
        s.phase = phase;
        s.message = message.to_string();
        s.pid = pid;
        if matches!(phase, Phase::Idle | Phase::Error) {
            s.started = None;
            s.pilot.halt();
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

/// True when a real PX4 source tree (Makefile) and a `gz`/`gazebo` binary are
/// both present — i.e. a real SITL run is possible on this host.
fn detect_toolchain(cfg: &Config) -> bool {
    let px4 = cfg
        .px4_dir
        .as_ref()
        .map(|d| px4_makefile_present(d))
        .unwrap_or(false);
    px4 && which(&cfg.gz_bin)
}

fn px4_makefile_present(dir: &Path) -> bool {
    dir.join("Makefile").is_file()
}

/// Minimal `which`: scan `$PATH` for an executable file named `bin`.
fn which(bin: &str) -> bool {
    if bin.contains('/') {
        return Path::new(bin).is_file();
    }
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| dir.join(bin).is_file())
}
