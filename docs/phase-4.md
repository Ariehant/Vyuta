# Phase 4 — Parameter Tuning Panel

**Goal:** Read, edit, and manage flight-controller parameters live from the IDE.

## What this delivers

| Deliverable                                          | Status | Where                                       |
| ---------------------------------------------------- | ------ | ------------------------------------------- |
| Parameter cache (`PARAM_VALUE`) in the gateway       | ✅     | `rust/maestros/src/params.rs`               |
| `PARAM_REQUEST_LIST` / `PARAM_REQUEST_READ` requests | ✅     | `params.rs` (`ParamService`)                |
| `PARAM_SET` write path over the live link            | ✅     | `params.rs` + `sources/mavlink_source.rs`   |
| Bidirectional gateway protocol (commands + sync)     | ✅     | `rust/maestros/src/ws.rs`                   |
| Snapshots + diff (changed / added / removed)         | ✅     | `params.rs` (`save_snapshot` / `diff`)      |
| Synthetic PX4-like param set (no vehicle needed)     | ✅     | `params.rs` (`seed_synthetic`)              |
| Subsystem-grouped tree view + filter                 | ✅     | `extensions/drone-tuning`                   |
| Live Tune toggle (immediate vs staged + Apply)       | ✅     | `extensions/.../media/tuning.js`            |
| Snapshot save / diff UI with in-tree highlighting    | ✅     | same                                        |
| `mav_sim` parameter server (for testing)             | ✅     | `rust/maestros/examples/mav_sim.rs`         |

### Design notes / decisions

- **Parameters live in `maestros`.** Parameter traffic (`PARAM_REQUEST_LIST`,
  `PARAM_VALUE`, `PARAM_SET`) rides the MAVLink link, and `maestros` already
  owns that link — so the parameter store lives there rather than in a new
  sidecar that would fight for the same UDP endpoint. The MAVLink reader now
  publishes its connection into a shared slot so the parameter service can
  *write* (`PARAM_SET` / requests) on the same connection it reads.

- **Bidirectional gateway, non-breaking.** The gateway's WebSocket became
  bidirectional. Telemetry frames are unchanged and still carry **no** `type`
  field; parameter messages are tagged (`param_value`, `param_progress`,
  `param_ack`, `snapshot_*`). A client only receives parameter messages after it
  sends a parameter command, so the Phase 1 telemetry panel — which never sends
  commands — is completely unaffected (no change to `drone-telemetry`). Each
  client gets a lazily-spawned param-sync task that forwards changed values
  (tracked by a store version counter) plus load progress.

- **Vanilla tree view, not React.** The plan calls for a "React tree view"; the
  fork's extensions build with plain `tsc` and vanilla webview scripts (no
  bundler), so the tree is a dependency-free DOM tree — the same kind of
  pragmatic substitution as JSON-over-FlatBuffers (Phase 1) and JSON control
  for the sim sidecar (Phase 3). It groups by the id prefix (subsystem),
  filters, and edits in place.

- **Optimistic + confirmed edits.** A `set_param` updates the store immediately
  (snappy UI) and, on a real link, sends `PARAM_SET`; the vehicle's echoed
  `PARAM_VALUE` confirms/corrects it. "Live Tune" off stages edits (highlighted)
  until **Apply**; **Revert** drops them.

- **Snapshots & diff** are kept in the store; a diff classifies each parameter
  as changed / added / removed and the panel highlights affected rows and lists
  the deltas.

## Protocol additions (client ↔ maestros)

Client → gateway (`{"cmd": …}`): `request_params`, `set_param` (id, value),
`refresh_param` (id), `save_snapshot` (name), `diff_snapshot` (name),
`delete_snapshot` (name), `list_snapshots`.

Gateway → client (`{"type": …}`): `param_value` (id, value, param_type, index,
count), `param_progress` (received, total), `param_ack` (id, ok, value, message),
`snapshot_list` (names), `snapshot_diff` (name, entries).

## Build & run

```sh
# Synthetic parameters out of the box:
cd rust && cargo run --bin maestros

# Against a (simulated) vehicle — mav_sim now also serves parameters:
VYUTA_MAVLINK_URL=udpin:0.0.0.0:14550 cargo run --bin maestros
cargo run --example mav_sim -- udpout:127.0.0.1:14550

# The extension:
cd extensions/drone-tuning && npm install && npm run compile
code --extensionDevelopmentPath="$PWD"   # then: "Vyuta: Open Parameter Tuning Panel"
```

## Verification performed

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`,
  `cargo test --workspace` all pass — including 6 new `maestros` param-store
  tests (versioned change tracking, set/type, snapshot diff add/change/remove,
  synthetic seeding, id encode/decode, MAV_PARAM_TYPE mapping).
- **Synthetic, end-to-end:** ran `maestros` and drove it from a WebSocket
  client — telemetry frames flow with no parameter messages until a command is
  sent (telemetry panel unaffected); `request_params` delivered all 40 seeded
  params + progress 40/40; `save_snapshot` then `set_param` updates were echoed
  back; `diff_snapshot` reported exactly the changed params; an unknown id was
  acked `ok:false`.
- **Real MAVLink, end-to-end:** ran `maestros` on `udpin:0.0.0.0:14550` with
  `mav_sim` as a parameter server — the live link reported `source=mavlink`,
  `request_params` round-tripped `PARAM_REQUEST_LIST` → 6 decoded `PARAM_VALUE`,
  and `set_param MC_ROLLRATE_P=0.30` sent a real `PARAM_SET` that `mav_sim`
  applied and echoed (confirmed in its log).
- **Extension:** `tsc` compiles; `tuning.js` passes `node --check`.

> The live tree rendering/editing is exercised in a GUI Extension Development
> Host; the synthetic + `mav_sim` paths exercise the full
> request/set/snapshot/diff pipeline headlessly here.

## Next: Phase 5

Flight-log analysis — a Rust ULog parser feeding an Arrow/HTTP log browser with
a mode-annotated timeline and an auto-review checklist.
