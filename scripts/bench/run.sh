#!/usr/bin/env bash
# Reproducible benchmarks (D81): wall time with hyperfine (≥ 10 runs) and peak memory with
# GNU time, on the committed datasets, synthetic scale-ups, every input compression and a
# thread-scaling series, optionally against DiMA v5.0.9 (`pip install dima-cli`). Writes
# BENCHMARK.md with the hardware, software versions and exact commands.
#
# Usage: scripts/bench/run.sh [--quick] [--dima PATH] [--v509 PATH] [--out FILE]
#   --quick   fewer runs (3) and smaller scale-ups, for constrained machines; marked as such
#   --dima    the dima binary (default: target/release/dima, built if missing)
#   --v509    a DiMA v5.0.9 executable (`dima-cli`) to compare with
#   --out     where to write the report (default: BENCHMARK.md)
# Needs: hyperfine, GNU time (/usr/bin/time), python3, gzip, bzip2, zstd.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
quick=0
dima="$root/target/release/dima"
v509=""
out="$root/BENCHMARK.md"
while [ $# -gt 0 ]; do
  case "$1" in
    --quick) quick=1 ;;
    --dima) dima=$2; shift ;;
    --v509) v509=$2; shift ;;
    --out) out=$2; shift ;;
    *) echo "unknown option $1" >&2; exit 2 ;;
  esac
  shift
done
for tool in hyperfine python3 gzip bzip2 zstd; do
  command -v "$tool" >/dev/null || { echo "error: $tool is required" >&2; exit 2; }
done
[ -x /usr/bin/time ] || { echo "error: GNU time (/usr/bin/time) is required" >&2; exit 2; }
if [ ! -x "$dima" ]; then
  cargo build --release --locked -p dima --manifest-path "$root/Cargo.toml" >&2
fi

if [ "$quick" = 1 ]; then runs=3; factor=10; genomes=200; diverse=1000; else runs=10; factor=1000; genomes=2000; diverse=10000; fi
data=$(mktemp -d "${TMPDIR:-/tmp}/dima-bench.XXXXXX")
results=$(mktemp -d "${TMPDIR:-/tmp}/dima-bench-results.XXXXXX")
trap 'rm -r -- "$data" "$results"' EXIT
cpus=$(nproc)

echo "preparing data in $data" >&2
cp "$root/samples/mers_spike_aa.fasta" "$data/mers_spike_aa.fasta"
bzip2 -dc "$root/tests/data/MERS_Spike_nt.fasta.bz2" > "$data/mers_spike_nt.fasta"
bzip2 -dc "$root/tests/data/HIV1_ALL_2020_pol_PRO.fasta.bz2" > "$data/hiv_pol_protein.fasta"
bzip2 -dc "$root/tests/data/HIV1_FLT_2022_pol_DNA.fasta.bz2" > "$data/hiv_pol_dna.fasta"
gzip -k -6 "$data/hiv_pol_dna.fasta"
bzip2 -k -9 "$data/hiv_pol_dna.fasta"
zstd -q -T0 -19 "$data/hiv_pol_dna.fasta" -o "$data/hiv_pol_dna.fasta.zst"
python3 "$root/scripts/bench/make_data.py" scale "$data/hiv_pol_dna.fasta" "$factor" "$data/hiv_x$factor.fasta"
zstd -q -T0 --rm "$data/hiv_x$factor.fasta" -o "$data/hiv_x$factor.fasta.zst"
python3 "$root/scripts/bench/make_data.py" genome 30000 "$genomes" "$data/genome30kb.fasta"
python3 "$root/scripts/bench/make_data.py" diverse "$diverse" 1500 "$data/diverse_protein.fasta"

records() { # sequences in a (possibly compressed) FASTA: its header lines
  case "$1" in
    *.gz) gzip -dc "$1" | grep -c '^>' ;;
    *.bz2) bzip2 -dc "$1" | grep -c '^>' ;;
    *.zst) zstd -dc "$1" | grep -c '^>' ;;
    *) grep -c '^>' "$1" ;;
  esac
}

