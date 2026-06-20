//! Vyuta companion-computer agent — Phase 0 scaffold.
//!
//! This daemon is meant to run on the drone's companion computer (RPi/Jetson).
//! Phase 6 turns it into a tonic gRPC server handling workspace file sync,
//! `colcon build`, and ROS 2 node lifecycle. For Phase 0 it only proves the
//! crate builds and runs inside the workspace, logging a heartbeat so the
//! deployment story has a real binary to target.

use std::time::Duration;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "vyuta_agent=info".into()),
        )
        .init();

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        "vyuta-agent starting (Phase 0 scaffold — gRPC server arrives in Phase 6)"
    );

    // A single heartbeat then exit keeps Phase 0 CI/build verification fast.
    // Set VYUTA_AGENT_DAEMON=1 to idle as a long-running heartbeat instead.
    let daemon = std::env::var("VYUTA_AGENT_DAEMON").is_ok();
    let mut ticks = 0u64;
    loop {
        tracing::info!(tick = ticks, "heartbeat");
        ticks += 1;
        if !daemon {
            break;
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }

    Ok(())
}
