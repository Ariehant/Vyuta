//! Binary-free JSON WebSocket server that streams [`TelemetryFrame`]s.
//!
//! Each connected client gets its own fixed-rate sender task that snapshots the
//! shared [`TelemetryState`] every `1/emit_hz` seconds. State is behind a
//! `std::sync::Mutex`; the lock is only ever held to clone a snapshot, never
//! across an `.await`.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use futures_util::SinkExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{interval, MissedTickBehavior};
use tokio_tungstenite::tungstenite::Message;

use crate::telemetry::TelemetryState;

pub async fn serve(
    addr: SocketAddr,
    state: Arc<Mutex<TelemetryState>>,
    emit_hz: f64,
    link_timeout: Duration,
) -> Result<()> {
    let listener = TcpListener::bind(addr).await?;
    tracing::info!(%addr, emit_hz, "telemetry WebSocket listening");

    loop {
        let (stream, peer) = listener.accept().await?;
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(err) = handle(stream, peer, state, emit_hz, link_timeout).await {
                tracing::warn!(%peer, error = %err, "connection closed with error");
            }
        });
    }
}

async fn handle(
    stream: TcpStream,
    peer: SocketAddr,
    state: Arc<Mutex<TelemetryState>>,
    emit_hz: f64,
    link_timeout: Duration,
) -> Result<()> {
    let mut ws = tokio_tungstenite::accept_async(stream).await?;
    tracing::info!(%peer, "client connected");

    let mut seq: u64 = 0;
    let mut ticker = interval(Duration::from_secs_f64(1.0 / emit_hz));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        ticker.tick().await;
        let frame = {
            let s = state.lock().expect("telemetry state mutex poisoned");
            s.to_frame(seq, link_timeout)
        };
        let payload = serde_json::to_string(&frame)?;
        if ws.send(Message::Text(payload)).await.is_err() {
            tracing::info!(%peer, "client disconnected");
            break;
        }
        seq = seq.wrapping_add(1);
    }
    Ok(())
}
