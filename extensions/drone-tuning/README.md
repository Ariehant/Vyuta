# Vyuta Drone Parameter Tuning

Live PX4 / ArduPilot **parameter tuning** from the **Vyuta** drone IDE.

## Features

- **Parameter tree** grouped by subsystem (`MC_`, `MPC_`, `EKF2_`, `BAT1_`, …),
  collapsible, with a filter box.
- **Live Tune toggle** — when on, edits are sent (`PARAM_SET`) immediately; when
  off, edits are staged and highlighted until **Apply** (or **Revert**).
- **Snapshots** — save the current values under a name and **Diff** the live
  values against any snapshot (changed / added / removed, highlighted in-tree
  and listed in a diff panel).
- **Load progress** while the full parameter list streams in.
- Works against either a real vehicle/SITL (via the MAVLink link maestros owns)
  or, with no link configured, a seeded synthetic PX4-like parameter set.

## Commands

- **Vyuta: Open Parameter Tuning Panel** (`vyuta.tuning.openPanel`)

## Settings

- `vyuta.tuning.gatewayUrl` — maestros gateway WebSocket (default `ws://127.0.0.1:9876`)
- `vyuta.tuning.liveTune` — default state of the Live Tune toggle (default `false`)

## How it works

The maestros gateway (`rust/maestros`) owns the MAVLink link and the parameter
store. This panel speaks a small JSON command set over the same WebSocket as the
telemetry stream:

- ↑ `request_params`, `set_param`, `refresh_param`, `save_snapshot`,
  `diff_snapshot`, `delete_snapshot`, `list_snapshots`
- ↓ `param_value`, `param_progress`, `param_ack`, `snapshot_list`,
  `snapshot_diff` (telemetry frames carry no `type` and are ignored here)

```sh
cd rust && cargo run --bin maestros          # synthetic params out of the box
#   real vehicle/SITL: VYUTA_MAVLINK_URL=udpin:0.0.0.0:14550 cargo run --bin maestros
```

Then run **Vyuta: Open Parameter Tuning Panel**.

See [`../../docs/phase-4.md`](../../docs/phase-4.md).
