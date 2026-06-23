# Phase 8 — Polish & Simulator-Agnostic Extensions

**Goal:** Make the stack simulator-agnostic, add flight recording, profile the
telemetry pipeline, and surface per-vehicle profiles.

## What this delivers

| Deliverable                                       | Status | Where                                       |
| ------------------------------------------------- | ------ | ------------------------------------------- |
| `SimControl` trait + Gazebo/jMAVSim/AirSim backends | ✅   | `rust/sim-manager/src/backend.rs`           |
| Simulator picker in the simulation panel          | ✅     | `extensions/drone-simulation`               |
| Per-vehicle profiles (class) in the catalogue     | ✅     | `sim-manager/src/worlds.rs` + panel         |
| Flight recording (telemetry → tlog JSONL)         | ✅     | `rust/maestros/src/recorder.rs`             |
| Record button in the telemetry cockpit            | ✅     | `extensions/drone-telemetry`                |
| >1 kHz telemetry pipeline benchmark               | ✅     | `maestros --bench`                          |

### Design notes / decisions

- **`SimControl` trait.** The sim-manager's launch logic moved behind a
  `SimControl` trait with `Gazebo`, `Jmavsim`, and `Airsim` implementations.
  Each knows how to *detect* its simulator and produce a `LaunchSpec`
  (program + args + cwd + env). The manager picks one by id; if it isn't
  available the built-in mock flight runs (unchanged), so the panel always
  works. The `simulator` field flows through Start → status → the panel's
  picker.

- **Per-vehicle profiles.** Each vehicle now carries a `class` (multirotor /
  vtol / fixedwing / rover), sent in the catalogue and shown next to the vehicle
  picker — the hook for per-vehicle tuning/profile defaults.

- **Flight recording.** maestros records the live telemetry to a
  newline-delimited JSON "tlog" (`vyuta-<epoch>.tlog.jsonl`) on a dedicated task
  at the emit rate; `record_start` / `record_stop` drive it from the cockpit's
  Record button. (`ros2 bag` recording for ROS topics lives with the companion
  agent; this is the MAVLink-telemetry side.) The JSONL is trivially
  re-ingestible for analysis.

- **>1 kHz profiling.** `maestros --bench [frames]` times frame build +
  JSON-serialize in a tight loop. Measured ~37 k Hz here — comfortably past the
  >1 kHz target — confirming JSON has ample headroom and that FlatBuffers (the
  Phase 1 note) remains a *future* optimisation, not a requirement.

- **Scope note.** AirSim can't be reliably auto-detected (its binary varies per
  project), so it reports unavailable and falls back to mock; the launch spec is
  in place for hosts that have it. "Record Flight" covers the telemetry tlog;
  per-vehicle *parameter* profiles build on the Phase 4 snapshot mechanism.

## Protocol additions

- sim-manager: `Start.simulator`; `Catalog.simulators[]`; catalogue entries gain
  `class`; `Status.simulator`.
- maestros: `record_start` / `record_stop` → `record_ack` (ok, recording, path,
  frames).

## Build & run

```sh
cd rust
cargo run --bin maestros -- --bench 200000      # pipeline throughput (>1 kHz)
cargo run --bin maestros                          # cockpit Record button records a tlog
VYUTA_RECORD_DIR=~/flights cargo run --bin maestros

cargo run --bin sim-manager                       # simulation panel: pick Gazebo/jMAVSim/AirSim
```

## Verification performed

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`,
  `cargo test --workspace` all pass — including 3 new `sim-manager` backend
  tests (Gazebo make-target args, jMAVSim quad-only, default-to-Gazebo + 3
  simulators).
- **Recording, end-to-end:** drove maestros from a WebSocket client —
  `record_start` then `record_stop` reported `frames=36` and wrote a
  `vyuta-*.tlog.jsonl` whose lines are well-formed telemetry frames (~30 Hz over
  ~1.2 s).
- **Catalogue, end-to-end:** sim-manager's catalogue exposed 3 simulators and
  `standard_vtol`'s class as `vtol`.
- **Benchmark:** `maestros --bench 300000` → ~37 k Hz, **PASS** (> 1 kHz).
- All eight extensions compile (`tsc`); `sim.js` and `cockpit.js` pass
  `node --check`.

> Live rendering, real Gazebo/jMAVSim launches, and AirSim run in a GUI / on a
> host with those tools; the backends, catalogue, recorder, and benchmark are
> exercised headlessly here.

## The plan is complete

Phases 0–8 are done. Vyuta is a drone-development cockpit: live MAVLink
telemetry, firmware build/flash/debug, simulation control with a 3D viewport,
live parameter tuning, ULog flight-log analysis, ROS 2 companion management,
pre-flight safety gating, mission notebooks, and a simulator-agnostic,
recordable, >1 kHz-capable backbone — all building offline with synthetic
fallbacks so every panel works out of the box.
