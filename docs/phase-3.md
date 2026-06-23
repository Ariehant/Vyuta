# Phase 3 — Simulation Control Panel (SITL + Gazebo)

**Goal:** Start, stop, and monitor PX4-SITL + Gazebo simulations from inside the
IDE, with an embedded 3D viewport that shows the vehicle flying.

## What this delivers

| Deliverable                                          | Status | Where                                                |
| ---------------------------------------------------- | ------ | ---------------------------------------------------- |
| `sim-manager` sidecar (process lifecycle)            | ✅     | `rust/sim-manager`                                   |
| Start/Stop/Reset PX4-SITL + Gazebo via `tokio::process` | ✅  | `rust/sim-manager/src/manager.rs`                   |
| World/vehicle catalogue → `make` targets             | ✅     | `rust/sim-manager/src/worlds.rs`                    |
| Control + streaming protocol (JSON/WebSocket)        | ✅     | `rust/sim-manager/src/protocol.rs`, `ws.rs`         |
| Built-in mock flight + mission autopilot             | ✅     | `rust/sim-manager/src/mock.rs`                      |
| Wind injection (live, perturbs the vehicle)          | ✅     | `mock.rs` / `manager.rs` (`set_wind`)              |
| Simulation control panel (start/stop/status/log)     | ✅     | `extensions/drone-simulation`                       |
| Embedded Three.js 3D viewport (pose-driven + trail)  | ✅     | `extensions/.../media/viewport3d.js`               |
| Mission REPL (arm/takeoff/goto/orbit/rtl/land)       | ✅     | panel REPL → `mock.rs::handle_command`             |

### Design notes / decisions

- **Transport — JSON over WebSocket, not gRPC (yet).** The plan calls for a
  gRPC control surface (`StartSim/StopSim/InjectWind/GetStatus`). No `protoc`
  toolchain is available in this environment, so — exactly as Phase 1 chose JSON
  over FlatBuffers (`flatc`) and Phase 2 launched `probe-rs dap-server` instead
  of a hand-rolled stub — `sim-manager` speaks JSON over a WebSocket, mirroring
  the `maestros` telemetry transport. The message set maps 1:1 onto the planned
  gRPC methods (`start`/`stop`/`set_wind`/`status`), so tonic + prost is a
  drop-in upgrade once protoc is in CI (noted in `sim-manager/Cargo.toml`).

- **3D viewport — vendored Three.js**, loaded in the webview via an import map
  and ES modules, with a strict CSP (`script-src ${cspSource} 'nonce-…'`). The
  vehicle is built from primitives (body, four arms, spinning rotors, a red nose
  marker); a `BufferGeometry` line draws the flight trail. A lightweight
  pointer-drag orbit camera avoids vendoring `OrbitControls`. Frame mapping:
  the sidecar streams local **ENU** metres (x=east, y=north, z=up); the viewport
  maps `(x, y, z)_ENU → (x, z, −y)_three` (Three.js is Y-up).

- **Mock flight out of the box.** Gazebo isn't installed here, so a real SITL
  run can't be exercised. `sim-manager` detects the toolchain (PX4 `Makefile` +
  a `gz` binary on `PATH`); when absent — or when *Force mock flight* is ticked
  — it flies a small P-D autopilot so the panel and viewport are immediately
  useful (badged **MOCK**), the same out-of-the-box approach as the synthetic
  telemetry source. A tiny mission REPL steers it and injected wind visibly
  pushes it downwind. The real-process path (spawn `make px4_sitl <target>`,
  stream stdout/stderr, kill on stop) is implemented and unit-/smoke-tested,
  and runs unchanged on a host with the toolchain installed.

- **One teardown path.** A single `stop` one-shot tears down whichever run is
  active — child processes *or* the mock ticker — so both share one code path.

## Protocol summary

Client → server (`{"cmd": …}`): `start` (world, vehicle, headless, mock),
`stop`, `reset`, `set_wind` (speed_mps, direction_deg, gust), `status`,
`send_mavlink` (mission REPL line).

Server → client (`{"type": …}`): `catalog` (worlds/vehicles, on connect),
`status` (phase, mock, wind, sim_time, flight_mode, armed, …, a few Hz),
`pose` (x/y/z, roll/pitch/yaw, velocities, 30 Hz), `log`, `ack`.

## Configuration (sim-manager — environment variables)

| Variable            | Default          | Meaning                                        |
| ------------------- | ---------------- | ---------------------------------------------- |
| `VYUTA_SIM_ADDR`    | `127.0.0.1:9877` | WebSocket bind address                         |
| `VYUTA_PX4_DIR`     | _(unset)_        | PX4-Autopilot source tree (enables real SITL)  |
| `VYUTA_GZ_BIN`      | `gz`             | Gazebo binary name/path used for detection     |
| `VYUTA_SIM_MOCK`    | _(unset)_        | `1`/`true` forces the built-in mock flight     |
| `VYUTA_SIM_WORLD`   | `default`        | Default world id                               |
| `VYUTA_SIM_VEHICLE` | `x500`           | Default vehicle id                             |

Extension settings: `vyuta.simulation.managerUrl`, `defaultWorld`,
`defaultVehicle`, `headless`, `forceMock`.

## Build & run

```sh
# Terminal 1 — the sidecar (mock flight if no PX4/gz toolchain present)
cd rust
cargo run --bin sim-manager
#   real SITL: VYUTA_PX4_DIR=/path/to/PX4-Autopilot cargo run --bin sim-manager

# Optional CLI probe (no IDE needed)
cargo run -p sim-manager --example sim_probe

# Terminal 2 — the extension
cd extensions/drone-simulation && npm install && npm run compile
code --extensionDevelopmentPath="$PWD"   # then: "Vyuta: Open Simulation Control Panel"
```

Press **Start**: the vehicle takes off and orbits. Type `goto 20 0 6` (or use
the verb buttons) in the REPL to fly it around; drag the wind sliders to push it
off station; drag in the viewport to orbit the camera.

## Verification performed

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`,
  `cargo test --workspace` all pass — including 11 `sim-manager` unit tests
  (world→target mapping, takeoff/goto/land transitions, wind drift, rover stays
  grounded, REPL parsing).
- **End-to-end (sidecar):** ran `sim-manager` in mock mode and drove it from a
  WebSocket client — catalogue + status on connect, `start` → takeoff to the
  orbit altitude (~5 m), REPL `goto 25 0 6` reached x≈25 m and switched to
  `HOLD`, `set_wind` reflected in status, `land` + `stop` returned to `idle`
  disarmed, with a steady 30 Hz pose stream and all acks `ok`. The
  `sim_probe` example shows the same takeoff-and-orbit from the CLI.
- **Extension:** `tsc` compiles; the webview ES modules pass `node --check`; the
  vendored Three.js module graph (`three.module.min.js` → `three.core.min.js`)
  loads and exposes the classes the viewport uses (REVISION 184).

> The live WebGL rendering and a real `make px4_sitl gz_x500` run require a GUI
> Extension Development Host and an installed Gazebo/PX4 toolchain respectively,
> which aren't available in this headless environment; the mock path exercises
> the full control/stream/render pipeline end to end.

## Next: Phase 4

Parameter tuning panel — extend `maestros` to cache `PARAM_VALUE`, support
`PARAM_SET` and diffing; a subsystem-grouped tree view with sliders and a
"Live Tune" toggle.
