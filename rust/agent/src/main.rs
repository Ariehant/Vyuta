//! Vyuta `vyuta-agent` — companion-computer agent (Phase 6).
//!
//! Runs on the drone's companion computer (RPi/Jetson). Introspects the ROS 2
//! graph, runs `colcon build`, and deploys the workspace to the drone over
//! rsync/SSH — exposed to the IDE panel as JSON over a WebSocket. When ROS 2 /
//! colcon / rsync are absent it serves a synthetic graph and simulates
//! build/deploy so the panel works on a dev box — the same out-of-the-box
//! philosophy as the other Vyuta sidecars.
//!
//! Configuration (environment variables):
//!   VYUTA_AGENT_ADDR      WebSocket bind address     (default 127.0.0.1:9879)
//!   VYUTA_ROS2_BIN        ros2 executable            (default ros2)
//!   VYUTA_COLCON_BIN      colcon executable          (default colcon)
//!   VYUTA_RSYNC_BIN       rsync executable           (default rsync)
//!   VYUTA_WS_DIR          colcon workspace directory (default .)
//!   VYUTA_DEPLOY_TARGET   rsync target host:path     (default: unset)

mod graph;
mod manager;
mod protocol;
mod state;
mod ws;

use std::net::SocketAddr;

use anyhow::Result;

use manager::{Agent, Config};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "vyuta_agent=info".into()),
        )
        .init();

    let addr: SocketAddr = env_or("VYUTA_AGENT_ADDR", "127.0.0.1:9879").parse()?;
    let cfg = Config {
        ros2_bin: env_or("VYUTA_ROS2_BIN", "ros2"),
        colcon_bin: env_or("VYUTA_COLCON_BIN", "colcon"),
        rsync_bin: env_or("VYUTA_RSYNC_BIN", "rsync"),
        workspace: env_or("VYUTA_WS_DIR", "."),
        deploy_target: std::env::var("VYUTA_DEPLOY_TARGET").unwrap_or_default(),
    };

    let agent = Agent::new(cfg);
    {
        let st = agent.state();
        let s = st.lock().expect("state poisoned");
        tracing::info!(
            ros = s.ros_available,
            colcon = s.colcon_available,
            "vyuta-agent starting"
        );
        if !s.ros_available {
            tracing::info!("ROS 2 not found — serving a synthetic graph + simulated build/deploy");
        }
    }

    ws::serve(addr, agent).await
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| default.to_string())
}
