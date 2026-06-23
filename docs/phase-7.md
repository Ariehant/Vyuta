# Phase 7 — Safety, Pre-flight Checks & Mission Scripting

**Goal:** Gate arming behind a pre-flight checklist, and script missions that
fly the simulation.

## What this delivers

| Deliverable                                       | Status | Where                                       |
| ------------------------------------------------- | ------ | ------------------------------------------- |
| Pre-flight check engine over telemetry + params   | ✅     | `rust/maestros/src/preflight.rs`            |
| Gated arm / disarm (`MAV_CMD_COMPONENT_ARM_DISARM`)| ✅     | `maestros` `ws.rs` + `params.rs::arm`       |
| Operator arm override (synthetic)                 | ✅     | `maestros` telemetry `manual_arm`           |
| Pre-flight panel gating the Arm button + alarms   | ✅     | `extensions/drone-safety`                   |
| `.mission` notebook (serializer + controller)     | ✅     | `extensions/drone-mission`                  |
| Mission cells fly the simulation in real time     | ✅     | `missionController.ts` → `sim-manager`      |

### Design notes / decisions

- **Pre-flight lives in maestros.** Checks need live telemetry *and* the
  parameter store, both of which maestros already owns — so the checklist and
  the arm command live there rather than in a new gRPC `PreFlightCheck` service
  (the plan's gRPC is the documented upgrade). Checks: telemetry link, battery
  ≥ 30 %, GPS/position fix, attitude level, parameters synced, currently
  disarmed. **Arm re-runs the checklist server-side** and refuses on any
  failure, so the gate can't be bypassed by a stale UI.

- **Arm path.** On a real link, arming sends `MAV_CMD_COMPONENT_ARM_DISARM`; in
  synthetic mode it sets an operator `manual_arm` override that the synthetic
  generator respects (so the demo is meaningful without a vehicle). A real
  HEARTBEAT remains authoritative.

- **Mission notebooks, not MAVSDK-Python.** The plan calls for MAVSDK-Python
  cells. Rather than embed a Python runtime, a `.mission` notebook (VS Code
  Notebook API) holds markdown + a tiny, safe mission DSL (`arm`, `takeoff`,
  `goto`, `orbit`, `rtl`, `land`, `wait`, …). The controller validates each cell
  and **flies it on the `sim-manager` sidecar** — whose 3D viewport shows the
  flight live, satisfying "wired to the viewport in real time." With no
  WebSocket in the host it falls back to a validating dry-run. The DSL verbs map
  1:1 onto the sim REPL, so the same missions run against real SITL.

- **Alarms.** The safety panel flashes an ARMED banner and beeps on arm and on
  any pre-flight regression while armed (reusing the cockpit's WebAudio tone).

## Protocol additions (panel ↔ maestros)

Client → maestros: `preflight`, `arm`, `disarm`.
maestros → client: `preflight` (ok, items[]), `arm_ack` (ok, armed, message).
(Telemetry frames remain untyped; the safety panel reads `armed` from them.)

## Build & run

```sh
cd rust && cargo run --bin maestros        # pre-flight passes on synthetic data
#   safety panel: "Vyuta: Open Pre-Flight & Safety Panel"

cargo run --bin sim-manager                # for mission notebooks
#   open examples/survey.mission (drone-mission) + the Simulation panel, run cells
```

## Verification performed

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`,
  `cargo test --workspace` all pass — including 3 new `maestros` pre-flight
  tests (passes when healthy; fails on low battery / no GPS / no params; blocks
  when already armed or link down).
- **Safety, end-to-end:** drove maestros from a WebSocket client — pre-flight
  reported all six checks passing at t=0, `arm` succeeded and the telemetry
  stream showed `armed=true`, a re-check then failed the "disarmed" item
  (re-arm blocked), and `disarm` cleared it.
- **Mission, end-to-end:** replayed `survey.mission`'s commands against
  `sim-manager` — the vehicle armed, took off, flew to the box corner (20, 20,
  6 m) into HOLD, and landed (disarmed) — exactly what the notebook controller
  drives. Both extensions compile (`tsc`); `safety.js` passes `node --check`.

> The notebook UI and the panel's live rendering run in a GUI Extension
> Development Host; the maestros + sim-manager paths exercise the safety and
> mission logic headlessly here.

## Next: Phase 8

Polish & simulator-agnostic extensions — a `SimControl` trait for
Gazebo/jMAVSim/AirSim, flight recording, >1 kHz telemetry profiling, and
per-vehicle profiles.
