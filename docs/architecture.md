# Vyuta Architecture

## High-level shape

```
┌──────────────────────────────────────────────────────────┐
│  Vyuta (VS Code fork)                                      │
│                                                            │
│  ┌────────────────────────┐      in-process (Neon/N-API)  │
│  │ extensions/             │◀─────────────┐                │
│  │   drone-telemetry (TS)  │              │                │
│  │   drone-firmware  (TS)  │      rust/probe-rs-extension  │
│  │   drone-simulation(TS)  │      (debug bridge, Phase 2)  │
│  │   drone-tuning    (TS)  │                               │
│  │   drone-logbook   (TS)  │                               │
│  └───────────┬─────────────┘                               │
│              │ WebSocket (JSON)                             │
│              ▼                                              │
│  ┌────────────────────────┐                                │
│  │ rust/maestros (sidecar) │  MAVLink telemetry + params    │
│  │ rust/sim-manager        │  PX4 SITL + Gazebo control     │
│  │ rust/logbook            │  ULog flight-log analysis      │
│  │ rust/agent  (companion) │  ROS 2 / deploy (drone-side)   │
│  └────────────────────────┘                                │
└──────────────────────────────────────────────────────────┘
```

## Monorepo layout

| Path                          | Language   | Role                                            |
| ----------------------------- | ---------- | ----------------------------------------------- |
| `extensions/drone-telemetry`  | TypeScript | Telemetry cockpit webview                       |
| `extensions/drone-firmware`   | TypeScript | Firmware build/flash/debug (Phase 2)            |
| `extensions/drone-simulation` | TypeScript | Simulation control panel + 3D viewport (Ph. 3)  |
| `extensions/drone-tuning`     | TypeScript | Parameter tuning tree + snapshots (Phase 4)     |
| `extensions/drone-logbook`    | TypeScript | ULog flight-log browser + auto-review (Phase 5)  |
| `rust/maestros`               | Rust       | MAVLink telemetry gateway sidecar               |
| `rust/sim-manager`            | Rust       | PX4 SITL + Gazebo control sidecar (Phase 3)     |
| `rust/logbook`                | Rust       | ULog parser + log-analysis sidecar (Phase 5)    |
| `rust/probe-rs-extension`     | Rust/Neon  | In-process debug bridge (probe-rs, Phase 2)     |
| `rust/agent`                  | Rust       | Companion-computer daemon (ROS 2/deploy, Ph. 6) |
| `docs/`                       | —          | Plan, architecture, per-phase notes             |

The `rust/` directory is a single Cargo workspace so all native crates share a
lockfile, target dir, and dependency versions (`rust/Cargo.toml`).

## TypeScript ↔ Rust transport — "mix per use case"

Two transports are used deliberately, chosen per workload:

1. **Standalone sidecars over WebSocket / gRPC** — for anything *streaming* or
   *process-managing*: telemetry (`maestros`), simulation control, the
   companion `agent`. Sidecars are separate OS processes, which keeps heavy or
   crash-prone native work (USB, sim subprocesses, high-rate decode) out of the
   extension host, and makes them independently testable and restartable.
   - Phase 0 uses a **JSON** WebSocket on `ws://127.0.0.1:9876`.
   - Phase 1 keeps the WebSocket but switches the payload to **FlatBuffers**
     for zero-copy, high-rate (30+ Hz, eventually >1 kHz) telemetry.
   - Phase 3's `sim-manager` (`ws://127.0.0.1:9877`) carries a *bidirectional*
     JSON control + streaming protocol (start/stop/wind/REPL ↑, status/pose/log
     ↓). The plan's gRPC surface is a drop-in upgrade once a `protoc` toolchain
     is available in CI; JSON keeps Phase 3 building offline, mirroring
     `maestros`.
   - Phase 4 makes the `maestros` WebSocket *bidirectional* too: a client that
     sends a parameter command (`request_params`/`set_param`/snapshot ops) gets
     a tagged parameter stream alongside telemetry. Telemetry frames stay
     untyped, so telemetry-only clients are unaffected.

2. **Neon (N-API) addons loaded in-process** — for *low-latency, synchronous*
   call/response that must share the extension host's lifetime, primarily the
   `probe-rs` debug bridge (`probe-rs-extension`). A DAP debug adapter benefits
   from in-process calls rather than another socket hop.

Rule of thumb: **stream → sidecar; tight synchronous call → Neon.**

## Build pipeline

- **Sidecars** (`maestros`, `sim-manager`, `agent`): `cargo build [--release]`
  produces native binaries under `rust/target/`. They are launched by the
  extension as child processes (or run manually during development).
- **Neon addon** (`probe-rs-extension`): `npm run build` in that folder runs
  `cargo build --release` and copies the resulting cdylib to `index.node`,
  which the extension `require()`s through `index.cjs`.
- **Extensions**: `npm run compile` (tsc) in each extension folder. Because the
  extensions live under VS Code's `extensions/` tree, they also participate in
  the fork's built-in extension build.

## Conventions

- Phase 0 code paths are explicitly marked `Phase 0 scaffold` and emit a
  `synthetic` flag so the UI can badge non-real data.
- Each native crate documents, in its `Cargo.toml`, the heavier dependency it
  will gain in a later phase (e.g. `mavlink`, `probe-rs`, `tonic`) and why it is
  deferred — so the scaffold builds quickly and offline.
