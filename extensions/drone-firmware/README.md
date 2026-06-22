# Vyuta Drone Firmware

Build, flash, and debug flight-controller firmware (PX4 + Gazebo target) from
the **Vyuta** drone IDE.

## Features

- **PX4 build tasks** — preset `make` targets for SITL airframes (Gazebo x500,
  VTOL, jMAVSim) and boards (Pixhawk 6X/6C/4) via a `px4` task provider.
- **Flash Firmware** — `make … upload`, `probe-rs download`, or `dfu-util`.
- **Debugging** — a `vyuta-probe-rs` debug type that launches
  `probe-rs dap-server` and connects VS Code's debugger (breakpoints, stepping,
  registers).
- **List Debug Probes** — enumerates attached probes via the in-process
  `probe-rs` Neon addon (`rust/probe-rs-extension`).
- **RTT / Semihosting terminal** — `probe-rs attach` output in a terminal.

## Commands

- **Vyuta: Build Firmware…** (`vyuta.firmware.build`)
- **Vyuta: Flash Firmware…** (`vyuta.firmware.flash`)
- **Vyuta: List Debug Probes** (`vyuta.firmware.listProbes`)
- **Vyuta: Open RTT / Semihosting Terminal** (`vyuta.firmware.openRttTerminal`)

## Settings

- `vyuta.firmware.px4Dir` — PX4-Autopilot source tree (default `${workspaceFolder}`)
- `vyuta.firmware.probeRsPath` — `probe-rs` executable (default `probe-rs`)
- `vyuta.firmware.dfuUtilPath` — `dfu-util` executable (default `dfu-util`)

## Prerequisites (runtime)

- [`probe-rs`](https://probe.rs) on `PATH` for debugging/flashing/RTT.
- The native addon built once: `cd rust/probe-rs-extension && npm run build`.
- For PX4 builds: the PX4-Autopilot toolchain and `make`.

See [`../../docs/phase-2.md`](../../docs/phase-2.md).
