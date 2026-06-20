#!/bin/bash
set -euo pipefail

# Vyuta SessionStart hook.
#
# Prepares the drone-IDE monorepo so linters and tests work in Claude Code on
# the web. It only sets up the Vyuta-specific projects (the Rust workspace and
# the drone-telemetry extension) — NOT VS Code's own ~full build, which is
# heavy and not needed for robotics development.
#
# Idempotent and non-interactive; safe to run repeatedly.

# Only do work in the remote (web) environment. Local sessions manage their
# own toolchains.
if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ]; then
  exit 0
fi

ROOT="${CLAUDE_PROJECT_DIR:-$(pwd)}"

# Rust workspace: warm the dependency cache so cargo build/clippy/test are
# ready without a first-run download. `--locked` honours Cargo.lock.
if [ -d "$ROOT/rust" ]; then
  echo "[vyuta] fetching Rust workspace dependencies…"
  (cd "$ROOT/rust" && (cargo fetch --locked || cargo fetch))
fi

# drone-telemetry extension: install dev dependencies (typescript, @types/*).
if [ -d "$ROOT/extensions/drone-telemetry" ]; then
  echo "[vyuta] installing drone-telemetry extension dependencies…"
  (cd "$ROOT/extensions/drone-telemetry" && npm install --no-audit --no-fund)
fi

echo "[vyuta] session setup complete."
