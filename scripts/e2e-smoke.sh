#!/usr/bin/env bash
# End-to-end check of the built web interface against a running API.
#
# Builds the frontend, starts the server on a spare port with a throwaway rule
# store, drives the real bundle in headless Chromium, and reports what each route
# renders. Fails when a route renders nothing, throws, cannot be deep-linked, or
# stops working once signed in.
#
#   make e2e
#
# Requirements: a Chromium or Chrome binary (the CI image has one) and no
# container runtime — everything runs from the workspace.
set -euo pipefail

cd "$(dirname "$0")/.."
REPO_ROOT=$(pwd)

# Refuse to share a port: if something already listens, the server this script starts
# dies and the browser silently talks to whatever was there — serving a stale
# `index.html` against freshly built chunks, which reads as "the app renders blank".
port_free() {
    ! (exec 3<>"/dev/tcp/127.0.0.1/$1") 2>/dev/null
}

if [ -n "${E2E_PORT:-}" ]; then
    PORT="$E2E_PORT"
    if ! port_free "$PORT"; then
        echo "e2e: port $PORT is already in use; stop that process or set E2E_PORT" >&2
        exit 1
    fi
else
    PORT=""
    for candidate in $(seq 8098 8120); do
        if port_free "$candidate"; then PORT="$candidate"; break; fi
    done
    if [ -z "$PORT" ]; then
        echo "e2e: no free port in 8098-8120" >&2
        exit 1
    fi
fi
WORKDIR="$REPO_ROOT/target/e2e"
mkdir -p "$WORKDIR/rules" "$WORKDIR/dmn"

# Prefer an explicitly named browser, else the first one on PATH.
CHROME="${E2E_CHROME:-}"
if [ -z "$CHROME" ]; then
    for candidate in chromium-browser chromium google-chrome google-chrome-stable; do
        if command -v "$candidate" >/dev/null 2>&1; then CHROME="$candidate"; break; fi
    done
fi
if [ -z "$CHROME" ]; then
    echo "e2e: no Chromium or Chrome found; set E2E_CHROME to a browser binary" >&2
    exit 1
fi

echo "==> building the interface"
if [ ! -d frontend/node_modules ]; then
    (cd frontend && npm ci --no-audit --no-fund)
fi
(cd frontend && npm run build >/dev/null)

echo "==> building the server"
cargo build --release --locked -p api --bin rRules-server

echo "==> starting the server on :$PORT"
RRULES_BIND_ADDRESS=127.0.0.1 PORT="$PORT" \
FRONTEND_DIST="$REPO_ROOT/frontend/dist" \
RULE_STORAGE_PATH="$WORKDIR/rules" \
DMN_STORAGE_PATH="$WORKDIR/dmn" \
WATCH_RULES_DIR=false RUST_LOG=warn \
    ./target/release/rRules-server > "$WORKDIR/server.log" 2>&1 &
SERVER_PID=$!
cleanup() { kill "$SERVER_PID" 2>/dev/null || true; }
trap cleanup EXIT

for _ in $(seq 1 50); do
    if curl -sf -o /dev/null "http://127.0.0.1:$PORT/health"; then break; fi
    sleep 0.2
done
if ! curl -sf -o /dev/null "http://127.0.0.1:$PORT/health"; then
    echo "e2e: the server did not become healthy; see $WORKDIR/server.log" >&2
    exit 1
fi

node frontend/e2e/smoke.mjs --base "http://127.0.0.1:$PORT" --chrome "$CHROME"
