//! Bidirectional JSON WebSocket server for the simulation control panel.
//!
//! Each connection gets:
//! - a **writer** task draining an mpsc queue to the socket;
//! - on connect, the world/vehicle **catalogue**, recent **logs**, and a
//!   **status** snapshot;
//! - a **log fan-out** task (subscribes to the manager's broadcast);
//! - a **status + pose ticker** (status at a few Hz, pose at 30 Hz);
//! - a **reader** loop that parses [`Command`]s and dispatches them.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc};
use tokio::time::{interval, MissedTickBehavior};
use tokio_tungstenite::tungstenite::Message;

use crate::manager::SimManager;
use crate::protocol::{AckFrame, CatalogFrame, Command, LogFrame, Outbound};
use crate::worlds;

const POSE_HZ: f64 = 30.0;
const STATUS_HZ: f64 = 4.0;
const OUT_CHANNEL_CAP: usize = 512;

pub async fn serve(addr: SocketAddr, manager: Arc<SimManager>) -> Result<()> {
    let listener = TcpListener::bind(addr).await?;
    tracing::info!(%addr, "sim-manager WebSocket listening");
    loop {
        let (stream, peer) = listener.accept().await?;
        let manager = manager.clone();
        tokio::spawn(async move {
            if let Err(e) = handle(stream, peer, manager).await {
                tracing::warn!(%peer, error = %e, "connection closed with error");
            }
        });
    }
}

async fn handle(stream: TcpStream, peer: SocketAddr, manager: Arc<SimManager>) -> Result<()> {
    let ws = tokio_tungstenite::accept_async(stream).await?;
    tracing::info!(%peer, "panel connected");
    let (mut write, mut read) = ws.split();

    let (out_tx, mut out_rx) = mpsc::channel::<String>(OUT_CHANNEL_CAP);

    // Writer: the single owner of the sink.
    let writer = tokio::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            if write.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    });

    // Initial snapshot: catalogue, then recent logs, then status.
    let (worlds_c, vehicles_c) = worlds::catalog();
    let _ = out_tx
        .send(
            Outbound::Catalog(CatalogFrame {
                worlds: worlds_c,
                vehicles: vehicles_c,
                simulators: crate::backend::simulators(),
            })
            .to_json(),
        )
        .await;
    {
        // Snapshot under the lock, then send without holding it (the guard is
        // not Send and must not be held across an `.await`).
        let snapshot: Vec<String> = {
            let state = manager.state();
            let s = state.lock().expect("state poisoned");
            let mut out: Vec<String> = s
                .recent_logs()
                .map(|log| {
                    Outbound::Log(LogFrame {
                        stream: log.stream,
                        line: log.line.clone(),
                    })
                    .to_json()
                })
                .collect();
            out.push(Outbound::Status(s.status_frame()).to_json());
            out
        };
        for msg in snapshot {
            let _ = out_tx.send(msg).await;
        }
    }

    // Log fan-out.
    let mut log_rx = manager.subscribe_logs();
    let log_tx = out_tx.clone();
    let log_task = tokio::spawn(async move {
        loop {
            match log_rx.recv().await {
                Ok(json) => {
                    if log_tx.send(json).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    // Status + pose ticker.
    let state = manager.state();
    let tick_tx = out_tx.clone();
    let ticker_task = tokio::spawn(async move {
        let mut pose_t = interval(Duration::from_secs_f64(1.0 / POSE_HZ));
        pose_t.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let mut status_t = interval(Duration::from_secs_f64(1.0 / STATUS_HZ));
        status_t.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = pose_t.tick() => {
                    let frame = { state.lock().expect("state poisoned").pose_frame(now_ms()) };
                    if tick_tx.send(Outbound::Pose(frame).to_json()).await.is_err() {
                        break;
                    }
                }
                _ = status_t.tick() => {
                    let frame = { state.lock().expect("state poisoned").status_frame() };
                    if tick_tx.send(Outbound::Status(frame).to_json()).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    // Reader: dispatch commands.
    while let Some(msg) = read.next().await {
        let msg = match msg {
            Ok(m) => m,
            Err(_) => break,
        };
        match msg {
            Message::Text(txt) => match serde_json::from_str::<Command>(&txt) {
                Ok(cmd) => {
                    let ack = manager.handle(cmd).await;
                    let _ = out_tx.send(Outbound::Ack(ack).to_json()).await;
                    let frame = {
                        manager
                            .state()
                            .lock()
                            .expect("state poisoned")
                            .status_frame()
                    };
                    let _ = out_tx.send(Outbound::Status(frame).to_json()).await;
                }
                Err(e) => {
                    let _ = out_tx
                        .send(
                            Outbound::Ack(AckFrame {
                                cmd: "?".to_string(),
                                ok: false,
                                message: format!("bad command: {e}"),
                            })
                            .to_json(),
                        )
                        .await;
                }
            },
            Message::Close(_) => break,
            _ => {}
        }
    }

    tracing::info!(%peer, "panel disconnected");
    drop(out_tx);
    log_task.abort();
    ticker_task.abort();
    let _ = writer.await;
    Ok(())
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}
