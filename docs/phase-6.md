# Phase 6 — Companion Computer & ROS 2 Integration

**Goal:** Manage the drone's companion computer from the IDE — browse the ROS 2
graph, build the workspace, and deploy it to the drone.

## What this delivers

| Deliverable                                       | Status | Where                                       |
| ------------------------------------------------- | ------ | ------------------------------------------- |
| Companion agent daemon (WebSocket control plane)  | ✅     | `rust/agent`                                |
| ROS 2 graph introspection (nodes/topics/services) | ✅     | `rust/agent/src/graph.rs` + `manager.rs`    |
| Topic echo (one sample)                           | ✅     | `manager.rs` (`ros2 topic echo --once`)     |
| `colcon build` (with package subset)              | ✅     | `manager.rs`                                |
| Deploy via rsync/SSH                              | ✅     | `manager.rs`                                |
| Synthetic graph + simulated build/deploy          | ✅     | `graph.rs` / `manager.rs`                   |
| mini-rqt graph browser panel                      | ✅     | `extensions/drone-companion`                |
| One-click Build / Deploy + log console            | ✅     | `extensions/.../media/companion.js`         |
| SSH terminal command                              | ✅     | `extensions/.../src/extension.ts`           |

### Design notes / decisions

- **JSON over WebSocket, not tonic gRPC.** The plan calls for a tonic gRPC
  server. With no `protoc` toolchain, the agent speaks JSON over a WebSocket
  like every other Vyuta sidecar (tonic + prost is the documented upgrade — see
  `agent/Cargo.toml`). The agent grew from the Phase 0 heartbeat scaffold into a
  full bidirectional server reusing the `sim-manager` lifecycle pattern (one
  `stop` one-shot tears down whichever task — real child or simulated — is
  running; logs fan out over a broadcast channel).

- **Real tools when present, synthetic otherwise.** The agent detects `ros2`,
  `colcon`, and `rsync` on `PATH`. ROS introspection shells out to
  `ros2 node/topic/service list` and parses the output; build/deploy spawn
  `colcon`/`rsync`. When a tool (or ROS 2 itself) is missing it serves a
  realistic **synthetic** PX4-companion graph and **simulates** build/deploy with
  streamed log lines — so the panel is fully usable on a dev box, the same
  out-of-the-box philosophy as the synthetic telemetry/sim/log paths.

- **SSH stays client-side.** "Deploy to Drone" is rsync (agent-side), but the
  SSH terminal is opened by the *extension* via VS Code's integrated terminal
  (`ssh <host>`), not the agent — the webview just posts a message asking for it.

- **Scope note.** The plan also mentions surfacing ROS 2 topics inside the
  telemetry panel and a full node-lifecycle controller; those are left as
  follow-ups. Introspection + echo + build + deploy + SSH cover the core
  companion workflow.

## Protocol (panel ↔ agent)

Client → agent (`{"cmd": …}`): `graph`, `echo` (topic), `build` (workspace,
packages), `deploy` (source, target), `cancel`, `status`.

Agent → client (`{"type": …}`): `graph` (ros_available, synthetic, nodes,
topics, services), `status` (phase, bridge, workspace, deploy_target, …),
`log`, `echo` (topic, sample), `ack`.

## Configuration (agent — environment variables)

| Variable              | Default          | Meaning                          |
| --------------------- | ---------------- | -------------------------------- |
| `VYUTA_AGENT_ADDR`    | `127.0.0.1:9879` | WebSocket bind address           |
| `VYUTA_ROS2_BIN`      | `ros2`           | ros2 executable                  |
| `VYUTA_COLCON_BIN`    | `colcon`         | colcon executable                |
| `VYUTA_RSYNC_BIN`     | `rsync`          | rsync executable                 |
| `VYUTA_WS_DIR`        | `.`              | colcon workspace directory       |
| `VYUTA_DEPLOY_TARGET` | _(unset)_        | rsync target, `host:path`        |

Extension settings: `vyuta.companion.agentUrl`, `sshHost`, `workspace`,
`deployTarget`.

## Build & run

```sh
cd rust
cargo run --bin vyuta-agent            # synthetic graph + simulated build/deploy
VYUTA_WS_DIR=~/ros2_ws VYUTA_DEPLOY_TARGET=pi@drone.local:/home/pi/ws \
  cargo run --bin vyuta-agent          # real colcon/rsync when present

cd ../extensions/drone-companion && npm install && npm run compile
code --extensionDevelopmentPath="$PWD"   # then: "Vyuta: Open Companion (ROS 2) Panel"
```

## Verification performed

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`,
  `cargo test --workspace` all pass — including 3 `agent` tests (ROS list/typed
  parsing, synthetic graph populated).
- **End-to-end:** ran `vyuta-agent` and drove it from a WebSocket client — the
  synthetic graph returned 6 nodes / 10 topics / 3 services; `echo` returned a
  sample; a simulated `build` transitioned `idle → building → idle` with a
  streamed summary; a simulated `deploy` ran to completion; all acks `ok`.
- **Extension:** `tsc` compiles; `companion.js` passes `node --check`.

> Real ROS 2 introspection and colcon/rsync runs require those tools on the
> companion; the synthetic + simulated paths exercise the full
> command/stream/render pipeline here.

## Next: Phase 7

Safety, pre-flight checks & mission scripting — a pre-flight gate over safety
params and a `.mission` notebook of MAVSDK cells wired to the viewport.
