#!/usr/bin/env bash
# PostToolUse hook: format Rust files after Claude edits them.
#
# Triggered for every Write/Edit. Bails quickly unless the touched file is a
# `.rs` file inside this repo, so non-Rust edits cost nothing. On a real Rust
# edit it touches `.claude/.rust-touched` (read by cargo-check.sh as a Stop-time
# gate) and runs `cargo fmt --all`.

set -uo pipefail

REPO_ROOT="${CLAUDE_PROJECT_DIR:-}"
if [[ -z "$REPO_ROOT" ]]; then
  REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
fi

# stdin is the tool-use JSON event; pull out the edited file path.
payload="$(cat || true)"
file_path="$(printf '%s' "$payload" | jq -r '.tool_input.file_path // empty' 2>/dev/null || true)"

[[ -z "$file_path" ]] && exit 0
[[ "$file_path" != *.rs ]] && exit 0

# Only care about files under this repo.
case "$file_path" in
  "$REPO_ROOT"/*) ;;
  *) exit 0 ;;
esac

mkdir -p "$REPO_ROOT/.claude"
touch "$REPO_ROOT/.claude/.rust-touched"

cd "$REPO_ROOT" || exit 0
cargo fmt --all >/dev/null 2>&1 || true
exit 0
