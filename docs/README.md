# Vyuta Documentation

**Vyuta** is a fork of VS Code being built into a drone / robotics development
cockpit. See [`../FORK.md`](../FORK.md) for fork provenance.

## Contents

- [`architecture.md`](./architecture.md) — monorepo layout, the TypeScript ↔
  Rust transport split, and the decisions behind them.
- [`plan.md`](./plan.md) — the full phased development plan (Phases 0–8) and
  current status.
- [`phase-0.md`](./phase-0.md) — scaffold: monorepo, Rust workspace, extension.
- [`phase-1.md`](./phase-1.md) — MAVLink telemetry engine + real-time cockpit.
- [`phase-2.md`](./phase-2.md) — firmware build/flash/debug (probe-rs + DAP).
- [`phase-3.md`](./phase-3.md) — simulation control panel + 3D viewport
  (PX4 SITL + Gazebo via the `sim-manager` sidecar).
- [`phase-4.md`](./phase-4.md) — parameter tuning panel (live `PARAM_SET`,
  snapshots + diff) over the `maestros` gateway.
- [`phase-5.md`](./phase-5.md) — flight-log analysis: a ULog parser + the
  `logbook` sidecar, with a mode-annotated browser and auto-review.
- [`phase-6.md`](./phase-6.md) — companion computer & ROS 2: the `agent` daemon
  (introspection, colcon build, deploy) and a mini-rqt panel.

## Quick start (Phase 0)

```sh
# 1. Build + run the telemetry gateway sidecar (synthetic JSON on :9876)
cd rust && cargo run --bin maestros

# 2. In another shell, build the telemetry extension
cd extensions/drone-telemetry && npm install && npm run compile

# 3. Launch an Extension Development Host pointed at the extension, then run
#    the command:  "Vyuta: Open Telemetry Panel"
```

The panel connects to the sidecar and shows a live readout updating at 30 Hz.
Full verification steps are in [`phase-0.md`](./phase-0.md).
