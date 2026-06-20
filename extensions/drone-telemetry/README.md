# Vyuta Drone Telemetry

Real-time MAVLink telemetry cockpit for the **Vyuta** drone IDE.

**Phase 0 scaffold:** opens a webview that connects to the `maestros` telemetry
gateway sidecar over a JSON WebSocket and shows a live attitude / position /
battery readout. Phase 1 grows this into a Three.js attitude indicator and a
Leaflet GPS map fed by FlatBuffers.

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
