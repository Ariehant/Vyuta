# Vyuta Documentation

**Vyuta** is a fork of VS Code being built into a drone / robotics development
cockpit. See [`../FORK.md`](../FORK.md) for fork provenance.

## Contents

- [`architecture.md`](./architecture.md) — monorepo layout, the TypeScript ↔
  Rust transport split, and the decisions behind them.
- [`plan.md`](./plan.md) — the full phased development plan (Phases 0–8) and
  current status.
- [`phase-0.md`](./phase-0.md) — what the Phase 0 scaffold delivers and how to
  build, run, and verify it.

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
