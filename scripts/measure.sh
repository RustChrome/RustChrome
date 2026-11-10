#!/usr/bin/env bash
# Launch RustChrome on a virtual display and record memory, idle CPU and the
# startup time it reports itself. Prints what it measured, nothing else.
set -uo pipefail

EXE="${1:?usage: measure.sh <exe> <out>}"
OUT="${2:?usage: measure.sh <exe> <out>}"
STARTUP_LOG="$(mktemp)"

{
  echo "# RustChrome measurement"
  echo "date: $(date -u +%FT%TZ)"
  echo "kernel: $(uname -srm)"
  echo "cpus: $(nproc)"
  echo "exe: $(stat -c %s "$EXE") bytes"
} >"$OUT"

RUSTCHROME_STARTUP_LOG="$STARTUP_LOG" \
xvfb-run -a env WEBKIT_DISABLE_COMPOSITING_MODE=1 LIBGL_ALWAYS_SOFTWARE=1 \
  RUSTCHROME_STARTUP_LOG="$STARTUP_LOG" \
  dbus-run-session -- "$EXE" >/dev/null 2>&1 &
WRAPPER=$!

sleep 3
PID=$(pgrep -n -f '/rustchrome$' || true)
if [ -z "${PID:-}" ]; then
  echo "target process found: no" >>"$OUT"
  kill "$WRAPPER" 2>/dev/null
  exit 1
fi
echo "pid: $PID" >>"$OUT"
echo "processes: $PROCS -> $PROCS2" >>"$OUT"

# Every descendant, so WebKitGTK's web and network processes are counted.
# Measuring only the parent would understate the browser by roughly ten times.
tree() {
  echo "$1"
  local c
  for c in $(pgrep -P "$1" 2>/dev/null); do tree "$c"; done
}

rss_tree() {
  local total=0 pid kb
  for pid in $(tree "$1"); do
    kb=$(awk '/VmRSS/{print $2}' "/proc/$pid/status" 2>/dev/null || echo 0)
    total=$((total + ${kb:-0}))
  done
  echo "$total"
}

cpu_tree() {
  local total=0 pid
  for pid in $(tree "$1"); do
    total=$((total + $(awk '{print $14+$15}' "/proc/$pid/stat" 2>/dev/null || echo 0)))
  done
  echo "$total"
}

sleep 5
PROCS=$(tree "$PID" | wc -l)
RSS1=$(rss_tree "$PID")
T1=$(cpu_tree "$PID")

# Idle window: no navigation, no interaction.
sleep 5
RSS2=$(rss_tree "$PID")
T2=$(cpu_tree "$PID")
PROCS2=$(tree "$PID" | wc -l)

DELTA_TICKS=$((T2 - T1))
# 100 ticks per second on Linux; one core's worth is 100*5 ticks over 5s.
CPU_PCT=$((DELTA_TICKS / 5))

echo "rss_kb_after_8s: $RSS1" >>"$OUT"
echo "rss_kb_after_13s: $RSS2" >>"$OUT"
echo "idle_window_s: 5" >>"$OUT"
echo "idle_cpu_ticks: $DELTA_TICKS" >>"$OUT"
echo "idle_cpu_percent_of_one_core: $CPU_PCT" >>"$OUT"

if [ -s "$STARTUP_LOG" ]; then
  grep startup_ms "$STARTUP_LOG" | head -1 >>"$OUT" || true
else
  echo "startup_ms: not reported" >>"$OUT"
fi

kill "$PID" 2>/dev/null
kill "$WRAPPER" 2>/dev/null
wait "$WRAPPER" 2>/dev/null
rm -f "$STARTUP_LOG"
cat "$OUT"
