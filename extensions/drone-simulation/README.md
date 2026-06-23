# Vyuta Drone Simulation

Start, stop, and monitor **PX4-SITL + Gazebo** simulations — with a live 3D
viewport — from the **Vyuta** drone IDE.

## Features

- **Start / Stop / Reset** simulations with world + vehicle pickers, driven by
  the `sim-manager` sidecar (`rust/sim-manager`).
- **3D viewport** — an embedded Three.js scene (ground grid, home pad, a
  quadcopter, and a flight trail) driven by live pose frames. Drag to orbit the
  camera, scroll to zoom.
- **Wind injection** — speed / direction / gust sliders that perturb the
  vehicle live.
- **Mission REPL** — a small command box (`arm`, `takeoff`, `goto x y z`,
  `orbit`, `rtl`, `land`, …) plus one-click verb buttons.
- **Live log console** — simulator stdout/stderr (or sidecar notes) streamed in.
- **Mock flight out of the box** — when no real PX4/Gazebo toolchain is present
  (or you tick *Force mock flight*), the sidecar flies a built-in autopilot so
  the panel and viewport work immediately, badged **MOCK** — the same
  philosophy as the synthetic telemetry source.

## Commands

- **Vyuta: Open Simulation Control Panel** (`vyuta.simulation.openPanel`)

## Settings

- `vyuta.simulation.managerUrl` — sim-manager WebSocket URL (default `ws://127.0.0.1:9877`)
- `vyuta.simulation.defaultWorld` — pre-selected world id (default `default`)
- `vyuta.simulation.defaultVehicle` — pre-selected vehicle id (default `x500`)
- `vyuta.simulation.headless` — start real sims headless (default `true`)
- `vyuta.simulation.forceMock` — always request the built-in mock (default `false`)

## Running the sidecar

The panel is a client of the `sim-manager` sidecar:

```sh
cd rust
cargo run --bin sim-manager            # mock flight if no PX4/gz toolchain

# real SITL (needs PX4-Autopilot + Gazebo `gz` on PATH):
VYUTA_PX4_DIR=/path/to/PX4-Autopilot cargo run --bin sim-manager
```

Then run **Vyuta: Open Simulation Control Panel** and press **Start**.

See [`../../docs/phase-3.md`](../../docs/phase-3.md).
