//! Bidirectional JSON WebSocket server.
//!
//! Every client receives the fixed-rate [`TelemetryFrame`] stream (Phase 1).
//! Clients that send a parameter command (Phase 4) additionally get a parameter
//! sync stream: `param_value` / `param_progress` / `param_ack` / `snapshot_*`
//! messages, all tagged with a `type` field. Telemetry frames carry no `type`,
//! so a telemetry-only client (which never sends commands) is unaffected and
//! the tuning panel simply ignores untyped frames.
//!
//! State lives behind `std::sync::Mutex`es; locks are only ever held to take a
//! snapshot, never across an `.await`.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::json;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::{interval, MissedTickBehavior};
use tokio_tungstenite::tungstenite::Message;

use crate::params::{ParamService, ParamStore};
use crate::preflight;
use crate::telemetry::TelemetryState;

const OUT_CAP: usize = 1024;
const PARAM_SYNC_HZ: f64 = 10.0;

#[derive(Debug, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
enum ClientCommand {
    RequestParams,
    SetParam { id: String, value: f64 },
    RefreshParam { id: String },
    SaveSnapshot { name: String },
    DiffSnapshot { name: String },
    DeleteSnapshot { name: String },
    ListSnapshots,
    Preflight,
    Arm,
    Disarm,
}

pub async fn serve(
    addr: SocketAddr,
    state: Arc<Mutex<TelemetryState>>,
    params: Arc<ParamService>,
    emit_hz: f64,
    link_timeout: Duration,
) -> Result<()> {
    let listener = TcpListener::bind(addr).await?;
    tracing::info!(%addr, emit_hz, "telemetry/param WebSocket listening");

    loop {
        let (stream, peer) = listener.accept().await?;
        let state = state.clone();
        let params = params.clone();
        tokio::spawn(async move {
            if let Err(err) = handle(stream, peer, state, params, emit_hz, link_timeout).await {
                tracing::warn!(%peer, error = %err, "connection closed with error");
            }
        });
    }
}

async fn handle(
    stream: TcpStream,
    peer: SocketAddr,
    state: Arc<Mutex<TelemetryState>>,
    params: Arc<ParamService>,
    emit_hz: f64,
    link_timeout: Duration,
) -> Result<()> {
    let ws = tokio_tungstenite::accept_async(stream).await?;
    tracing::info!(%peer, "client connected");
    let (mut write, mut read) = ws.split();

    let (out_tx, mut out_rx) = mpsc::channel::<String>(OUT_CAP);

    let writer = tokio::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            if write.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    });

    // Telemetry stream (unchanged behaviour, now via the out queue).
    let tele_tx = out_tx.clone();
    let tele_state = state.clone();
    let ticker_task = tokio::spawn(async move {
        let mut seq: u64 = 0;
        let mut ticker = interval(Duration::from_secs_f64(1.0 / emit_hz));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            let payload = {
                let s = tele_state.lock().expect("telemetry state mutex poisoned");
                serde_json::to_string(&s.to_frame(seq, link_timeout))
            };
            match payload {
                Ok(p) => {
                    if tele_tx.send(p).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
            seq = seq.wrapping_add(1);
        }
    });

    // Param sync starts lazily on the first parameter command.
    let mut param_task: Option<JoinHandle<()>> = None;

    while let Some(msg) = read.next().await {
        match msg {
            Ok(Message::Text(txt)) => {
                if let Ok(cmd) = serde_json::from_str::<ClientCommand>(&txt) {
                    if param_task.is_none() {
                        param_task = Some(spawn_param_sync(params.store.clone(), out_tx.clone()));
                    }
                    handle_command(cmd, &params, &state, link_timeout, &out_tx).await;
                }
                // Non-command text is ignored.
            }
            Ok(Message::Close(_)) => break,
            Ok(_) => {}
            Err(_) => break,
        }
    }

    tracing::info!(%peer, "client disconnected");
    drop(out_tx);
    ticker_task.abort();
    if let Some(t) = param_task {
        t.abort();
    }
    let _ = writer.await;
    Ok(())
}

