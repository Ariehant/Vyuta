#!/bin/bash
set -euo pipefail

# Vyuta SessionStart hook.
#
# Prepares the drone-IDE monorepo so linters and tests work in Claude Code on
# the web. It only sets up the Vyuta-specific projects (the Rust workspace and
# the drone-* extensions) — NOT VS Code's own ~full build, which is heavy and
# not needed for robotics development.
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

# Vyuta extensions: install dev dependencies (typescript, @types/*) for each.
for ext in "$ROOT"/extensions/drone-*/; do
  if [ -f "$ext/package.json" ]; then
    echo "[vyuta] installing $(basename "$ext") extension dependencies…"
    (cd "$ext" && npm install --no-audit --no-fund)
  fi
done

echo "[vyuta] session setup complete."
