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
| 3     | Simulation control panel (SITL + Gazebo)        | ✅ Complete     |
| 4     | Parameter tuning panel                          | ✅ Complete     |
| 5     | Flight-log analysis (ULog)                      | ✅ Complete     |
| 6     | Companion computer & ROS 2 integration          | ✅ Complete     |
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

## Phase 3 — Simulation Control Panel (SITL + Gazebo) ✅

`sim-manager` sidecar driving PX4 SITL + Gazebo, with a start/stop/monitor panel
and an embedded Three.js 3D viewport. Details and verification in
[`phase-3.md`](./phase-3.md).

- **`sim-manager` sidecar:** manage PX4 SITL + `gz sim` via `tokio::process`;
  start/stop/inject-wind/status as JSON over WebSocket (gRPC is the documented
  upgrade once protoc is available — see phase-3 notes). Built-in mock flight +
  mission autopilot so the panel works with no toolchain installed.
- **TS panel:** start/stop/reset, world + vehicle pickers, status + log console;
  embedded Three.js viewport driven by pose (with a flight trail); mission REPL
  (`arm`/`takeoff`/`goto`/`orbit`/`rtl`/`land`); wind speed/direction/gust sliders.

## Phase 4 — Parameter Tuning Panel ✅

`maestros` caches `PARAM_VALUE`, reads/writes parameters over the live MAVLink
link, and serves a subsystem-grouped tuning panel with snapshots + diff. Details
and verification in [`phase-4.md`](./phase-4.md).

- **`maestros`:** parameter store + `PARAM_REQUEST_LIST`/`PARAM_SET`/
  `PARAM_REQUEST_READ`; a bidirectional WebSocket protocol (telemetry stream is
  unchanged); snapshots + diffing; a synthetic param set when no link is present.
- **TS panel (`drone-tuning`):** subsystem-grouped tree with a filter; "Live
  Tune" toggle (immediate vs staged + Apply/Revert); save/diff snapshots with
  in-tree highlighting. (Vanilla DOM tree rather than React — no bundler in the
  fork's extension build.)

## Phase 5 — Flight Log Analysis (ULog) ✅

`logbook` sidecar parses PX4 ULog into time series and serves them + an
auto-review to a browser panel. Details and verification in
[`phase-5.md`](./phase-5.md).

- **`logbook` sidecar:** hand-rolled ULog parser → named series; min/max
  downsampling; auto-review (vibration, failsafe, battery, altitude, modes);
  request/response JSON over WebSocket (Arrow/Flight is the documented upgrade);
  a synthetic flight log when no `.ulg` is given.
- **TS browser (`drone-logbook`):** flight-mode timeline, field picker + stacked
  uPlot charts (mode bands, synced cursors), auto-review checklist, logged
  messages. (Vendored uPlot; single-log view — two-log compare deferred.)

## Phase 6 — Companion Computer & ROS 2 Integration ✅

`agent` companion daemon does ROS 2 introspection, `colcon build`, and rsync
deploy; the `drone-companion` panel is a mini-rqt browser with build/deploy/SSH.
Details and verification in [`phase-6.md`](./phase-6.md).

- **`agent` (drone-side):** ROS 2 graph introspection, topic echo, `colcon
  build`, rsync deploy, MAVLink-ROS bridge status — JSON over WebSocket (tonic
  gRPC is the documented upgrade); synthetic graph + simulated build/deploy when
  the tools/ROS are absent.
- **TS panel (`drone-companion`):** node/topic/service tree (mini-rqt) with
  click-to-echo; one-click Build / "Deploy to Drone" with a log console; SSH
  terminal. (Surfacing ROS topics in the telemetry panel + node lifecycle are
  follow-ups.)

## Phase 7 — Safety, Pre-flight Checks & Mission Scripting

- `PreFlightCheck` gRPC over safety params; pre-flight panel gating the "Arm"
  button; visual/audible alarms.
- Notebook API `.mission` files running MAVSDK-Python cells, wired to the 3D
  viewport in real time.

## Phase 8 — Polish & Simulator-Agnostic Extensions

- Rust `SimControl` trait implemented for Gazebo / jMAVSim / AirSim; "Record
  Flight" (`ros2 bag` / MAVLink log); profile telemetry pipeline for >1 kHz;
  per-vehicle configuration profiles (fixed-wing, VTOL, rover).
