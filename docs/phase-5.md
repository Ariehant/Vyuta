# Phase 5 — Flight Log Analysis (ULog)

**Goal:** Open PX4 ULog flight logs in the IDE, plot any signal, and get an
automatic review of the flight.

## What this delivers

| Deliverable                                       | Status | Where                                       |
| ------------------------------------------------- | ------ | ------------------------------------------- |
| ULog (`.ulg`) parser → named time series          | ✅     | `rust/logbook/src/ulog.rs`                  |
| Flight-mode timeline from `vehicle_status`        | ✅     | `ulog.rs` (`build_modes`)                   |
| Plotting-aware downsampling (min/max buckets)     | ✅     | `rust/logbook/src/model.rs`                 |
| Auto-review engine (vibration/failsafe/battery/…) | ✅     | `rust/logbook/src/review.rs`                |
| Request/response WebSocket server                 | ✅     | `rust/logbook/src/ws.rs`                    |
| Synthetic flight log out of the box               | ✅     | `rust/logbook/src/synthetic.rs`             |
| Log browser: timeline, field plots, review        | ✅     | `extensions/drone-logbook`                  |
| `--write-ulog` sample generator                   | ✅     | `rust/logbook` binary flag                  |

### Design notes / decisions

- **Hand-rolled ULog parser, no `nom`.** The plan names `nom`/`ulog`; the format
  is simple enough (length-prefixed messages over a small set of type codes)
  that a dependency-free cursor parser keeps the workspace lean and offline. It
  reads the header, `F` formats, `A` subscriptions, `I` info, `D` data and `L`
  logged strings, and decodes every flat numeric field (arrays expanded to
  `field[i]`) into a `message[instance].field` time series. Nested-struct fields
  are size-skipped (the common review topics are flat).

- **JSON over WebSocket, not Arrow Flight.** The plan targets Apache Arrow +
  Arrow Flight/HTTP. Arrow and a Flight/protoc stack are heavy and unnecessary
  at Phase 5 sizes, so decoded series are **downsampled** server-side
  (min/max-per-bucket, so spikes survive) and sent as JSON over the same
  request/response WebSocket the other sidecars use. Arrow columns + Flight are
  the documented drop-in upgrade for very large logs (noted in `Cargo.toml`).

- **Synthetic log out of the box.** With no `.ulg` provided, `logbook` *generates
  a valid ULog byte buffer* (takeoff → POSCTL → a vibration burst → an RC-loss
  failsafe → RTL → land) and parses it with the real parser — so the browser is
  immediately useful and the parser is exercised on every run. The same bytes
  back a `--write-ulog <path>` helper and the parser round-trip test.

- **uPlot, vendored.** The plan says "reuse uPlot"; it's vendored like
  Leaflet/Three (an IIFE global, no bundler needed). Each selected field is a
  stacked uPlot chart with synced cursors and flight-mode background bands.

- **Scope note.** Single-log browsing is implemented; the plan's *side-by-side
  two-log comparison* is deferred (multiple fields can already be compared as
  stacked synced charts).

## Protocol (browser ↔ logbook)

Client → server (`{"cmd": …}`): `overview`, `series` (names, max_points),
`review`, `load` (path), `synthetic`.

Server → client (`{"type": …}`): `overview` (source, name, duration, series
summaries, modes, messages, info), `series` (downsampled `{t, v}` per name),
`review` (findings with severity), `error`.

## Configuration (logbook — environment variables)

| Variable             | Default          | Meaning                              |
| -------------------- | ---------------- | ------------------------------------ |
| `VYUTA_LOGBOOK_ADDR` | `127.0.0.1:9878` | WebSocket bind address               |
| `VYUTA_ULOG_PATH`    | _(unset)_        | `.ulg` to load at start (else synth) |

Extension setting: `vyuta.logbook.serverUrl`.

## Build & run

```sh
cd rust
cargo run --bin logbook                                   # synthetic log
VYUTA_ULOG_PATH=/path/to/flight.ulg cargo run --bin logbook
cargo run --bin logbook -- --write-ulog sample.ulg        # emit a sample .ulg

cd ../extensions/drone-logbook && npm install && npm run compile
code --extensionDevelopmentPath="$PWD"   # then: "Vyuta: Open Flight Log Analyzer"
```

## Verification performed

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`,
  `cargo test --workspace` all pass — including 6 `logbook` tests (parser
  round-trip on the synthetic ULog: expected series + expanded accel axes +
  mode timeline; review flags vibration/failsafe/battery; magic/format checks).
- **End-to-end:** `logbook --write-ulog` produced a 125 KB `.ulg`; running the
  server with `VYUTA_ULOG_PATH` pointed at that file loaded **21 series** and a
  WebSocket client received the overview (modes incl. `AUTO_RTL`, 30 s), the
  auto-review (`vibration=warning`, `failsafe=warning`, `battery=warning`), and
  downsampled series on request (accel 1500→60 pts) — proving the writer →
  on-disk → parser → server path.
- **Extension:** `tsc` compiles; `logbook.js` passes `node --check`; uPlot is
  vendored (IIFE global).

> The live uPlot rendering runs in a GUI Extension Development Host; the
> synthetic + on-disk paths exercise parse → review → serve headlessly here.

## Next: Phase 6

Companion computer & ROS 2 integration — grow `agent` into a tonic gRPC daemon
(file sync, `colcon build`, node lifecycle) with a node/topic browser panel.
