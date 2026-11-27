#!/usr/bin/env bash
# D96: as PID 1 (e.g. the entry point of a container) a re-raised default-action signal is
# ignored by the kernel, so dima must exit with 128 + n itself, never abort. Runs dima as PID 1
# of a new PID namespace (needs root or sudo) and checks the ways a run can end:
#   1. the main thread notices the cancellation (input: a pipe that never ends) → prompt exit;
#   2. the input has not sent a byte yet (a FIFO without a writer, opened on a thread of its
#      own) → prompt exit, with the run report;
#   3. the main thread is blocked (opening an output FIFO without a reader) → exit after the
#      grace period;
#   4. a second signal while blocked → immediate exit.
# Usage: scripts/ci/pid1-signal-test.sh path/to/dima
set -euo pipefail

bin=$(realpath "$1")
root=$(cd "$(dirname "$0")/../.." && pwd)
sudo=()
[ "$(id -u)" -eq 0 ] || sudo=(sudo)
work=$(mktemp -d)
cd "$work"
failed=0

descendants() { # every process below $1 (sudo -> unshare -> dima under sudo)
  local p
  for p in $(pgrep -P "$1"); do
    echo "$p"
    descendants "$p"
  done
}

pid1_of() { # the process below $1 that is PID 1 of its namespace
  local p
  for p in $(descendants "$1"); do
    if grep -q $'NSpid:\t[0-9]*\t1$' "/proc/$p/status" 2>/dev/null; then echo "$p"; return; fi
  done
}

wait_for_pid1() {
  local u=$1 p="" i
  for i in $(seq 1 100); do
    p=$(pid1_of "$u")
    [ -n "$p" ] && break
    sleep 0.05
  done
  echo "$p"
}

check() { # name expected actual
  if [ "$2" = "$3" ]; then echo "ok: $1 (status $3)"; else echo "FAIL: $1: expected $2, got $3"; failed=1; fi
}

# 1. Prompt cancellation from the main thread.
mkfifo feed
(cat "$root/tests/golden/tiny/input.fasta"; sleep 60) > feed &
feeder=$!
"${sudo[@]}" unshare --pid --fork --mount-proc "$bin" analyze -a protein -k 3 \
  --validation permissive -o out1.json --run-report run1.json < feed 2>/dev/null &
u=$!
sleep 1
p=$(wait_for_pid1 "$u")
[ -n "$p" ] || { echo "FAIL: dima did not start as PID 1"; exit 1; }
start=$(date +%s)
"${sudo[@]}" kill -TERM "$p"
set +e; wait "$u"; status=$?; set -e
check "SIGTERM, main thread" 143 "$status"
[ $(( $(date +%s) - start )) -le 2 ] || { echo "FAIL: not prompt"; failed=1; }
kill "$feeder" 2>/dev/null || true
grep -q '"status": "cancelled"' run1.json || { echo "FAIL: run report status"; failed=1; }

# 2. An input that has not sent a byte yet: opened on a thread of its own, so the cancellation
#    is noticed at once and the run report written.
mkfifo in.fifo
"${sudo[@]}" unshare --pid --fork --mount-proc "$bin" analyze -i in.fifo -o out2.json \
  --run-report run2.json 2>/dev/null &
u=$!
sleep 1
p=$(wait_for_pid1 "$u")
[ -n "$p" ] || { echo "FAIL: dima did not start as PID 1"; exit 1; }
start=$(date +%s)
"${sudo[@]}" kill -TERM "$p"
set +e; wait "$u"; status=$?; set -e
check "SIGTERM, input not yet open" 143 "$status"
[ $(( $(date +%s) - start )) -le 2 ] || { echo "FAIL: not prompt"; failed=1; }
grep -q '"status": "cancelled"' run2.json || { echo "FAIL: run report status"; failed=1; }

# 3. Blocked main thread (opening an output FIFO that no one reads): the grace period ends the
#    process.
mkfifo out.fifo
tiny="$root/tests/golden/tiny/input.fasta"
"${sudo[@]}" unshare --pid --fork --mount-proc "$bin" analyze -i "$tiny" -a protein -k 3 \
  --validation permissive -o out.fifo -O json 2>/dev/null &
u=$!
sleep 1
p=$(wait_for_pid1 "$u")
[ -n "$p" ] || { echo "FAIL: dima did not start as PID 1"; exit 1; }
"${sudo[@]}" kill -TERM "$p"
set +e; wait "$u"; status=$?; set -e
check "SIGTERM, blocked, grace period" 143 "$status"

# 4. Blocked main thread: a second signal ends the process at once. (SIGTERM, not SIGINT: a
#    non-interactive shell starts background jobs with SIGINT ignored, and dima keeps signals
#    that were ignored at startup ignored.)
"${sudo[@]}" unshare --pid --fork --mount-proc "$bin" analyze -i "$tiny" -a protein -k 3 \
  --validation permissive -o out.fifo -O json 2>/dev/null &
u=$!
sleep 1
p=$(wait_for_pid1 "$u")
[ -n "$p" ] || { echo "FAIL: dima did not start as PID 1"; exit 1; }
"${sudo[@]}" kill -TERM "$p"
sleep 0.5
start=$(date +%s%N)
"${sudo[@]}" kill -TERM "$p"
set +e; wait "$u"; status=$?; set -e
check "second SIGTERM, blocked" 143 "$status"
[ $(( ($(date +%s%N) - start) / 1000000 )) -lt 1000 ] || { echo "FAIL: second signal not immediate"; failed=1; }

for f in out1.json out2.json; do
  [ ! -e "$f" ] || { echo "FAIL: $f was written"; failed=1; }
done
# The atomic writer's temporary files are named .<output>.dima-<pid>-<n>.tmp.
if find . -name '*.dima-*.tmp' | grep -q .; then echo "FAIL: temporary files left"; failed=1; fi
exit "$failed"
