# Vyuta — Phased Development Plan

Transform the forked VS Code into a drone development cockpit:

- Real-time MAVLink telemetry dashboard
- Flight-controller firmware build & debug
- Embedded simulation control (PX4 SITL + Gazebo)
- Parameter editor with live tuning
- ULog flight-log analysis
- Companion computer (ROS 2 / MAVSDK) management
- Mission scripting & pre-flight safety checks

**Tech split:** TypeScript (UI) ↔ Rust (sidecars / Neon addons) ↔ gRPC/WebSocket.
**Primary target stack:** PX4 + Gazebo (ArduPilot / jMAVSim / AirSim later).

## Status

| Phase | Title                                           | Status         |
| ----- | ----------------------------------------------- | -------------- |
| 0     | Project scaffold & architecture setup           | ✅ Complete     |
| 1     | MAVLink telemetry engine & real-time dashboard  | ✅ Complete     |
| 2     | Flight-controller firmware integration          | ✅ Complete     |
| 3     | Simulation control panel (SITL + Gazebo)        | ⬜ Not started  |
| 4     | Parameter tuning panel                          | ⬜ Not started  |
| 5     | Flight-log analysis (ULog)                      | ⬜ Not started  |
| 6     | Companion computer & ROS 2 integration          | ⬜ Not started  |
| 7     | Safety, pre-flight checks & mission scripting   | ⬜ Not started  |
| 8     | Polish & simulator-agnostic extensions          | ⬜ Not started  |

---

## Phase 0 — Project Scaffold & Architecture Setup ✅

Monorepo, Rust workspace, extension skeleton, and a verified Rust→TS WebSocket.
Details and verification in [`phase-0.md`](./phase-0.md).

## Phase 1 — MAVLink Telemetry Engine & Real-Time Dashboard ✅

Live MAVLink decode + real-time cockpit (artificial horizon, GPS map, battery,
alarms). Details and verification in [`phase-1.md`](./phase-1.md).

- **`maestros` sidecar:** listen on UDP/TCP, decode `HEARTBEAT`, `ATTITUDE`,
  `GLOBAL_POSITION_INT`, `BATTERY_STATUS`, `VFR_HUD`; serialize with FlatBuffers;
  push over binary WebSocket.
- **TS panel:** Three.js attitude indicator + Leaflet GPS map at 30+ fps;
  battery gauge; armed/mode indicator; loss-of-signal / low-voltage alarms.
- **Test:** `make px4_sitl jmavsim`, telemetry updates < 50 ms latency.

## Phase 2 — Flight Controller Firmware Integration ✅

Build/flash/debug via a probe-rs Neon addon, a `vyuta-probe-rs` DAP debug
adapter, and PX4 build tasks. Details and verification in
[`phase-2.md`](./phase-2.md).

- Wrap `probe-rs` as a Neon addon (GDB/DAP-compatible) in `probe-rs-extension`.
- Register a DAP debug adapter; build-task provider for PX4 airframe/variant
  presets; "Flash Firmware" via `dfu-util`/`probe-rs run`; xterm.js semihosting.
- **Test:** build PX4 for Pixhawk 4, flash, breakpoint in `px4_simple_app.c`.

## Phase 3 — Simulation Control Panel (SITL + Gazebo)

- **`sim-manager` sidecar:** manage PX4 SITL + `gz sim` via `tokio::process`;
  gRPC `StartSim/StopSim/InjectWind/GetStatus`.
- **TS panel:** start/stop, world selector, status; embedded Three.js viewport
  driven by pose/TF; mission REPL of MAVLink commands; wind-tunnel slider.

## Phase 4 — Parameter Tuning Panel

- Extend `maestros` to cache `PARAM_VALUE`, support `PARAM_SET` and diffing.
- React tree view grouped by subsystem; sliders/enums; "Live Tune" toggle;
  save/diff parameter snapshots.

## Phase 5 — Flight Log Analysis (ULog)

- Rust ULog parser (`nom`/`ulog`) → Apache Arrow; serve via Arrow Flight gRPC or
  paginated HTTP; auto-review engine (vibration, innovations, failsafes).
- TS log browser: timeline with mode annotations, side-by-side comparison,
  auto-review checklist with severities (reuse uPlot).

## Phase 6 — Companion Computer & ROS 2 Integration

- **`agent` (drone-side):** tonic gRPC for file sync, `colcon build`, node
  lifecycle, MAVLink-ROS bridge status; ROS node introspection.
- TS panel: node/topic/service tree (mini rqt); one-click "Deploy to Drone";
  SSH terminal; surface ROS 2 topics in the telemetry panel.

## Phase 7 — Safety, Pre-flight Checks & Mission Scripting

- `PreFlightCheck` gRPC over safety params; pre-flight panel gating the "Arm"
  button; visual/audible alarms.
- Notebook API `.mission` files running MAVSDK-Python cells, wired to the 3D
  viewport in real time.

## Phase 8 — Polish & Simulator-Agnostic Extensions

- Rust `SimControl` trait implemented for Gazebo / jMAVSim / AirSim; "Record
  Flight" (`ros2 bag` / MAVLink log); profile telemetry pipeline for >1 kHz;
  per-vehicle configuration profiles (fixed-wing, VTOL, rover).
