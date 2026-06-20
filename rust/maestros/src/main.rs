//! Vyuta `maestros` telemetry gateway — Phase 0 scaffold.
//!
//! In Phase 0 this sidecar simply serves a JSON WebSocket on
//! `localhost:9876` and pushes synthetic telemetry frames at a fixed rate.
//! Its only job here is to prove the Rust → TypeScript transport works end to
//! end: the `drone-telemetry` VS Code extension connects to this socket and
//! renders whatever it receives.
//!
//! Phase 1 replaces the synthetic generator with a real `mavlink` decoder
//! (UDP/TCP) and swaps the JSON payload for FlatBuffers. The connection
//! lifecycle and broadcast plumbing below are designed to survive that change.

use std::net::SocketAddr;
use std::time::Duration;

use anyhow::Result;
use futures_util::SinkExt;
use serde::Serialize;
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{interval, MissedTickBehavior};
use tokio_tungstenite::tungstenite::Message;

/// Default bind address for the Phase 0 dummy WebSocket.
const DEFAULT_ADDR: &str = "127.0.0.1:9876";
/// Telemetry emit rate (Hz) for the synthetic generator.
const EMIT_HZ: f64 = 30.0;

/// A synthetic telemetry frame. The shape mirrors the fields the Phase 1
/// MAVLink decoder will eventually populate (attitude + position + battery),
/// so the webview can be built against a stable contract from day one.
#[derive(Debug, Serialize)]
struct TelemetryFrame {
    /// Monotonic frame counter.
    seq: u64,
    /// Milliseconds since the sidecar started.
    t_ms: u128,
    /// Attitude in radians.
    roll: f64,
    pitch: f64,
    yaw: f64,
    /// Position (synthetic walk around a fixed origin).
    lat: f64,
    lon: f64,
    alt_m: f64,
    /// Battery state.
    battery_v: f64,
    battery_pct: f64,
    /// Vehicle status flags.
    armed: bool,
    mode: &'static str,
    /// Marks Phase 0 synthetic data so the UI can badge it clearly.
    synthetic: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "maestros=info".into()),
        )
        .init();

    let addr: SocketAddr = std::env::var("VYUTA_MAESTROS_ADDR")
        .unwrap_or_else(|_| DEFAULT_ADDR.to_string())
        .parse()?;

    let listener = TcpListener::bind(addr).await?;
    tracing::info!(%addr, "maestros telemetry gateway listening (Phase 0 / synthetic JSON)");

    loop {
        let (stream, peer) = listener.accept().await?;
        tokio::spawn(async move {
            if let Err(err) = handle_connection(stream, peer).await {
                tracing::warn!(%peer, error = %err, "connection closed with error");
            }
        });
    }
}

/// Upgrade a TCP stream to a WebSocket and stream synthetic telemetry until the
/// client disconnects.
async fn handle_connection(stream: TcpStream, peer: SocketAddr) -> Result<()> {
    let mut ws = tokio_tungstenite::accept_async(stream).await?;
    tracing::info!(%peer, "client connected");

    let start = std::time::Instant::now();
    let mut seq: u64 = 0;

    let mut ticker = interval(Duration::from_secs_f64(1.0 / EMIT_HZ));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        ticker.tick().await;
        let frame = synthesize(seq, start.elapsed().as_millis());
        let payload = serde_json::to_string(&frame)?;

        if ws.send(Message::Text(payload)).await.is_err() {
            tracing::info!(%peer, "client disconnected");
            break;
        }
        seq = seq.wrapping_add(1);
    }
    Ok(())
}

/// Produce a deterministic-ish synthetic frame so the UI shows lively motion.
fn synthesize(seq: u64, t_ms: u128) -> TelemetryFrame {
    let t = t_ms as f64 / 1000.0;
    TelemetryFrame {
        seq,
        t_ms,
        roll: 0.35 * (t * 0.7).sin(),
        pitch: 0.20 * (t * 0.5).cos(),
        yaw: (t * 0.2) % std::f64::consts::TAU,
        lat: 47.397_742 + 0.0002 * (t * 0.3).sin(),
        lon: 8.545_594 + 0.0002 * (t * 0.3).cos(),
        alt_m: 10.0 + 2.0 * (t * 0.4).sin(),
        battery_v: 16.8 - (t * 0.01).min(4.0),
        battery_pct: (100.0 - t * 0.5).max(0.0),
        armed: (t as u64 / 5).is_multiple_of(2),
        mode: "HOLD",
        synthetic: true,
    }
}
