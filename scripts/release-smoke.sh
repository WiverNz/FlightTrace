#!/usr/bin/env bash
# release-smoke.sh — verify a packaged FlightTrace tree end-to-end.
#
# Runs `flighttrace serve --web-dir <pkg-dir>/web-dist` on a random
# loopback port, hits the endpoints a real release archive would need
# to satisfy, and tears the server down cleanly on both success and
# failure paths.
#
# Usage:
#   scripts/release-smoke.sh                      # default: expect the
#                                                 # dev-local layout —
#                                                 # target/release/flighttrace
#                                                 # + web/dist
#   scripts/release-smoke.sh --pkg-dir dist/xyz   # explicit packaged tree
#                                                 # containing `flighttrace`
#                                                 # and `web-dist/`
#   scripts/release-smoke.sh -h                   # help
#
# Exits 0 on success, non-zero otherwise. All state (SQLite file, logs)
# lands in a `mktemp -d` scratch dir that is removed on exit.

set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"

pkg_dir=""
usage() {
  sed -n '2,20{s/^# \{0,1\}//;p;}' "$0"
}

while (( $# )); do
  case "$1" in
    -h|--help) usage; exit 0 ;;
    --pkg-dir) shift; pkg_dir="${1:?--pkg-dir requires a directory}" ;;
    *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
  shift
done

# ---------- resolve the packaged tree ----------
if [[ -n "$pkg_dir" ]]; then
  bin="$pkg_dir/flighttrace"
  web_dist="$pkg_dir/web-dist"
else
  # Dev-local layout: build outputs + the frontend build in web/dist.
  bin="$repo_root/target/release/flighttrace"
  web_dist="$repo_root/web/dist"
fi

echo "→ checking layout"
[[ -x "$bin" ]] || { echo "error: binary not found or not executable: $bin" >&2; exit 1; }
[[ -f "$web_dist/index.html" ]] || { echo "error: web-dist missing index.html: $web_dist/index.html" >&2; exit 1; }
echo "  binary:   $bin"
echo "  web-dist: $web_dist"

scratch="$(mktemp -d -t flighttrace-smoke-XXXXXX)"
db="$scratch/flighttrace-smoke.db"
log="$scratch/server.log"
pid=""

cleanup() {
  if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
    kill "$pid" 2>/dev/null || true
    # Give the process a beat to flush; then be firm.
    for _ in 1 2 3 4 5 6 7 8 9 10; do
      kill -0 "$pid" 2>/dev/null || break
      sleep 0.2
    done
    kill -9 "$pid" 2>/dev/null || true
  fi
  rm -rf "$scratch"
}
trap cleanup EXIT INT TERM

# ---------- pick a free localhost port ----------
port="$(python3 -c "import socket; s=socket.socket(); s.bind(('127.0.0.1', 0)); print(s.getsockname()[1]); s.close()")"
echo "→ starting server on 127.0.0.1:$port"

"$bin" serve \
  --bind "127.0.0.1:$port" \
  --database "$db" \
  --web-dir "$web_dist" \
  >"$log" 2>&1 &
pid=$!

# ---------- wait for /health ----------
echo "→ waiting for /health"
ready=0
for _ in $(seq 1 50); do
  if curl -fsS --max-time 1 "http://127.0.0.1:$port/health" >/dev/null 2>&1; then
    ready=1
    break
  fi
  # If the server already died, no point in continuing to poll.
  if ! kill -0 "$pid" 2>/dev/null; then break; fi
  sleep 0.2
done

if (( ready == 0 )); then
  echo "error: server did not become ready in 10 s" >&2
  echo "--- last 40 lines of server log ---" >&2
  tail -n 40 "$log" >&2 || true
  exit 1
fi

fail=0
check() {
  local name="$1"; shift
  if "$@"; then
    echo "  OK   $name"
  else
    echo "  FAIL $name" >&2
    fail=1
  fi
}

echo "→ running checks"

# 1. /health returns JSON with status=ok.
check "/health returns JSON status=ok" bash -c "
  body=\$(curl -fsS 'http://127.0.0.1:$port/health')
  [[ \$(echo \"\$body\" | python3 -c 'import sys,json; print(json.load(sys.stdin)[\"status\"])') == ok ]]
"

# 2. HEAD / returns 200 text/html (index.html, not JSON).
check "HEAD / returns 200 text/html" bash -c "
  headers=\$(curl -fsSI 'http://127.0.0.1:$port/')
  echo \"\$headers\" | grep -qi '^HTTP/1\\.[01] 200' &&
  echo \"\$headers\" | grep -qi '^content-type: text/html'
"

# 3. POST /api/demo/run returns 201 with a UUID.
run_id=""
if run_id=$(curl -fsS -X POST "http://127.0.0.1:$port/api/demo/run" \
              | python3 -c 'import sys,json; print(json.load(sys.stdin)["id"])'); then
  echo "  OK   POST /api/demo/run → id=$run_id"
else
  echo "  FAIL POST /api/demo/run" >&2
  fail=1
fi

# 4. /api/runs/<id>/failure-report contains TAKEOFF_TIMEOUT.
if [[ -n "$run_id" ]]; then
  check "GET /api/runs/<id>/failure-report → TAKEOFF_TIMEOUT" bash -c "
    body=\$(curl -fsS 'http://127.0.0.1:$port/api/runs/$run_id/failure-report')
    reason=\$(echo \"\$body\" | python3 -c 'import sys,json; print(json.load(sys.stdin)[\"reason\"])')
    [[ \$reason == TAKEOFF_TIMEOUT ]]
  "
fi

# 5. Unknown /api/... returns JSON 404, not the SPA fallback.
check "GET /api/nonsense → 404 JSON envelope" bash -c "
  status=\$(curl -sS -o '$scratch/api-nonsense.body' -w '%{http_code} %{content_type}' 'http://127.0.0.1:$port/api/nonsense')
  code=\$(cut -d' ' -f1 <<<\"\$status\")
  ctype=\$(cut -d' ' -f2 <<<\"\$status\")
  [[ \$code == 404 ]] &&
  [[ \$ctype == application/json* ]] &&
  python3 -c \"import json; d=json.load(open('$scratch/api-nonsense.body')); assert d['error']['code'] == 'not_found'\"
"

# 6. HEAD /runs/<id> falls back to index.html (SPA fallback).
if [[ -n "$run_id" ]]; then
  check "HEAD /runs/<id> → SPA fallback to index.html" bash -c "
    headers=\$(curl -fsSI 'http://127.0.0.1:$port/runs/$run_id')
    echo \"\$headers\" | grep -qi '^HTTP/1\\.[01] 200' &&
    echo \"\$headers\" | grep -qi '^content-type: text/html'
  "
fi

echo
if (( fail == 0 )); then
  echo "release-smoke: all checks passed"
else
  echo "release-smoke: one or more checks FAILED" >&2
  echo "--- last 40 lines of server log ---" >&2
  tail -n 40 "$log" >&2 || true
  exit 1
fi
