//! Request/response WebSocket server for the flight-log browser.
//!
//! The log is offline data, so this is a simple request/response protocol (no
//! periodic streams): the client asks for the overview, specific series
//! (downsampled), the auto-review, or to load a different `.ulg`, and the
//! server replies. On connect the overview + review are pushed immediately.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;

use crate::model::Log;
use crate::{review, synthetic, ulog};

const DEFAULT_MAX_POINTS: usize = 2000;

pub type LogState = Arc<Mutex<Log>>;

#[derive(Debug, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
enum Command {
    Overview,
    Series {
        names: Vec<String>,
        #[serde(default)]
        max_points: Option<usize>,
    },
    Review,
    Load {
        path: String,
    },
    Synthetic,
}

pub async fn serve(addr: SocketAddr, state: LogState) -> Result<()> {
    let listener = TcpListener::bind(addr).await?;
    tracing::info!(%addr, "logbook WebSocket listening");
    loop {
        let (stream, peer) = listener.accept().await?;
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(e) = handle(stream, peer, state).await {
                tracing::warn!(%peer, error = %e, "connection closed with error");
            }
        });
    }
}

async fn handle(stream: TcpStream, peer: SocketAddr, state: LogState) -> Result<()> {
    let mut ws = tokio_tungstenite::accept_async(stream).await?;
    tracing::info!(%peer, "browser connected");

    // Push the current overview + review on connect.
    send(&mut ws, overview_json(&state)).await?;
    send(&mut ws, review_json(&state)).await?;

    while let Some(msg) = ws.next().await {
        let msg = match msg {
            Ok(m) => m,
            Err(_) => break,
        };
        match msg {
            Message::Text(txt) => match serde_json::from_str::<Command>(&txt) {
                Ok(cmd) => handle_command(&mut ws, &state, cmd).await?,
                Err(e) => send(&mut ws, error_json(&format!("bad command: {e}"))).await?,
            },
            Message::Close(_) => break,
            _ => {}
        }
    }
    tracing::info!(%peer, "browser disconnected");
    Ok(())
}

async fn handle_command(
    ws: &mut WebSocketStream<TcpStream>,
    state: &LogState,
    cmd: Command,
) -> Result<()> {
    match cmd {
        Command::Overview => send(ws, overview_json(state)).await,
        Command::Review => send(ws, review_json(state)).await,
        Command::Series { names, max_points } => {
            send(
                ws,
                series_json(state, &names, max_points.unwrap_or(DEFAULT_MAX_POINTS)),
            )
            .await
        }
        Command::Synthetic => {
            {
                let mut g = state.lock().expect("log state poisoned");
                *g = synthetic::synthetic_log();
            }
            send(ws, overview_json(state)).await?;
            send(ws, review_json(state)).await
        }
        Command::Load { path } => match ulog::parse_file(&path) {
            Ok(log) => {
                {
                    let mut g = state.lock().expect("log state poisoned");
                    *g = log;
                }
                send(ws, overview_json(state)).await?;
                send(ws, review_json(state)).await
            }
            Err(e) => send(ws, error_json(&format!("load failed: {e}"))).await,
        },
    }
}

async fn send(ws: &mut WebSocketStream<TcpStream>, payload: String) -> Result<()> {
    ws.send(Message::Text(payload)).await?;
    Ok(())
}

fn overview_json(state: &LogState) -> String {
    let log = state.lock().expect("log state poisoned");
    let series: Vec<Value> = log
        .summaries()
        .into_iter()
        .map(|s| json!({"name": s.name, "count": s.count, "min": s.min, "max": s.max}))
        .collect();
    let modes: Vec<Value> = log
        .modes
        .iter()
        .map(|m| json!({"mode": m.mode, "t0": m.t0, "t1": m.t1}))
        .collect();
    let messages: Vec<Value> = log
        .messages
        .iter()
        .map(|m| json!({"t": m.t, "level": m.level, "text": m.text}))
        .collect();
    let info: Map<String, Value> = log
        .info
        .iter()
        .map(|(k, v)| (k.clone(), Value::String(v.clone())))
        .collect();
    json!({
        "type": "overview",
        "source": log.source,
        "name": log.name,
        "duration_s": log.duration_s,
        "series": series,
        "modes": modes,
        "messages": messages,
        "info": info,
    })
    .to_string()
}

fn series_json(state: &LogState, names: &[String], max_points: usize) -> String {
    let log = state.lock().expect("log state poisoned");
    let mut map = Map::new();
    for name in names {
        if let Some(s) = log.series.get(name) {
            let (t, v) = s.downsample(max_points);
            map.insert(name.clone(), json!({"t": t, "v": v}));
        }
    }
    json!({"type": "series", "series": map}).to_string()
}

fn review_json(state: &LogState) -> String {
    let log = state.lock().expect("log state poisoned");
    let findings: Vec<Value> = review::review(&log)
        .into_iter()
        .map(|f| json!({"id": f.id, "title": f.title, "severity": f.severity, "detail": f.detail}))
        .collect();
    json!({"type": "review", "findings": findings}).to_string()
}

fn error_json(message: &str) -> String {
    json!({"type": "error", "message": message}).to_string()
}
