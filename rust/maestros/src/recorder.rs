//! Flight recording (Phase 8).
//!
//! Records the telemetry stream to a newline-delimited JSON "tlog" while active.
//! A single global recorder snapshots the shared [`TelemetryState`] at the emit
//! rate on its own task; start/stop are driven from the WebSocket layer.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::io::AsyncWriteExt;
use tokio::sync::{oneshot, Mutex as AsyncMutex};
use tokio::time::{interval, MissedTickBehavior};

use crate::telemetry::TelemetryState;

struct Active {
    stop: oneshot::Sender<()>,
    task: tokio::task::JoinHandle<()>,
}

#[derive(Clone, Default)]
pub struct RecStatus {
    pub recording: bool,
    pub path: String,
}

pub struct Recorder {
    active: AsyncMutex<Option<Active>>,
    status: Mutex<RecStatus>,
    frames: Arc<AtomicU64>,
    dir: String,
}

impl Recorder {
    pub fn new(dir: String) -> Arc<Self> {
        Arc::new(Self {
            active: AsyncMutex::new(None),
            status: Mutex::new(RecStatus::default()),
            frames: Arc::new(AtomicU64::new(0)),
            dir,
        })
    }

    pub fn status(&self) -> RecStatus {
        self.status
            .lock()
            .expect("recorder status poisoned")
            .clone()
    }

    pub fn frames(&self) -> u64 {
        self.frames.load(Ordering::Relaxed)
    }

    /// Start recording to a timestamped file. Returns the path, or an error.
    pub async fn start(
        &self,
        state: Arc<Mutex<TelemetryState>>,
        emit_hz: f64,
        link_timeout: Duration,
    ) -> Result<String, String> {
        let mut active = self.active.lock().await;
        if active.is_some() {
            return Err("already recording".to_string());
        }
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let path = format!("{}/vyuta-{secs}.tlog.jsonl", self.dir.trim_end_matches('/'));
        let mut file = tokio::fs::File::create(&path)
            .await
            .map_err(|e| format!("create {path}: {e}"))?;

        self.frames.store(0, Ordering::Relaxed);
        let frames = self.frames.clone();
        let (stop_tx, mut stop_rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            let mut seq: u64 = 0;
            let mut ticker = interval(Duration::from_secs_f64(1.0 / emit_hz));
            ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        let line = {
                            let s = state.lock().expect("telemetry state mutex poisoned");
                            serde_json::to_string(&s.to_frame(seq, link_timeout))
                        };
                        if let Ok(mut l) = line {
                            l.push('\n');
                            if file.write_all(l.as_bytes()).await.is_err() {
                                break;
                            }
                            seq += 1;
                            frames.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    _ = &mut stop_rx => break,
                }
            }
            let _ = file.flush().await;
        });

        *active = Some(Active {
            stop: stop_tx,
            task,
        });
        *self.status.lock().expect("recorder status poisoned") = RecStatus {
            recording: true,
            path: path.clone(),
        };
        Ok(path)
    }

    /// Stop recording. Returns the path that was being written, if any.
    pub async fn stop(&self) -> Option<String> {
        let mut active = self.active.lock().await;
        let a = active.take()?;
        let _ = a.stop.send(());
        let _ = a.task.await;
        let mut st = self.status.lock().expect("recorder status poisoned");
        let path = st.path.clone();
        st.recording = false;
        Some(path)
    }
}
