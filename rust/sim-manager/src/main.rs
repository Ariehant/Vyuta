//! Vyuta `sim-manager` — simulation control sidecar (Phase 3).
//!
//! Manages PX4-SITL + Gazebo as child processes and streams pose/status/logs
//! to the simulation control panel over a JSON WebSocket. When no real
//! toolchain is present (or `VYUTA_SIM_MOCK=1`) it flies a built-in mock so the
//! panel and 3D viewport work out of the box — the same out-of-the-box
//! philosophy as the synthetic telemetry source in `maestros`.
//!
//! Configuration (environment variables):
//!   VYUTA_SIM_ADDR     WebSocket bind address        (default 127.0.0.1:9877)
//!   VYUTA_PX4_DIR      PX4-Autopilot source tree     (default: unset)
//!   VYUTA_GZ_BIN       Gazebo binary name/path       (default: gz)
//!   VYUTA_SIM_MOCK     Force the mock flight (1/true) (default: auto-detect)
//!   VYUTA_SIM_WORLD    Default world id              (default: default)
//!   VYUTA_SIM_VEHICLE  Default vehicle id            (default: x500)

mod manager;
mod mock;
mod protocol;
mod state;
mod worlds;
mod ws;

use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::Result;

use manager::{Config, SimManager};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "sim_manager=info".into()),
        )
        .init();

    let addr: SocketAddr = env_or("VYUTA_SIM_ADDR", "127.0.0.1:9877").parse()?;
    let px4_dir = std::env::var("VYUTA_PX4_DIR")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from);
    let gz_bin = env_or("VYUTA_GZ_BIN", "gz");
    let force_mock = is_truthy(&std::env::var("VYUTA_SIM_MOCK").unwrap_or_default());
    let default_world = env_or("VYUTA_SIM_WORLD", worlds::DEFAULT_WORLD);
    let default_vehicle = env_or("VYUTA_SIM_VEHICLE", worlds::DEFAULT_VEHICLE);

    let cfg = Config {
        px4_dir: px4_dir.clone(),
        gz_bin,
        force_mock,
        default_world,
        default_vehicle,
    };

    let manager = SimManager::new(cfg);
    {
        let s = manager.state();
        let s = s.lock().expect("state poisoned");
        if s.toolchain_ok {
            tracing::info!(px4 = ?px4_dir, "real PX4 + Gazebo toolchain detected");
        } else if force_mock {
            tracing::info!("VYUTA_SIM_MOCK set — mock flight forced");
        } else {
            tracing::info!(
                "no PX4/Gazebo toolchain — mock flight will be used (set VYUTA_PX4_DIR + install gz for real SITL)"
            );
        }
    }

    ws::serve(addr, manager).await
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn is_truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}