table_rows=()
# bench LABEL FILE [dima options…]
bench() {
  local label=$1 file=$2
  shift 2
  # Quote the options for hyperfine's shell: `--header-format 'Id|Host|Year'` is not a pipe.
  local quoted=""
  [ $# -eq 0 ] || quoted=$(printf ' %q' "$@")
  local cmd="$dima analyze -i $file -q -o $results/out.json$quoted"
  hyperfine --style none --runs "$runs" --warmup 1 --export-json "$results/$label.json" "$cmd" >/dev/null
  /usr/bin/time -f "%M" -o "$results/$label.rss" "$dima" analyze -i "$file" -q -o "$results/out.json" "$@"
  local stats
  stats=$(python3 - "$results/$label.json" "$results/$label.rss" <<'PY'
import json, sys
r = json.load(open(sys.argv[1]))["results"][0]
rss_kb = int(open(sys.argv[2]).read().split()[-1])
print(f"{r['mean']:.3f} ± {r['stddev']:.3f} | {r['min']:.3f} | {r['max']:.3f} | {rss_kb * 1024 / 1e6:.1f}")
PY
)
  local n
  n=$(records "$file")
  local args=$*
  args=${args//|/\\|} # a literal | would end the table cell
  table_rows+=("| $label | $(printf "%'d" "$n") | ${args:-defaults} | $stats |")
}

echo "benchmarking ($runs runs each)" >&2
bench "MERS spike protein" "$data/mers_spike_aa.fasta"
bench "MERS spike nucleotide" "$data/mers_spike_nt.fasta"
bench "HIV-1 pol protein" "$data/hiv_pol_protein.fasta"
bench "HIV-1 pol DNA" "$data/hiv_pol_dna.fasta"
bench "HIV-1 pol DNA (.gz)" "$data/hiv_pol_dna.fasta.gz"
bench "HIV-1 pol DNA (.bz2)" "$data/hiv_pol_dna.fasta.bz2"
bench "HIV-1 pol DNA (.zst)" "$data/hiv_pol_dna.fasta.zst"
bench "HIV-1 pol DNA ×$factor (.zst)" "$data/hiv_x$factor.fasta.zst"
bench "30 kb genome" "$data/genome30kb.fasta" -a dna
bench "High-diversity protein" "$data/diverse_protein.fasta"
# LANL headers are '.'-separated, so metadata stratification is measured on the synthetic
# genome alignment, whose headers are 'Id|Host|Year'.
bench "30 kb genome, metadata" "$data/genome30kb.fasta" -a dna --header-format 'Id|Host|Year' --metadata-fields 'Host|Year'

scaling_rows=()
for t in 1 2 4 8 "$cpus"; do
  [ "$t" -le "$cpus" ] || continue
  case " ${seen:-} " in *" $t "*) continue ;; esac
  seen="${seen:-} $t"
  hyperfine --style none --runs "$runs" --warmup 1 --export-json "$results/threads$t.json" \
    "$dima analyze -i $data/hiv_x$factor.fasta.zst -q -o $results/out.json -t $t" >/dev/null
  scaling_rows+=("$t $(python3 -c "import json,sys;r=json.load(open(sys.argv[1]))['results'][0];print(r['mean'],r['stddev'])" "$results/threads$t.json")")
done

v509_rows=()
if [ -n "$v509" ]; then
  for pair in "MERS spike protein:$data/mers_spike_aa.fasta" "HIV-1 pol DNA:$data/hiv_pol_dna.fasta"; do
    label=${pair%%:*}
    file=${pair#*:}
    hyperfine --style none --runs "$runs" --warmup 1 --export-json "$results/v509.json" \
      "$v509 -i $file -o $results/v509-out.json" >/dev/null
    v509_rows+=("| $label | $(python3 -c "import json,sys;r=json.load(open(sys.argv[1]))['results'][0];print(f\"{r['mean']:.3f} ± {r['stddev']:.3f}\")" "$results/v509.json") |")
  done
fi

cpu_model=$(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2- | sed 's/^ //' || uname -m)
memory=$(awk '/MemTotal/ {printf "%.1f GB", $2 / 1e6}' /proc/meminfo 2>/dev/null || echo unknown)
os=$( (. /etc/os-release 2>/dev/null && echo "$PRETTY_NAME") || uname -s)

{
  echo "# Benchmarks"
  echo
  echo "Generated by \`scripts/bench/run.sh$([ "$quick" = 1 ] && echo ' --quick')\` on $(date -u +%Y-%m-%d)."
  if [ "$quick" = 1 ]; then
    echo
    echo "> **Quick mode**: $runs runs per measurement and a ×$factor scale-up, on a constrained"
    echo "> machine. Run \`scripts/bench/run.sh\` (10 runs, ×1,000 ≈ 2.6 million sequences) on the"
    echo "> hardware you care about for publishable numbers."
  fi
  echo
  echo "## Setup"
  echo
  echo "| | |"
  echo "|---|---|"
  echo "| CPU | $cpu_model ($cpus logical CPUs) |"
  echo "| Memory | $memory |"
  echo "| Operating system | $os, kernel $(uname -r) |"
  echo "| dima | $("$dima" --version) |"
  echo "| Rust | $(rustc --version 2>/dev/null || echo unknown) |"
  echo "| hyperfine | $(hyperfine --version) |"
  echo
  echo "Wall time is the mean ± standard deviation of $runs runs after one warm-up run"
  echo "(hyperfine); peak memory is the maximum resident set size of one run (GNU time)."
  echo "Each run writes the complete JSON results. Datasets: the paper's alignments in"
  echo "\`samples/\` and \`tests/data/\` (see \`tests/data/README.md\`) and deterministic synthetic"
  echo "alignments from \`scripts/bench/make_data.py\`. Threads: all $cpus CPUs unless stated."
  echo
  echo "## Datasets"
  echo
  echo "| Dataset | Sequences | Options | Wall time (s) | Min (s) | Max (s) | Peak memory (MB) |"
  echo "|---|---|---|---|---|---|---|"
  printf '%s\n' "${table_rows[@]}"
  echo
  echo "## Thread scaling"
  echo
  echo "HIV-1 pol DNA ×$factor (zstd), $(printf "%'d" "$(records "$data/hiv_x$factor.fasta.zst")") sequences."
  echo
  echo "| Threads | Wall time (s) | Speed-up |"
  echo "|---|---|---|"
  base=""
  for row in "${scaling_rows[@]}"; do
    read -r t mean sd <<< "$row"
    [ -n "$base" ] || base=$mean
    printf '| %s | %.3f ± %.3f | %.2f× |\n' "$t" "$mean" "$sd" "$(python3 -c "print($base / $mean)")"
  done
  if [ "${#v509_rows[@]}" -gt 0 ]; then
    echo
    echo "## DiMA v5.0.9"
    echo
    echo "The published implementation (\`pip install dima-cli\`), default options."
    echo
    echo "| Dataset | Wall time (s) |"
    echo "|---|---|"
    printf '%s\n' "${v509_rows[@]}"
  fi
  echo
  echo "## Reproduce"
  echo
  echo '```sh'
  echo "cargo build --release --locked -p dima"
  echo "scripts/bench/run.sh$([ "$quick" = 1 ] && echo ' --quick')"
  echo '```'
  echo
  echo "Micro-benchmarks of the reader, the k-mer engine and the entropy estimators:"
  echo "\`cargo bench -p dima-core\`."
} > "$out"
echo "wrote $out" >&2
