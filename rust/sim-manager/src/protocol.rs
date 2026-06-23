//! Wire protocol between the simulation control panel (TypeScript webview) and
//! the `sim-manager` sidecar.
//!
//! The transport is JSON over a WebSocket — the same pragmatic choice the
//! `maestros` telemetry gateway makes (Phase 1: JSON rather than FlatBuffers
//! for want of `flatc`). The plan calls for gRPC; that is the documented
//! upgrade path once a protoc toolchain is available (see `Cargo.toml`).
//!
//! Two directions:
//! - [`Command`] — client → server control messages (tagged by `cmd`).
//! - [`Outbound`] — server → client frames (tagged by `type`).

use serde::{Deserialize, Serialize};

/// A control message sent by the panel to the sidecar.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    /// Start a simulation. All fields fall back to the sidecar's configured
    /// defaults when omitted.
    Start {
        #[serde(default)]
        world: Option<String>,
        #[serde(default)]
        vehicle: Option<String>,
        /// Simulator backend id (`gazebo` | `jmavsim` | `airsim`). Default gazebo.
        #[serde(default)]
        simulator: Option<String>,
        /// Run the simulator headless (no Gazebo GUI). Defaults to `true`.
        #[serde(default)]
        headless: Option<bool>,
        /// Force the built-in mock flight even when a real toolchain is present
        /// (useful for UI work). When omitted the sidecar auto-detects.
        #[serde(default)]
        mock: Option<bool>,
    },
    /// Stop the running simulation (kills child processes / mock flight).
    Stop,
    /// Reset: stop and clear pose back to the origin.
    Reset,
    /// Inject a steady wind plus optional gusting. Applied live to the mock
    /// flight; logged (and forwarded to Gazebo in a later phase) for real sims.
    SetWind {
        speed_mps: f64,
        direction_deg: f64,
        #[serde(default)]
        gust: Option<f64>,
    },
    /// Request an immediate status frame.
    Status,
    /// Mission REPL line. In mock mode a tiny built-in autopilot interprets a
    /// handful of verbs (`arm`, `takeoff`, `goto`, `land`, `rtl`, …); for real
    /// sims it is echoed/logged pending MAVLink forwarding.
    SendMavlink { text: String },
}

/// A frame streamed from the sidecar to the panel.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Outbound {
    /// Sent once on connect: the world/vehicle catalogue for the pickers.
    Catalog(CatalogFrame),
    /// Lifecycle + configuration snapshot (low rate / on change).
    Status(StatusFrame),
    /// Vehicle pose for the 3D viewport (high rate while running).
    Pose(PoseFrame),
    /// A line of simulator output (or a sidecar note).
    Log(LogFrame),
    /// Reply to a [`Command`].
    Ack(AckFrame),
}

#[derive(Debug, Clone, Serialize)]
pub struct CatalogFrame {
    pub worlds: Vec<CatalogEntry>,
    pub vehicles: Vec<CatalogEntry>,
    pub simulators: Vec<CatalogEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CatalogEntry {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    /// Vehicle class for per-vehicle profiles (multirotor | vtol | fixedwing |
    /// rover); empty for worlds/simulators.
    pub class: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatusFrame {
    /// `idle` | `starting` | `running` | `stopping` | `error`.
    pub phase: &'static str,
    /// True when the built-in mock flight is driving the scene (no real sim).
    pub mock: bool,
    /// Whether a real PX4 + Gazebo toolchain was detected on this host.
    pub toolchain_ok: bool,
    pub world: String,
    pub vehicle: String,
    /// Selected simulator backend id.
    pub simulator: String,
    /// `make` target the sidecar would run (or did run) for this combo.
    pub make_target: String,
    pub pid: Option<u32>,
    pub wind: WindFrame,
    /// Seconds since the current run started (0 when idle).
    pub sim_time_s: f64,
    /// Mock autopilot mode (`IDLE`, `TAKEOFF`, `HOLD`, `GOTO`, `LAND`, `RTL`).
    pub flight_mode: String,
    pub armed: bool,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct WindFrame {
    pub speed_mps: f64,
    pub direction_deg: f64,
    pub gust: f64,
}

/// Vehicle pose in a local ENU frame (metres / radians), origin at the home
/// position. The viewport maps this to its scene graph directly.
#[derive(Debug, Clone, Serialize)]
pub struct PoseFrame {
    pub t_ms: u128,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub roll: f64,
    pub pitch: f64,
    pub yaw: f64,
    pub vx: f64,
    pub vy: f64,
    pub vz: f64,
    pub airborne: bool,
    pub armed: bool,
    pub flight_mode: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LogFrame {
    /// `stdout` | `stderr` | `sim` (sidecar note).
    pub stream: &'static str,
    pub line: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AckFrame {
    pub cmd: String,
    pub ok: bool,
    pub message: String,
}

impl Outbound {
    /// Serialize to a JSON string for sending over the socket.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|e| {
            format!("{{\"type\":\"log\",\"stream\":\"sim\",\"line\":\"serialize error: {e}\"}}")
        })
    }
}
