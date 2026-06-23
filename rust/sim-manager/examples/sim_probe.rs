//! Tiny CLI client for the `sim-manager` sidecar — a development aid.
//!
//! Connects, starts a (mock) simulation, prints a handful of pose frames, then
//! stops and exits. Mirrors the role of `maestros`' `mav_sim` example as a
//! quick way to exercise the sidecar without launching the full IDE panel.
//!
//! Usage:
//!   cargo run -p sim-manager --example sim_probe            # ws://127.0.0.1:9877
//!   cargo run -p sim-manager --example sim_probe -- ws://127.0.0.1:9879

use std::time::Duration;

use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;

#[tokio::main]
async fn main() -> Result<()> {
    let url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "ws://127.0.0.1:9877".to_string());
    println!("connecting to {url} …");
    let (mut ws, _) = tokio_tungstenite::connect_async(&url).await?;

    // Kick off a mock flight; the sidecar auto-takes-off and orbits.
    ws.send(Message::Text(
        r#"{"cmd":"start","vehicle":"x500","world":"default","mock":true}"#.into(),
    ))
    .await?;

    let mut poses = 0u32;
    let deadline = tokio::time::sleep(Duration::from_secs(6));
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            _ = &mut deadline => break,
            msg = ws.next() => {
                let Some(Ok(Message::Text(txt))) = msg else { continue };
                let v: serde_json::Value = match serde_json::from_str(&txt) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                match v.get("type").and_then(|t| t.as_str()) {
                    Some("status") => println!(
                        "status  phase={} mock={} mode={} armed={}",
                        v["phase"], v["mock"], v["flight_mode"], v["armed"]
                    ),
                    Some("pose") => {
                        poses += 1;
                        if poses.is_multiple_of(15) {
                            println!(
                                "pose    x={:7.2} y={:7.2} z={:6.2} yaw={:6.2} mode={}",
                                v["x"].as_f64().unwrap_or(0.0),
                                v["y"].as_f64().unwrap_or(0.0),
                                v["z"].as_f64().unwrap_or(0.0),
                                v["yaw"].as_f64().unwrap_or(0.0),
                                v["flight_mode"]
                            );
                        }
                    }
                    Some("log") => println!("log     {}", v["line"]),
                    _ => {}
                }
            }
        }
    }

    ws.send(Message::Text(r#"{"cmd":"stop"}"#.into())).await?;
    println!("stopped — received {poses} pose frames");
    Ok(())
}
