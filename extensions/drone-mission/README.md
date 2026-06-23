# Vyuta Drone Mission Notebooks

Author and run **`.mission`** notebooks that fly the **Vyuta** simulation in
real time.

## Features

- **`.mission` notebooks** — markdown + code cells (VS Code Notebook API).
- **Run cells** — each code cell's commands are validated and flown on the
  `sim-manager` sidecar; the simulation panel's 3D viewport shows the flight
  live. Per-step acknowledgements are written to the cell output.
- **Dry-run fallback** — if the host has no WebSocket, cells validate and print
  the plan instead of flying.

## Mission commands

`arm` · `disarm` · `takeoff [alt]` · `goto x y [z]` · `orbit [r] [alt]` ·
`hold` · `rtl` · `land` · `wait <seconds>` · `# comment`

## Commands

- **Vyuta: New Mission Notebook** (`vyuta.mission.new`) — scaffold a sample.
- Open any `*.mission` file to edit/run it.

## Settings

- `vyuta.mission.simUrl` — sim-manager WebSocket (default `ws://127.0.0.1:9877`)
- `vyuta.mission.stepPauseMs` — pause after each command (default `600`)

## Usage

```sh
cd rust && cargo run --bin sim-manager     # start the simulation
```

Open `examples/survey.mission` (or run **Vyuta: New Mission Notebook**), open the
Simulation Control Panel to watch, then run the cells.

See [`../../docs/phase-7.md`](../../docs/phase-7.md).
