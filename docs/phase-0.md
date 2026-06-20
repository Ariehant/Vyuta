# Phase 0 — Project Scaffold & Architecture Setup

**Goal:** Stand up the monorepo, Rust toolchains, and VS Code extension
skeleton, and prove Rust → TypeScript communication end to end.

## What this delivers

| Deliverable                                   | Status | Where                                |
| --------------------------------------------- | ------ | ------------------------------------ |
| VS Code fork on a dev branch                  | ✅     | `main` (fork) + this robotics branch |
| Monorepo structure                            | ✅     | `extensions/`, `rust/`, `docs/`      |
| Rust workspace w/ Cargo (tokio, etc.)         | ✅     | `rust/Cargo.toml`                    |
| Rust sidecar compiles & runs                  | ✅     | `rust/maestros`                      |
| Neon native-module build pipeline             | ✅     | `rust/probe-rs-extension`            |
| "Hello World" extension + telemetry webview   | ✅     | `extensions/drone-telemetry`         |
| Rust→TS verified via JSON WebSocket           | ✅     | `maestros` → webview, port `9876`    |

### Deferred dependencies (added in later phases)

To keep the scaffold building quickly and offline, the heavier crates named in
the original plan are declared as TODOs in each crate's `Cargo.toml` rather than
pulled in now:

- `mavlink` → **Phase 1** (real telemetry decode in `maestros`)
- `probe-rs` → **Phase 2** (real probe enumeration / debug in the Neon addon)
- `tonic` / `prost` → **Phase 6** (gRPC in `agent`)
- `ulog` parser → **Phase 5** (new crate)

## Build & run

### 1. Telemetry gateway sidecar

```sh
cd rust
cargo build                 # builds maestros, agent, probe-rs-extension
cargo run --bin maestros    # serves synthetic JSON telemetry on ws://127.0.0.1:9876
```

Override the bind address with `VYUTA_MAESTROS_ADDR=127.0.0.1:9999`.

### 2. Telemetry extension

```sh
cd extensions/drone-telemetry
npm install
npm run compile             # tsc -> out/
```

### 3. Neon debug-bridge addon (optional in Phase 0)

```sh
cd rust/probe-rs-extension
npm run build               # cargo build --release + copy cdylib to index.node
node -e "console.log(require('./index.cjs').hello())"
```

### 4. Open the panel

Launch an Extension Development Host that loads `extensions/drone-telemetry`
(e.g. `code --extensionDevelopmentPath=$(pwd)/extensions/drone-telemetry`),
then run **"Vyuta: Open Telemetry Panel"** from the Command Palette. With the
sidecar running, the status pill turns **connected** and the Attitude /
Position / Battery readouts update at 30 Hz, badged **SYNTHETIC**.

## Verification performed

- `cargo build` for the whole `rust/` workspace — all three crates compile.
- `maestros` launched and a WebSocket client (Node 22 global `WebSocket`)
  received well-formed JSON telemetry frames from `ws://127.0.0.1:9876`.
- `tsc -p ./` compiles the extension to `out/` with no errors.

## Notes / decisions

- The drone extension lives under VS Code's `extensions/` tree per the plan;
  it therefore participates in the fork's built-in extension build. Its
  `node_modules/` and `out/` are covered by the repo's existing `.gitignore`.
- Connection string is configurable via the `vyuta.telemetry.gatewayUrl`
  setting; the webview CSP permits `ws:`/`wss:` connections accordingly.

## Next: Phase 1

Replace the synthetic generator in `maestros` with a real `mavlink` UDP/TCP
decoder, switch the payload to FlatBuffers, and grow the webview into a
Three.js attitude indicator + Leaflet GPS map.
