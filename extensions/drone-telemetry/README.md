# Vyuta Drone Telemetry

Real-time MAVLink telemetry cockpit for the **Vyuta** drone IDE.

Opens a webview that connects to the `maestros` telemetry gateway over a
WebSocket and renders a live cockpit:

- **Artificial horizon** attitude indicator (Canvas 2D)
- **GPS map** (Leaflet) with a heading-rotated vehicle marker + breadcrumb trail
- **Battery gauge**, armed/flight-mode indicators, air-data readouts
- **Alarm system** — low battery and link-loss raise a visual banner (and an
  optional audible tone)

Data is decoded from real MAVLink by the `maestros` sidecar (PX4 + Gazebo
target); with no link configured it falls back to synthetic telemetry.

## Commands

- **Vyuta: Open Telemetry Panel** (`vyuta.openTelemetryPanel`)

## Settings

- `vyuta.telemetry.gatewayUrl` — WebSocket URL of the gateway sidecar
  (default `ws://127.0.0.1:9876`).

## Develop

```sh
npm install
npm run compile          # or: npm run watch
```

Then launch an Extension Development Host:

```sh
code --extensionDevelopmentPath="$PWD"
```

Run the `maestros` sidecar (`cd ../../rust && cargo run --bin maestros`) and
invoke the command. See [`../../docs/phase-0.md`](../../docs/phase-0.md).
