# Phase 1 — MAVLink Telemetry Engine & Real-Time Dashboard

**Goal:** Decode live MAVLink streams and render attitude, GPS, battery, and
status in real time, with an alarm system.

## What this delivers

| Deliverable                                       | Status | Where                                         |
| ------------------------------------------------- | ------ | --------------------------------------------- |
| MAVLink decode (UDP/TCP) in the gateway           | ✅     | `rust/maestros/src/sources/mavlink_source.rs` |
| HEARTBEAT / ATTITUDE / GLOBAL_POSITION_INT decode | ✅     | same                                          |
| SYS_STATUS / BATTERY_STATUS / VFR_HUD decode      | ✅     | same                                          |
| PX4 flight-mode decoding (+ unit tests)           | ✅     | `rust/maestros/src/px4.rs`                    |
| Link-loss detection (HEARTBEAT staleness)         | ✅     | `rust/maestros/src/telemetry.rs`             |
| Artificial-horizon attitude indicator             | ✅     | `extensions/.../media/attitude.js`            |
| Leaflet GPS map w/ heading marker + trail          | ✅     | `extensions/.../media/map.js`                 |
| Battery gauge + armed/mode indicators              | ✅     | `extensions/.../media/cockpit.js`             |
| Alarm system (low battery / link loss, visual+tone)| ✅     | `extensions/.../media/cockpit.js`             |
| Config UI for connection + alarm thresholds        | ✅     | `extensions/.../package.json`                 |
| MAVLink simulator for testing without PX4          | ✅     | `rust/maestros/examples/mav_sim.rs`           |

### Design notes / decisions

- **Serialization:** frames are sent as JSON over the WebSocket. The plan calls
  for FlatBuffers; at the dashboard's 30 Hz rate JSON is more than adequate, and
  no `flatc` toolchain is available in this environment. FlatBuffers is the
  drop-in zero-copy upgrade for the >1 kHz pipeline targeted in **Phase 8**.
- **Attitude indicator:** rendered with **Canvas 2D** (the conventional flight
  instrument), not Three.js. The plan's Three.js *3D model viewport* is **Phase
  3** ("Simulation Control Panel"); this 2D artificial horizon is the correct
  Phase 1 instrument and carries no 3D asset bloat.
- **Unknown values** are `null` in the frame (`Option<f64>` in Rust) so the UI
  shows "—" instead of a misleading `0`.
- **Synthetic fallback:** with no MAVLink endpoint configured, maestros emits
  synthetic telemetry so the dashboard works out of the box.

## Configuration (maestros — environment variables)

| Variable                | Default            | Meaning                                   |
| ----------------------- | ------------------ | ----------------------------------------- |
| `VYUTA_MAVLINK_URL`     | _(unset)_          | MAVLink endpoint, e.g. `udpin:0.0.0.0:14550`. Unset ⇒ synthetic source. |
| `VYUTA_MAESTROS_ADDR`   | `127.0.0.1:9876`   | WebSocket bind address                    |
| `VYUTA_EMIT_HZ`         | `30`               | UI frame rate                             |
| `VYUTA_LINK_TIMEOUT_MS` | `3000`             | HEARTBEAT staleness ⇒ link-loss           |

Extension settings: `vyuta.telemetry.gatewayUrl`, `batteryWarnPercent`,
`batteryCriticalPercent`, `audibleAlarms`.

## Build & run

```sh
# Terminal 1 — gateway listening for MAVLink over UDP
cd rust
VYUTA_MAVLINK_URL=udpin:0.0.0.0:14550 cargo run --bin maestros

# Terminal 2 — feed it telemetry. Either real PX4 SITL forwarding to
# udp:14550, or the bundled simulator:
cargo run --example mav_sim -- udpout:127.0.0.1:14550

# Terminal 3 — the extension
cd extensions/drone-telemetry && npm install && npm run compile
code --extensionDevelopmentPath="$PWD"   # then: "Vyuta: Open Telemetry Panel"
```

With **PX4 SITL**: `make px4_sitl jmavsim` (or `gz`), which streams MAVLink on
UDP 14550 — point `VYUTA_MAVLINK_URL` at it.

## Verification performed

- `cargo fmt --check`, `cargo clippy --all-targets -D warnings`, `cargo test`
  (3 PX4 mode-decode tests) all pass.
- **End-to-end:** ran `maestros` with `VYUTA_MAVLINK_URL=udpin:127.0.0.1:14550`
  + `mav_sim`, connected a WebSocket client, and confirmed a decoded frame:
  `source=mavlink, link_ok=true, mode=POSCTL, armed=true`, with live attitude,
  GPS (lat/lon/alt), battery (V/%), groundspeed, and heading.
- Extension compiles (`tsc`); webview scripts pass `node --check`.

> Note: the webview's visual rendering (horizon/map) is exercised in the live
> VS Code Extension Development Host; it can't be screenshotted from this
> headless environment.

## Next: Phase 2

Flight-controller firmware build/flash/debug — wrap `probe-rs` in the Neon
addon, register a DAP debug adapter, and add PX4 build tasks.
