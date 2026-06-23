//! Shared, snapshot-friendly simulation state.
//!
//! A single [`SimState`] behind a `std::sync::Mutex` is mutated by the manager
//! (lifecycle, wind, mock flight) and snapshotted into wire frames by the
//! per-connection sender — the same locking discipline as `maestros`: the lock
//! is only ever held briefly, never across an `.await`.

use std::collections::VecDeque;
use std::time::Instant;

use crate::mock::MockPilot;
use crate::protocol::{PoseFrame, StatusFrame, WindFrame};
use crate::worlds;

const LOG_RING_CAP: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Starting,
    Running,
    Stopping,
    Error,
}

impl Phase {
    pub fn as_str(self) -> &'static str {
        match self {
            Phase::Idle => "idle",
            Phase::Starting => "starting",
            Phase::Running => "running",
            Phase::Stopping => "stopping",
            Phase::Error => "error",
        }
    }
}

/// One buffered log line (kept so a freshly connected panel sees recent output).
#[derive(Debug, Clone)]
pub struct LogLine {
    pub stream: &'static str,
    pub line: String,
}

pub struct SimState {
    pub phase: Phase,
    pub mock: bool,
    pub toolchain_ok: bool,
    pub world: String,
    pub vehicle: String,
    pub simulator: String,
    pub wind: WindFrame,
    pub pid: Option<u32>,
    pub message: String,
    pub started: Option<Instant>,
    pub pilot: MockPilot,
    logs: VecDeque<LogLine>,
}

impl SimState {
    pub fn new(world: String, vehicle: String, toolchain_ok: bool) -> Self {
        let ground = worlds::is_ground_vehicle(&vehicle);
        Self {
            phase: Phase::Idle,
            mock: !toolchain_ok,
            toolchain_ok,
            world,
            vehicle,
            simulator: worlds::DEFAULT_SIMULATOR.to_string(),
            wind: WindFrame {
                speed_mps: 0.0,
                direction_deg: 0.0,
                gust: 0.0,
            },
            pid: None,
            message: "idle".to_string(),
            started: None,
            pilot: MockPilot::new(ground),
            logs: VecDeque::with_capacity(LOG_RING_CAP),
        }
    }

    /// Re-seat the mock pilot for the (possibly new) vehicle kind.
    pub fn reset_pilot(&mut self) {
        self.pilot = MockPilot::new(worlds::is_ground_vehicle(&self.vehicle));
    }

    pub fn push_log(&mut self, stream: &'static str, line: String) {
        if self.logs.len() == LOG_RING_CAP {
            self.logs.pop_front();
        }
        self.logs.push_back(LogLine { stream, line });
    }

    pub fn recent_logs(&self) -> impl Iterator<Item = &LogLine> {
        self.logs.iter()
    }

    pub fn sim_time_s(&self) -> f64 {
        self.started
            .map(|t| t.elapsed().as_secs_f64())
            .unwrap_or(0.0)
    }

    pub fn status_frame(&self) -> StatusFrame {
        StatusFrame {
            phase: self.phase.as_str(),
            mock: self.mock,
            toolchain_ok: self.toolchain_ok,
            world: self.world.clone(),
            vehicle: self.vehicle.clone(),
            simulator: self.simulator.clone(),
            make_target: worlds::make_target(&self.vehicle, &self.world),
            pid: self.pid,
            wind: self.wind,
            sim_time_s: self.sim_time_s(),
            flight_mode: self.pilot.mode().as_str().to_string(),
            armed: self.pilot.armed(),
            message: self.message.clone(),
        }
    }

    pub fn pose_frame(&self, t_ms: u128) -> PoseFrame {
        self.pilot.pose_frame(t_ms)
    }
}
