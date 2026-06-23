# Vyuta Drone Flight Log Analyzer

Browse PX4 **ULog** flight logs from the **Vyuta** drone IDE.

## Features

- **Auto-review checklist** — vibration, failsafe activations, battery, altitude,
  mode changes, and logged warnings, each with a severity.
- **Flight-mode timeline** — a colour-coded bar of `nav_state` spans with a
  legend; modes are also drawn as background bands on every chart.
- **Field browser + plots** — every numeric field of every logged message is
  available (arrays expanded to `field[i]`); pick any to plot as stacked uPlot
  charts with synced cursors.
- **Logged messages** — the log's text messages (`L`) with timestamps.
- **Loads any `.ulg`** or falls back to a built-in synthetic flight so the panel
  works out of the box.

## Commands

- **Vyuta: Open Flight Log Analyzer** (`vyuta.logbook.openPanel`)

## Settings

- `vyuta.logbook.serverUrl` — logbook sidecar WebSocket (default `ws://127.0.0.1:9878`)

## Running the sidecar

```sh
cd rust
cargo run --bin logbook                       # serves a synthetic flight log
VYUTA_ULOG_PATH=/path/to/flight.ulg cargo run --bin logbook   # load a real log
# write a sample log to disk:
cargo run --bin logbook -- --write-ulog sample.ulg
```

Then run **Vyuta: Open Flight Log Analyzer**, or type a `.ulg` path in the panel
and press **Load**.

See [`../../docs/phase-5.md`](../../docs/phase-5.md).
