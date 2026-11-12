#!/usr/bin/env bash
# Start, stop or restart the Leos API for local development.
#
#   scripts/dev-server.sh start    build and run in the background
#   scripts/dev-server.sh stop     stop it
#   scripts/dev-server.sh restart  stop, rebuild, start
#   scripts/dev-server.sh logs     follow the log
#
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PID_FILE="$ROOT/.leos-server.pid"
LOG_FILE="${LEOS_LOG_FILE:-/tmp/leos-server.log}"
BIN="$ROOT/target/debug/leos-server"

export LEOS_DATABASE_URL="${LEOS_DATABASE_URL:-postgres://leos:leos@localhost:5432/leos}"
export LEOS_BIND_ADDR="${LEOS_BIND_ADDR:-127.0.0.1:8080}"
export LEOS_DEV_MODE="${LEOS_DEV_MODE:-true}"
export LEOS_SEED="${LEOS_SEED:-true}"

stop() {
    if [[ -f "$PID_FILE" ]] && kill -0 "$(cat "$PID_FILE")" 2>/dev/null; then
        kill "$(cat "$PID_FILE")" 2>/dev/null || true
        sleep 1
        kill -9 "$(cat "$PID_FILE")" 2>/dev/null || true
    fi
    rm -f "$PID_FILE"
    # Anything still holding the port would stop the new server binding.
    if command -v fuser >/dev/null 2>&1; then
        fuser -k "${LEOS_BIND_ADDR##*:}/tcp" >/dev/null 2>&1 || true
    fi
}

start() {
    stop
    (cd "$ROOT" && cargo build -p leos-server)
    # setsid detaches the server from this shell so it keeps running after
    # the script exits.
    (cd "$ROOT" && setsid "$BIN" </dev/null >"$LOG_FILE" 2>&1 & echo $! >"$PID_FILE")
    sleep 3
    if ! kill -0 "$(cat "$PID_FILE")" 2>/dev/null; then
        echo "the server failed to start; last lines of $LOG_FILE:" >&2
        tail -20 "$LOG_FILE" >&2
        exit 1
    fi
    echo "leos-server listening on http://${LEOS_BIND_ADDR} (pid $(cat "$PID_FILE"))"
}

case "${1:-start}" in
    start) start ;;
    stop) stop; echo "stopped" ;;
    restart) start ;;
    logs) tail -f "$LOG_FILE" ;;
    *) echo "usage: $0 {start|stop|restart|logs}" >&2; exit 1 ;;
esac
