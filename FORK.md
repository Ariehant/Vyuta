# Vyuta — VS Code Fork

**Vyuta** is a fork of [Visual Studio Code](https://github.com/microsoft/vscode)
being developed into a drone / robotics development IDE ("drone cockpit").

## Fork provenance

This `main` branch tracks a clean, vendored copy of upstream VS Code so it can be
re-synced against future upstream releases. **All Vyuta-specific work** (branding,
extensions, Rust sidecars, telemetry, simulation control, etc.) happens on the
robotics development branch — not here on `main`.

| Field            | Value                                      |
| ---------------- | ------------------------------------------ |
| Upstream repo    | https://github.com/microsoft/vscode        |
| Upstream tag     | `1.125.0`                                  |
| Upstream commit  | `93cfdd489c3b228840d0f86ec77c3636277c93ea` |
| Import method    | Clean tree import (no upstream git history) |
| Date imported    | 2026-06-20                                 |

## Re-syncing with upstream

Because history was not vendored, upstream updates are applied as a tree refresh
against a newer pinned tag:

```sh
git clone --depth 1 --branch <new-tag> https://github.com/microsoft/vscode.git /tmp/vscode-src
# copy /tmp/vscode-src (excluding .git) over this tree, review the diff, commit
```

Update the table above whenever the pinned tag changes.

### Vyuta modifications layered on the baseline

`main` is no longer byte-for-byte upstream; the following Vyuta changes are
applied on top and must be preserved across re-syncs:

- **`product.json`** — rebranded identity fields (`nameShort`/`nameLong` →
  `Vyuta`, `applicationName` → `vyuta`, data folders, win32 AppId GUIDs
  regenerated, URLs pointed at this repo). Icon art assets are not yet
  replaced (still the upstream OSS icons) — that is a follow-up.
- **`.github/workflows/`** — VS Code's upstream CI was removed (it requires
  Microsoft build infrastructure / self-hosted runners and hardcodes
  `microsoft/vscode`). Replaced with `vyuta-ci.yml`.
- **`.claude/`** — SessionStart hook + settings for Claude Code on the web.

When re-syncing, re-apply (or merge) these after refreshing the tree.

## Development branches

- `main` — clean upstream VS Code baseline (this branch).
- `claude/drone-robotics-ide-plan-jehu2f` — active robotics IDE development
  (Phase 0 scaffold and beyond). See `docs/` on that branch for the phased plan.

## License

VS Code source is licensed under the MIT License (see `LICENSE.txt`). Vyuta
additions are subject to the project's own licensing; consult the robotics
branch for details.
