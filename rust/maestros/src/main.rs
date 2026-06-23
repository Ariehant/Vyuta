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

mod params;
mod preflight;
mod px4;
mod recorder;
mod sources;
mod telemetry;
mod ws;

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::Result;

use params::{Link, ParamService, ParamStore, SharedConn};
use recorder::Recorder;
use telemetry::{Source, TelemetryState};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "maestros=info".into()),
        )
        .init();

    // Dev tool: `maestros --bench [frames]` profiles the telemetry pipeline.
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--bench") {
        let n: u64 = args
            .get(i + 1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(200_000);
        bench(n);
        return Ok(());
    }

    let addr: SocketAddr = env_or("VYUTA_MAESTROS_ADDR", "127.0.0.1:9876").parse()?;
    let emit_hz: f64 = env_or("VYUTA_EMIT_HZ", "30").parse().unwrap_or(30.0);
    let link_timeout = Duration::from_millis(
        env_or("VYUTA_LINK_TIMEOUT_MS", "3000")
            .parse()
            .unwrap_or(3000),
    );
    let record_dir = env_or("VYUTA_RECORD_DIR", ".");
    let recorder = Recorder::new(record_dir);
    let mav_url = std::env::var("VYUTA_MAVLINK_URL")
        .ok()
        .filter(|s| !s.is_empty());

    let source = if mav_url.is_some() {
        Source::Mavlink
    } else {
        Source::Synthetic
    };
    let state = Arc::new(Mutex::new(TelemetryState::new(source)));
    let pstore = Arc::new(Mutex::new(ParamStore::new()));

    let params = match mav_url {
        Some(url) => {
            tracing::info!(%url, "telemetry source: MAVLink");
            let conn_slot: SharedConn = Arc::new(Mutex::new(None));
            sources::mavlink_source::spawn(url, state.clone(), pstore.clone(), conn_slot.clone());
            ParamService::new(pstore.clone(), Link::Mavlink(conn_slot))
        }
        None => {
            tracing::info!("telemetry source: synthetic (set VYUTA_MAVLINK_URL for a real link)");
            sources::synthetic::spawn(state.clone());
            params::seed_synthetic(&pstore);
            ParamService::new(pstore.clone(), Link::Synthetic)
        }
    };

    ws::serve(addr, state, params, recorder, emit_hz, link_timeout).await
}

/// Profile the build+serialize cost of a telemetry frame to confirm the JSON
/// pipeline has headroom well beyond the >1 kHz Phase 8 target.
fn bench(n: u64) {
    let mut s = TelemetryState::new(Source::Synthetic);
    s.battery_pct = Some(73.0);
    s.lat = Some(47.397_742);
    s.lon = Some(8.545_594);
    s.heading_deg = Some(123.0);
    let timeout = Duration::from_millis(3000);
    let start = Instant::now();
    let mut bytes = 0u64;
    for seq in 0..n {
        let frame = s.to_frame(seq, timeout);
        let json = serde_json::to_string(&frame).expect("serialize");
        bytes += json.len() as u64;
    }
    let elapsed = start.elapsed().as_secs_f64();
    let hz = n as f64 / elapsed;
    println!(
        "maestros bench: {n} frames in {elapsed:.3}s = {hz:.0} Hz ({:.1} MB)\n>1 kHz target: {}",
        bytes as f64 / 1e6,
        if hz > 1000.0 { "PASS" } else { "FAIL" }
    );
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| default.to_string())
}
