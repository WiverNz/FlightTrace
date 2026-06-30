#!/usr/bin/env bash
# Stop hook: fmt-check + clippy + workspace tests after a turn that touched
# Rust. Gated on the `.claude/.rust-touched` sentinel that cargo-fmt.sh writes
# during PostToolUse, so turns that only edited docs/web/yaml skip the gate.
#
# Why `--workspace` for tests: FlightTrace's workspace tests are fast (~1s
# warm), so there's no reason to scope down to a single crate the way a project
# with a slow-linking shell (e.g. Tauri) would. Clippy uses `--workspace`
# without `--all-targets` to stay quick — tests cover the test-only code.

set -uo pipefail

REPO_ROOT="${CLAUDE_PROJECT_DIR:-}"
if [[ -z "$REPO_ROOT" ]]; then
  REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
fi

sentinel="$REPO_ROOT/.claude/.rust-touched"
[[ -f "$sentinel" ]] || exit 0
rm -f "$sentinel"

cd "$REPO_ROOT" || exit 0

errs=""

if ! out=$(cargo fmt --all -- --check 2>&1); then
  errs+=$'\ncargo fmt --all -- --check failed:\n'"$out"
fi

if ! out=$(cargo clippy --workspace -- -D warnings 2>&1); then
  errs+=$'\ncargo clippy --workspace -- -D warnings failed:\n'"$out"
fi

if ! out=$(cargo test --workspace --quiet 2>&1); then
  errs+=$'\ncargo test --workspace --quiet failed:\n'"$out"
fi

if [[ -n "$errs" ]]; then
  printf '%s\n' "$errs" >&2
  exit 2
fi

exit 0
