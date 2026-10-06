#!/usr/bin/env bash
# D92: compares the static musl build (mimalloc) with the glibc build on one alignment, three
# runs each, and prints a Markdown table of the median wall time and the median peak memory for
# the release notes. Threads: 1 and 8 always (8 oversubscribes a smaller runner, which still
# exercises the allocators under contention), 32 and 64 when the runner has that many CPUs.
#
# Usage: scripts/release/allocator-benchmark.sh GNU_BINARY MUSL_BINARY ALIGNMENT
set -euo pipefail
gnu=$1
musl=$2
input=$3
cpus=$(nproc)
out=$(mktemp -d)
trap 'rm -r -- "$out"' EXIT

measure() { # binary threads -> "seconds megabytes": the median of 3 runs of each
  local i
  for i in 1 2 3; do
    /usr/bin/time -f "%e %M" -o "$out/t.$i" "$1" analyze -i "$input" -t "$2" -q -o "$out/r.json"
  done
  # GNU time's %M is in KiB.
  cat "$out"/t.1 "$out"/t.2 "$out"/t.3 | python3 -c '
import statistics, sys
rows = [line.split() for line in sys.stdin if line.strip()]
wall = statistics.median(float(r[0]) for r in rows)
rss = statistics.median(int(r[1]) for r in rows) * 1024 / 1e6
print(f"{wall:.2f} {rss:.0f}")'
}

echo "### Allocator benchmark (musl + mimalloc vs glibc)"
echo
echo "$(basename "$input"), $(grep -c '>' "$input") sequences; runner: ${cpus} CPUs. Medians of 3 runs."
echo
echo "| Threads | glibc wall (s) | glibc peak RSS (MB) | musl wall (s) | musl peak RSS (MB) |"
echo "|---|---|---|---|---|"
for t in 1 8 32 64; do
  [ "$t" -le 8 ] || [ "$t" -le "$cpus" ] || continue
  read -r gw gm < <(measure "$gnu" "$t")
  read -r mw mm < <(measure "$musl" "$t")
  echo "| $t | $gw | $gm | $mw | $mm |"
done
