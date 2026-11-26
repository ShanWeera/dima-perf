#!/usr/bin/env bash
# Compares dima with the values published in the DiMA paper (Tharanga et al. 2025, Briefings in
# Bioinformatics 26(1):bbae607, doi:10.1093/bib/bbae607) on the paper's datasets, and prints a
# Markdown table (D26). The methodology pages of the documentation explain every difference.
#
# Usage: scripts/validate_paper.sh [path/to/dima]
# Default binary: target/release/dima (built if missing). Needs python3 (standard library only).
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
dima=${1:-$root/target/release/dima}
if [ ! -x "$dima" ]; then
  cargo build --release --locked -p dima --manifest-path "$root/Cargo.toml" >&2
fi
work=$(mktemp -d)
trap 'rm -r -- "$work"' EXIT

run() { # name, input, extra options…
  local name=$1 input=$2
  shift 2
  "$dima" analyze -i "$input" -q -o "$work/$name.json" "$@"
}

data=$root/tests/data
run hiv_dna "$data/HIV1_FLT_2022_pol_DNA.fasta.bz2" --hcs-output "$work/hiv_dna.hcs.json" --hcs-min-incidence 70
run hiv_protein "$data/HIV1_ALL_2020_pol_PRO.fasta.bz2"
run mers_aa_t30 "$root/samples/mers_spike_aa.fasta" -s 30
run mers_aa "$root/samples/mers_spike_aa.fasta"

python3 - "$work" <<'PY'
import json
import sys
from pathlib import Path

work = Path(sys.argv[1])
load = lambda name: json.loads((work / name).read_text())
hiv = load("hiv_dna.json")
hcs = load("hiv_dna.hcs.json")
pro = load("hiv_protein.json")
mers30 = load("mers_aa_t30.json")
mers = load("mers_aa.json")

# The spread of the paper's random procedure (exact), written by scripts/reference/docs_numbers.py.
# <!-- docs-numbers:notes:start -->
RANGE_NOTE = "the extremes of one random run vary (SD 0.0661 bits at positions 2176 and 2177)"
FIG1C_NOTE = "1.38 is 0.29 SD below the exact expectation (SD 0.2241 bits)"
# <!-- docs-numbers:notes:end -->

rows = []
add = lambda *r: rows.append(r)

labels = hiv["support_label_counts"]
add("HIV-1 pol DNA: positions", "3,364", f"{hiv['position_count']:,}", "")
add("HIV-1 pol DNA: LS positions", "564", f"{labels['LS']:,}", "")
add("HIV-1 pol DNA: NS positions", "680", f"{labels['NS']:,}", "")
add("HIV-1 pol DNA: mean entropy (support > T)", "~5.35", f"{hiv['average_entropy']:.4f}",
    "paper: one random run; dima: the exact expectation of the procedure's intercept")
lo, hi = hiv["lowest_entropy"], hiv["highest_entropy"]
add("HIV-1 pol DNA: entropy range", "~0.86 to ~9.57", f"{lo['entropy']:.4f} to {hi['entropy']:.4f}", RANGE_NOTE)
add("HIV-1 pol DNA: highest-entropy position", "2177", f"{hi['position']}",
    "positions 2176 and 2177 differ by less than the random procedure's spread")
eligible = [p for p in hiv["results"] if p["low_support"] is None]
low = min(p["total_variants_incidence"] for p in eligible)
lows = [p["position"] for p in eligible if p["total_variants_incidence"] == low]
add("HIV-1 pol DNA: lowest total variants", "position 3046", f"position {lows[0]} ({low:.3f}%"
    + (f", tied with {', '.join(map(str, lows[1:]))})" if len(lows) > 1 else ")"), "")
regions = ", ".join(f"columns {r['start_column']}–{r['end_column']} ({r['length']} nt, "
                    f"min {r['min_incidence']:.1f}%)" for r in hcs["regions"])
add("HIV-1 pol DNA: conserved sequences at 70%", "2 (36 and 34 nt)", f"{hcs['region_count']}: {regions}",
    "DiMA v5.0.9's own code also gives 32 and 30 nt; the paper's \"one HCS at ~88%\": "
    f"region 1 reaches {max(m['incidence'] for m in hcs['regions'][0]['members']):.1f}%")
add("HIV-1 pol protein: mean entropy (support > T)", "~1.91", f"{pro['average_entropy']:.4f}",
    "stops (*) and frameshifts (#) are excluded, not residues")

def position(results, number):
    return next(p for p in results["results"] if p["position"] == number)

p30 = position(mers30, 25)
p100 = position(mers, 25)
add("MERS spike (Fig. 1C) position 25: support", "170", f"{p30['support']}", "")
add("MERS spike position 25: distinct 9-mers", "10", f"{len(p30['diversity_motifs'])}", "")
index = next(v for v in p30["diversity_motifs"] if v["motif_short"] == "I")
add("MERS spike position 25: Index", "SVKSACIEV 129 (75.88%)",
    f"{index['sequence']} {index['count']} ({index['incidence']:.2f}%)", "")
add("MERS spike position 25: distinct-variants incidence", "22.0%",
    f"{p30['distinct_variants_incidence']:.1f}%", "")
add("MERS spike position 25: entropy (T = 30)", "1.38", f"{p30['entropy']:.4f}", FIG1C_NOTE)
add("MERS spike position 25: entropy (T = 100)", "–", f"{p100['entropy']:.4f}", "")

print("| Quantity | Paper | dima | Note |")
print("|---|---|---|---|")
for r in rows:
    print("| " + " | ".join(r) + " |")
PY
