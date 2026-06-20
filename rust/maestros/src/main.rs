//! Vyuta `maestros` telemetry gateway.
//!
//! Decodes MAVLink telemetry (Phase 1) and streams it to the UI as JSON over a
//! WebSocket. When no MAVLink endpoint is configured it falls back to a
//! synthetic generator so the dashboard works out of the box.
//!
//! Configuration (environment variables):
//!   VYUTA_MAESTROS_ADDR   WebSocket bind address      (default 127.0.0.1:9876)
//!   VYUTA_MAVLINK_URL     MAVLink endpoint, e.g.       (default: unset ->
//!                         `udpin:0.0.0.0:14550`         synthetic source)
//!   VYUTA_EMIT_HZ         UI frame rate                (default 30)
//!   VYUTA_LINK_TIMEOUT_MS HEARTBEAT staleness for      (default 3000)
//!                         link-loss

mod px4;
mod sources;
mod telemetry;
mod ws;

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;

use telemetry::{Source, TelemetryState};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "maestros=info".into()),
        )
        .init();

    let addr: SocketAddr = env_or("VYUTA_MAESTROS_ADDR", "127.0.0.1:9876").parse()?;
    let emit_hz: f64 = env_or("VYUTA_EMIT_HZ", "30").parse().unwrap_or(30.0);
    let link_timeout = Duration::from_millis(
        env_or("VYUTA_LINK_TIMEOUT_MS", "3000")
            .parse()
            .unwrap_or(3000),
    );
    let mav_url = std::env::var("VYUTA_MAVLINK_URL")
        .ok()
        .filter(|s| !s.is_empty());

    let source = if mav_url.is_some() {
        Source::Mavlink
    } else {
        Source::Synthetic
    };
    let state = Arc::new(Mutex::new(TelemetryState::new(source)));

    match mav_url {
        Some(url) => {
            tracing::info!(%url, "telemetry source: MAVLink");
            sources::mavlink_source::spawn(url, state.clone());
        }
        None => {
            tracing::info!("telemetry source: synthetic (set VYUTA_MAVLINK_URL for a real link)");
            sources::synthetic::spawn(state.clone());
        }
    }

    ws::serve(addr, state, emit_hz, link_timeout).await
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}
