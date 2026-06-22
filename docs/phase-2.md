# Phase 2 — Flight Controller Firmware Integration

**Goal:** Build, flash, and debug PX4/ArduPilot firmware directly from the IDE.

## What this delivers

| Deliverable                                        | Status | Where                                            |
| -------------------------------------------------- | ------ | ------------------------------------------------ |
| probe-rs Neon addon (real probe enumeration)       | ✅     | `rust/probe-rs-extension`                        |
| Chip-family/target listing for pickers             | ✅     | `rust/probe-rs-extension` (`listChipFamilies`)   |
| PX4 build-task provider (airframes + boards)       | ✅     | `extensions/drone-firmware/src/taskProvider.ts`  |
| `vyuta-probe-rs` debug adapter (DAP)               | ✅     | `extensions/drone-firmware/src/debugAdapter.ts`  |
| Flash command (make upload / probe-rs / dfu-util)  | ✅     | `extensions/drone-firmware/src/commands.ts`      |
| List Debug Probes command                          | ✅     | same                                             |
| RTT / semihosting terminal                         | ✅     | same                                             |

### Design notes / decisions

- **probe-rs 0.31** compiles in this environment with no `libudev` system
  dependency (it uses pure-Rust USB). The Neon addon exposes synchronous
  `hello()`, `listProbes()`, and `listChipFamilies()` — verified in-process
  (returns `[]` probes here since no hardware is attached, and 225 chip
  families).
- **Debug adapter:** instead of hand-rolling a GDB stub through Neon (as the
  original plan sketched), the `vyuta-probe-rs` debug type launches
  **`probe-rs dap-server`** and connects VS Code to it over TCP. This is the
  same approach the upstream probe-rs VS Code extension uses, and it gives
  breakpoints/stepping/registers/RTT for free. The Neon addon is retained for
  fast synchronous probe/target queries that back the UI.
- **Semihosting/RTT:** surfaced via `probe-rs attach` in an integrated
  terminal. A dedicated xterm.js webview pane is deferred as polish.
- **Separate extension:** firmware/debug lives in `extensions/drone-firmware`,
  distinct from `drone-telemetry`, matching the plan's "debug adapter
  extension". CI and the SessionStart hook now build/install every
  `extensions/drone-*` package.

## Commands & contributions

- Commands: **Build Firmware…**, **Flash Firmware…**, **List Debug Probes**,
  **Open RTT / Semihosting Terminal**.
- Task type `px4` with presets (Gazebo x500, Standard VTOL, jMAVSim; Pixhawk
  6X/6C/4) and custom `tasks.json` support.
- Debug type `vyuta-probe-rs` (launch; `chip`, `program`, `speed`,
  `connectUnderReset`).
- Settings: `vyuta.firmware.px4Dir`, `probeRsPath`, `dfuUtilPath`.

## Prerequisites (runtime, on the user's machine)

- [`probe-rs`](https://probe.rs) on `PATH` (debug/flash/RTT).
- Native addon built once: `cd rust/probe-rs-extension && npm run build`.
- PX4-Autopilot toolchain + `make` for builds; `dfu-util` for DFU flashing.

## Build & verify

```sh
cd rust && cargo build --workspace          # builds maestros, agent, addon
cd probe-rs-extension && npm run build       # produces index.node
node -e "console.log(require('./index.cjs').listProbes())"   # => []

cd ../../extensions/drone-firmware && npm install && npm run compile
```

## Verification performed

- `cargo fmt --check`, `cargo clippy --all-targets -D warnings`, `cargo test`
  all pass (probe-rs addon included).
- Built `index.node`; the firmware extension's addon loader resolves it and
  returned `hello`, `listProbes()` (`[]`), and 225 chip families.
- `drone-firmware` compiles with `tsc`.

> Hardware-dependent paths (actual flashing, live debug sessions, RTT output)
> require a probe + board and can't be exercised in this headless environment;
> the logic compiles and the DAP server is launched exactly as upstream does.

## Next: Phase 3

Simulation control panel — a `sim-manager` sidecar (PX4 SITL + Gazebo via
gRPC) and a panel to start/stop/monitor sims with an embedded 3D viewport.
