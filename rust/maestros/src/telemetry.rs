//! Shared telemetry state and the wire frame pushed to the UI.
//!
//! A single [`TelemetryState`] is updated by whichever source is active
//! (real MAVLink or the synthetic generator) and snapshotted into a
//! [`TelemetryFrame`] by the WebSocket sender at a fixed rate. Fields that are
//! not yet known are `Option::None` so the UI can render them as "—" rather
//! than a misleading zero.

use std::time::{Duration, Instant};

use serde::Serialize;

/// Where the telemetry is coming from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// Decoded from a live MAVLink link.
    Mavlink,
    /// Generated locally (no link configured) for demos/tests.
    Synthetic,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Mavlink => "mavlink",
            Source::Synthetic => "synthetic",
        }
    }
}

/// Mutable, source-agnostic vehicle state. Cheap to clone for snapshotting.
#[derive(Debug, Clone)]
pub struct TelemetryState {
    pub started: Instant,
    pub source: Source,

    // Attitude (radians). Always present (defaults level).
    pub roll: f64,
    pub pitch: f64,
    pub yaw: f64,

    // Position / navigation.
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub alt_m: Option<f64>,
    pub rel_alt_m: Option<f64>,
    pub heading_deg: Option<f64>,

    // Battery.
    pub battery_v: Option<f64>,
    pub battery_pct: Option<f64>,
    pub current_a: Option<f64>,

    // Air data.
    pub groundspeed_mps: Option<f64>,
    pub airspeed_mps: Option<f64>,
    pub climb_mps: Option<f64>,
    pub throttle_pct: Option<f64>,

    // Status.
    pub armed: bool,
    pub mode: String,
    pub system_status: String,

    pub last_heartbeat: Option<Instant>,
    pub last_update: Option<Instant>,
}

impl TelemetryState {
    pub fn new(source: Source) -> Self {
        Self {
            started: Instant::now(),
            source,
            roll: 0.0,
            pitch: 0.0,
            yaw: 0.0,
            lat: None,
            lon: None,
            alt_m: None,
            rel_alt_m: None,
            heading_deg: None,
            battery_v: None,
            battery_pct: None,
            current_a: None,
            groundspeed_mps: None,
            airspeed_mps: None,
            climb_mps: None,
            throttle_pct: None,
            armed: false,
            mode: "—".to_string(),
            system_status: "—".to_string(),
            last_heartbeat: None,
            last_update: None,
        }
    }

    /// Snapshot the current state into a wire frame.
    ///
    /// `link_timeout` decides whether the link is considered healthy based on
    /// the age of the last HEARTBEAT (synthetic source is always healthy).
    pub fn to_frame(&self, seq: u64, link_timeout: Duration) -> TelemetryFrame {
        let now = Instant::now();
        let heartbeat_age_ms = self
            .last_heartbeat
            .map(|t| now.duration_since(t).as_millis());
        let link_ok = match self.source {
            Source::Synthetic => true,
            Source::Mavlink => self
                .last_heartbeat
                .map(|t| now.duration_since(t) < link_timeout)
                .unwrap_or(false),
        };

        TelemetryFrame {
            seq,
            t_ms: now.duration_since(self.started).as_millis(),
            source: self.source.as_str(),
            synthetic: self.source == Source::Synthetic,
            link_ok,
            heartbeat_age_ms,
            roll: self.roll,
            pitch: self.pitch,
            yaw: self.yaw,
            heading_deg: self.heading_deg,
            lat: self.lat,
            lon: self.lon,
            alt_m: self.alt_m,
            rel_alt_m: self.rel_alt_m,
            battery_v: self.battery_v,
            battery_pct: self.battery_pct,
            current_a: self.current_a,
            groundspeed_mps: self.groundspeed_mps,
            airspeed_mps: self.airspeed_mps,
            climb_mps: self.climb_mps,
            throttle_pct: self.throttle_pct,
            armed: self.armed,
            mode: self.mode.clone(),
            system_status: self.system_status.clone(),
        }
    }
}

/// The JSON frame sent to the UI over the WebSocket.
#[derive(Debug, Clone, Serialize)]
pub struct TelemetryFrame {
    pub seq: u64,
    pub t_ms: u128,
    pub source: &'static str,
    pub synthetic: bool,
    pub link_ok: bool,
    pub heartbeat_age_ms: Option<u128>,

    pub roll: f64,
    pub pitch: f64,
    pub yaw: f64,
    pub heading_deg: Option<f64>,

    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub alt_m: Option<f64>,
    pub rel_alt_m: Option<f64>,

    pub battery_v: Option<f64>,
    pub battery_pct: Option<f64>,
    pub current_a: Option<f64>,

    pub groundspeed_mps: Option<f64>,
    pub airspeed_mps: Option<f64>,
    pub climb_mps: Option<f64>,
    pub throttle_pct: Option<f64>,

    pub armed: bool,
    pub mode: String,
    pub system_status: String,
}
