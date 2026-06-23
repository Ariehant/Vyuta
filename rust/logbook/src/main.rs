//! Vyuta `logbook` — flight-log analysis sidecar (Phase 5).
//!
//! Parses PX4 ULog (`.ulg`) into named time series, serves downsampled data +
//! an auto-review over a request/response WebSocket, and falls back to a
//! synthetic flight log when no file is given so the browser works out of the
//! box — the same philosophy as the synthetic telemetry source in `maestros`.
//!
//! Configuration (environment variables):
//!   VYUTA_LOGBOOK_ADDR  WebSocket bind address   (default 127.0.0.1:9878)
//!   VYUTA_ULOG_PATH     `.ulg` to load at start  (default: synthetic log)

mod model;
mod review;
mod synthetic;
mod ulog;
mod ws;

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "logbook=info".into()),
        )
        .init();

    // Dev helper: `logbook --write-ulog <path>` writes a synthetic .ulg and exits.
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--write-ulog") {
        let path = args
            .get(i + 1)
            .cloned()
            .unwrap_or_else(|| "synthetic.ulg".into());
        std::fs::write(&path, synthetic::synthetic_ulog_bytes())?;
        tracing::info!(path = %path, "wrote synthetic ULog");
        return Ok(());
    }

    let addr: SocketAddr = env_or("VYUTA_LOGBOOK_ADDR", "127.0.0.1:9878").parse()?;
    let path = std::env::var("VYUTA_ULOG_PATH")
        .ok()
        .filter(|s| !s.is_empty());

    let log = match &path {
        Some(p) => match ulog::parse_file(p) {
            Ok(log) => {
                tracing::info!(path = %p, series = log.series.len(), "loaded ULog");
                log
            }
            Err(e) => {
                tracing::warn!(path = %p, error = %e, "failed to load ULog — using synthetic log");
                synthetic::synthetic_log()
            }
        },
        None => {
            tracing::info!("no VYUTA_ULOG_PATH — serving a synthetic flight log");
            synthetic::synthetic_log()
        }
    };

    let state = Arc::new(Mutex::new(log));
    ws::serve(addr, state).await
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| default.to_string())
}