async fn handle_command(
    cmd: ClientCommand,
    params: &Arc<ParamService>,
    state: &Arc<Mutex<TelemetryState>>,
    link_timeout: Duration,
    out_tx: &mpsc::Sender<String>,
) {
    match cmd {
        ClientCommand::RequestParams => {
            params.request_list();
            let (received, total) = {
                let s = params.store.lock().expect("param store poisoned");
                (s.received(), s.total())
            };
            let _ = out_tx
                .send(
                    json!({"type": "param_progress", "received": received, "total": total})
                        .to_string(),
                )
                .await;
        }
        ClientCommand::SetParam { id, value } => {
            let ok = params.set_param(&id, value);
            let msg = if ok { "set" } else { "unknown parameter" };
            let _ = out_tx
                .send(
                    json!({"type":"param_ack","id":id,"ok":ok,"value":value,"message":msg})
                        .to_string(),
                )
                .await;
        }
        ClientCommand::RefreshParam { id } => {
            params.refresh_param(&id);
        }
        ClientCommand::SaveSnapshot { name } => {
            let count = params
                .store
                .lock()
                .expect("param store poisoned")
                .save_snapshot(name.clone());
            let _ = out_tx
                .send(json!({"type":"snapshot_saved","name":name,"count":count}).to_string())
                .await;
            send_snapshot_list(params, out_tx).await;
        }
        ClientCommand::DiffSnapshot { name } => {
            let diff = params
                .store
                .lock()
                .expect("param store poisoned")
                .diff(&name);
            match diff {
                Some(entries) => {
                    let list: Vec<_> = entries
                        .into_iter()
                        .map(|d| json!({"id": d.id, "from": d.from, "to": d.to, "kind": d.kind}))
                        .collect();
                    let _ = out_tx
                        .send(
                            json!({"type":"snapshot_diff","name":name,"entries":list}).to_string(),
                        )
                        .await;
                }
                None => {
                    let _ = out_tx
                        .send(json!({"type":"param_ack","id":"","ok":false,"message":format!("no snapshot '{name}'")}).to_string())
                        .await;
                }
            }
        }
        ClientCommand::DeleteSnapshot { name } => {
            params
                .store
                .lock()
                .expect("param store poisoned")
                .delete_snapshot(&name);
            send_snapshot_list(params, out_tx).await;
        }
        ClientCommand::ListSnapshots => send_snapshot_list(params, out_tx).await,
        ClientCommand::Preflight => {
            let _ = out_tx
                .send(preflight_json(params, state, link_timeout))
                .await;
        }
        ClientCommand::Arm => {
            let _ = out_tx.send(do_arm(params, state, link_timeout)).await;
        }
        ClientCommand::Disarm => {
            let sent = params.arm(false);
            if !sent {
                let mut s = state.lock().expect("telemetry state mutex poisoned");
                s.manual_arm = Some(false);
                s.armed = false;
            }
            let _ = out_tx
                .send(
                    json!({"type":"arm_ack","ok":true,"armed":false,"message":"disarmed"})
                        .to_string(),
                )
                .await;
        }
    }
}

/// Evaluate the pre-flight checklist into a JSON frame.
fn preflight_json(
    params: &Arc<ParamService>,
    state: &Arc<Mutex<TelemetryState>>,
    link_timeout: Duration,
) -> String {
    let s = state.lock().expect("telemetry state mutex poisoned");
    let store = params.store.lock().expect("param store poisoned");
    let (ok, items) = preflight::evaluate(&s, &store, s.link_ok(link_timeout));
    let items: Vec<_> = items
        .into_iter()
        .map(|i| json!({"id": i.id, "label": i.label, "pass": i.pass, "detail": i.detail}))
        .collect();
    json!({"type": "preflight", "ok": ok, "items": items}).to_string()
}

/// Run pre-flight and, if it passes, arm the vehicle.
fn do_arm(
    params: &Arc<ParamService>,
    state: &Arc<Mutex<TelemetryState>>,
    link_timeout: Duration,
) -> String {
    let (ok, fail_reason) = {
        let s = state.lock().expect("telemetry state mutex poisoned");
        let store = params.store.lock().expect("param store poisoned");
        let (ok, items) = preflight::evaluate(&s, &store, s.link_ok(link_timeout));
        let reason = items
            .iter()
            .find(|i| !i.pass)
            .map(|i| format!("{}: {}", i.label, i.detail))
            .unwrap_or_default();
        (ok, reason)
    };
    if !ok {
        return json!({"type":"arm_ack","ok":false,"armed":false,"message":format!("pre-flight failed — {fail_reason}")}).to_string();
    }
    let sent = params.arm(true);
    if !sent {
        let mut s = state.lock().expect("telemetry state mutex poisoned");
        s.manual_arm = Some(true);
        s.armed = true;
    }
    json!({"type":"arm_ack","ok":true,"armed":true,"message":"armed"}).to_string()
}

async fn send_snapshot_list(params: &Arc<ParamService>, out_tx: &mpsc::Sender<String>) {
    let names = params
        .store
        .lock()
        .expect("param store poisoned")
        .snapshot_names();
    let _ = out_tx
        .send(json!({"type": "snapshot_list", "names": names}).to_string())
        .await;
}

/// Per-connection task that forwards parameter changes (deltas) and load
/// progress to the client at a low rate.
fn spawn_param_sync(store: Arc<Mutex<ParamStore>>, out_tx: mpsc::Sender<String>) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut last_ver: u64 = 0;
        let mut ticker = interval(Duration::from_secs_f64(1.0 / PARAM_SYNC_HZ));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            let (changed, total, received, ver) = {
                let s = store.lock().expect("param store poisoned");
                (
                    s.changed_since(last_ver),
                    s.total(),
                    s.received(),
                    s.current_version(),
                )
            };
            if changed.is_empty() {
                continue;
            }
            for p in &changed {
                let m = json!({
                    "type": "param_value",
                    "id": p.id,
                    "value": p.value,
                    "param_type": p.ptype,
                    "index": p.index,
                    "count": total,
                });
                if out_tx.send(m.to_string()).await.is_err() {
                    return;
                }
            }
            let progress = json!({"type": "param_progress", "received": received, "total": total});
            if out_tx.send(progress.to_string()).await.is_err() {
                return;
            }
            last_ver = ver;
        }
    })
}
