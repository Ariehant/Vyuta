# Vyuta Drone Companion (ROS 2)

Manage the drone's **companion computer** from the **Vyuta** drone IDE:
ROS 2 introspection, `colcon build`, deploy, and SSH.

## Features

- **ROS 2 graph browser** (mini-rqt) — nodes, topics, and services with types;
  click a topic to **echo** one message.
- **Build** — run `colcon build` (optionally a package subset) with a live log.
- **Deploy to Drone** — rsync the workspace to the companion over SSH.
- **Cancel** a running build/deploy.
- **SSH terminal** — opens an integrated terminal to the companion.
- Works against a real companion (ROS 2 / colcon / rsync) or, on a dev box, a
  **synthetic graph + simulated build/deploy** so the panel works out of the box.

## Commands

- **Vyuta: Open Companion (ROS 2) Panel** (`vyuta.companion.openPanel`)
- **Vyuta: Open Drone SSH Terminal** (`vyuta.companion.openSshTerminal`)

## Settings

- `vyuta.companion.agentUrl` — vyuta-agent WebSocket (default `ws://127.0.0.1:9879`)
- `vyuta.companion.sshHost` — SSH destination, e.g. `pi@drone.local`
- `vyuta.companion.workspace` — colcon workspace to build/deploy
- `vyuta.companion.deployTarget` — rsync target, e.g. `pi@drone.local:/home/pi/ws`

## Running the agent

The `vyuta-agent` daemon runs on the companion computer (or locally for dev):

```sh
cd rust
cargo run --bin vyuta-agent          # synthetic graph if ROS 2 is absent
VYUTA_WS_DIR=~/ros2_ws VYUTA_DEPLOY_TARGET=pi@drone.local:/home/pi/ws \
  cargo run --bin vyuta-agent        # real colcon/rsync when tools exist
```

Then run **Vyuta: Open Companion (ROS 2) Panel**.

See [`../../docs/phase-6.md`](../../docs/phase-6.md).
