# Vyuta Drone Pre-Flight & Safety

A pre-flight safety checklist that **gates arming**, from the **Vyuta** drone IDE.

## Features

- **Pre-flight checklist** evaluated by maestros against live telemetry +
  parameters: telemetry link, battery, GPS/position, attitude level, parameters
  synced, currently disarmed. Re-checked every second.
- **Gated Arm** — the ARM button is enabled only when every check passes; arming
  re-runs the checklist server-side and refuses on any failure.
- **Disarm** any time.
- **Alarms** — a flashing ARMED banner and an audible tone on arm and on a
  safety regression while armed.

## Commands

- **Vyuta: Open Pre-Flight & Safety Panel** (`vyuta.safety.openPanel`)

## Settings

- `vyuta.safety.gatewayUrl` — maestros gateway WebSocket (default `ws://127.0.0.1:9876`)
- `vyuta.safety.audibleAlarms` — play tones (default `true`)

## How it works

maestros owns the telemetry + parameter state and the MAVLink link, so it runs
the checklist and the arm command (`MAV_CMD_COMPONENT_ARM_DISARM`, or a local
arm state in synthetic mode). The panel speaks JSON over the same WebSocket:
`{cmd:"preflight"}`, `{cmd:"arm"}`, `{cmd:"disarm"}` →
`{type:"preflight", ok, items}`, `{type:"arm_ack", ok, armed, message}`.

```sh
cd rust && cargo run --bin maestros   # synthetic telemetry passes pre-flight
```

See [`../../docs/phase-7.md`](../../docs/phase-7.md).
