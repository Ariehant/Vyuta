//! Bidirectional JSON WebSocket server for the companion panel.
//!
//! Per connection: a writer task draining an mpsc queue; on connect the graph +
//! recent logs + status; a log fan-out task; a low-rate status ticker; and a
//! reader that dispatches [`Command`]s.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc};
use tokio::time::{interval, MissedTickBehavior};
use tokio_tungstenite::tungstenite::Message;

use crate::manager::Agent;
use crate::protocol::{Command, LogFrame, Outbound};

const OUT_CAP: usize = 512;
const STATUS_HZ: f64 = 2.0;

pub async fn serve(addr: SocketAddr, agent: Arc<Agent>) -> Result<()> {
    let listener = TcpListener::bind(addr).await?;
    tracing::info!(%addr, "agent WebSocket listening");
    loop {
        let (stream, peer) = listener.accept().await?;
        let agent = agent.clone();
        tokio::spawn(async move {
            if let Err(e) = handle(stream, peer, agent).await {
                tracing::warn!(%peer, error = %e, "connection closed with error");
            }
        });
    }
}

async fn handle(stream: TcpStream, peer: SocketAddr, agent: Arc<Agent>) -> Result<()> {
    let ws = tokio_tungstenite::accept_async(stream).await?;
    tracing::info!(%peer, "panel connected");
    let (mut write, mut read) = ws.split();
    let (out_tx, mut out_rx) = mpsc::channel::<String>(OUT_CAP);

    let writer = tokio::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            if write.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    });

    // On connect: graph + recent logs + status.
    let graph = agent.graph().await;
    let _ = out_tx.send(Outbound::Graph(graph).to_json()).await;
    {
        let snapshot: Vec<String> = {
            let st = agent.state();
            let s = st.lock().expect("state poisoned");
            let mut out: Vec<String> = s
                .recent_logs()
                .map(|l| {
                    Outbound::Log(LogFrame {
                        stream: l.stream,
                        line: l.line.clone(),
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
    let mut log_rx = agent.subscribe_logs();
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

    // Status ticker.
    let st = agent.state();
    let tick_tx = out_tx.clone();
    let ticker_task = tokio::spawn(async move {
        let mut t = interval(Duration::from_secs_f64(1.0 / STATUS_HZ));
        t.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            t.tick().await;
            let frame = { st.lock().expect("state poisoned").status_frame() };
            if tick_tx
                .send(Outbound::Status(frame).to_json())
                .await
                .is_err()
            {
                break;
            }
        }
    });

    // Reader.
    while let Some(msg) = read.next().await {
        let msg = match msg {
            Ok(m) => m,
            Err(_) => break,
        };
        match msg {
            Message::Text(txt) => match serde_json::from_str::<Command>(&txt) {
                Ok(cmd) => handle_command(cmd, &agent, &out_tx).await,
                Err(e) => {
                    let _ = out_tx
                        .send(
                            Outbound::Ack(crate::protocol::AckFrame {
                                cmd: "?".into(),
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

async fn handle_command(cmd: Command, agent: &Arc<Agent>, out_tx: &mpsc::Sender<String>) {
    match cmd {
        Command::Graph => {
            let g = agent.graph().await;
            let _ = out_tx.send(Outbound::Graph(g).to_json()).await;
        }
        Command::Echo { topic } => {
            let e = agent.echo(&topic).await;
            let _ = out_tx.send(Outbound::Echo(e).to_json()).await;
        }
        Command::Build {
            workspace,
            packages,
        } => {
            let ack = agent.build(workspace, packages).await;
            let _ = out_tx.send(Outbound::Ack(ack).to_json()).await;
        }
        Command::Deploy { source, target } => {
            let ack = agent.deploy(source, target).await;
            let _ = out_tx.send(Outbound::Ack(ack).to_json()).await;
        }
        Command::Cancel => {
            let ack = agent.cancel().await;
            let _ = out_tx.send(Outbound::Ack(ack).to_json()).await;
        }
        Command::Status => {
            let frame = { agent.state().lock().expect("state poisoned").status_frame() };
            let _ = out_tx.send(Outbound::Status(frame).to_json()).await;
        }
    }
}
