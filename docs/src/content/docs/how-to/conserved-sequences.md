---
title: Find conserved sequences
description: Report historically conserved sequences during an analysis or from existing results, choose the threshold, interpret flagged regions and their confidence bounds, map regions to a reference sequence and count the sequences that carry a whole region.
---

Historically conserved sequences (HCS) are stretches of the alignment whose most common k-mer is
carried by (almost) every sequence; see
[Historically conserved sequences](/explanation/conserved-sequences/) for the method.

## During an analysis

```sh
dima analyze -i spike.fasta -o spike.json --hcs-output spike.hcs.json
```

The regions are also written as `<name>.hcs.tsv` by `--tsv-dir`, and as a single table by
`-o FILE.tsv --table hcs`. The summary line of the run then ends with the number of regions,
such as `44 conserved regions`.

The name given to `--hcs-output` chooses the format: `.json` or `.tsv`, optionally followed by
`.gz` or `.zst`. A name with neither, such as `regions.txt`, gets JSON, the documented default,
with a warning (`output_format_assumed`); a name that asks for JSON Lines (`.jsonl`) or for a
compression DiMA does not write (`.bz2`, `.xz`, …) is refused before anything runs. See
[Names DiMA refuses](/how-to/outputs/#names-dima-refuses).

The default threshold is the paper's, 100%: positions where every sequence with a valid k-mer
carries the Index [@tharanga2025]. Set another with `--hcs-min-incidence`:

```sh
dima analyze -i hiv.fasta -o hiv.json --hcs-output hiv.hcs.tsv --hcs-min-incidence 70
```

The value is a percentage above 50 and at most 100, written as a plain decimal number: `70`,
`95.5`, with at most 9 decimal places. It is compared exactly, without rounding. `95%`, an
exponent such as `1e2`, a value of 50 or less and one above 100 are refused with a message that
says why ([Troubleshooting](/how-to/troubleshooting/#the-conserved-sequence-threshold)).

## From existing results

Regions are computed from the results, so you can explore thresholds without reanalysing the
alignment:

```sh
dima hcs -i spike.json                                # 100%, JSON on standard output
dima hcs -i spike.json --hcs-min-incidence 95 -o spike.hcs95.tsv
dima hcs -i spike.json.zst --hcs-min-incidence 90 --hcs-eligible-only -O tsv
```

`-o` follows the same name rules as `--hcs-output`; `-O json` or `-O tsv` chooses the format
instead of the name (and must agree with it). `dima hcs` ends with a summary line on standard
error, so it is shown even when standard output goes to a file or a pipe (`-q` hides it). For the
HIV-1 *pol* protein alignment:

```text
1 conserved region (1 flagged for low support) with an Index incidence of at least 100%
21 conserved regions (1 flagged for low support) with an Index incidence of at least 90%
21 conserved regions with an Index incidence of at least 90%, eligible positions only
```

The count includes the [flagged regions](#flagged-regions), counted again in brackets;
`eligible positions only` marks a run with `--hcs-eligible-only`.

## Read the regions

```sh
dima hcs -i mers.json -O tsv | head -3
```

```text
region_id	start_column	end_column	length	min_incidence	min_support	conservation_lower_bound_95	flagged	low_support_position_count	sequence
1	7	22	16	100.0	170	0.9785344450190554	false	0	LLMFLLTPTESYVDVG
2	35	83	49	100.0	171	0.9786586254334971	false	0	IQQTFFDKTWPRPIDVSKADGIIYPQGRTYSNITITYQGLFPYQGDHGD
```

For each region: the alignment columns it covers and its sequence; the lowest Index incidence and
the lowest support among its member positions; the lowest per-position exact 95%
Clopper–Pearson lower bound among its members [@clopper1934]; and whether it is **flagged**
because a position from its first to its last member has support at or below $T$. These are
numbers of its member positions, one by one, not of the whole stretch:
[What a region's numbers mean](/explanation/conserved-sequences/#what-a-regions-numbers-mean)
explains the difference. The JSON output adds every member position with its k-mer, count,
support, incidence and bound, and the list of low-support positions; see
[JSON outputs](/reference/json/#conserved-sequences).

The table holds the regions only. The settings that produced them — the threshold, whether
`--hcs-eligible-only` was used — and the run they came from are recorded only in the
conserved-sequences JSON (`parameters` and `source`), so keep the JSON when you keep a table.

## Flagged regions

A region is flagged when it spans positions supported by $T$ sequences or fewer — "100%
conserved" may then mean 3 of 3 sequences. DiMA keeps such regions, as the DiMA lineage did
[@raman2020; @hu2013], but marks them and warns (`hcs_low_support_regions`). Look at
`min_support` and `conservation_lower_bound_95`: a k-mer carried by all of 3 sequences has a
lower bound of 29.2%, by all of 100 sequences 96.4%.

To build regions from well-supported positions only, add `--hcs-eligible-only`: positions with
support ≤ $T$ cannot be members, and they end a region instead of being included in it.

## Choose a threshold

- **100%** (default) asks for complete conservation, the paper's definition.
- Lower thresholds tolerate rare variants; above 50% the most common k-mer is still unique at
  every member position, which is why lower values are refused. Below 100%, a region's sequence
  can be carried whole by far fewer sequences than the threshold suggests: see
  [Count the sequences that carry a whole region](#count-the-sequences-that-carry-a-whole-region).
- Report the threshold and whether eligible-only was used; both are recorded in the
  conserved-sequences JSON (`parameters`), not in the TSV table.

Examples on the paper's datasets are in
[Historically conserved sequences](/explanation/conserved-sequences/#examples).

## Report regions in reference coordinates

A region's `start_column` and `end_column` are alignment columns. To give them as positions of a
reference sequence in the alignment, count that sequence's residues up to the column, leaving out
its gaps. This `awk` command does it for the first record whose header contains `HXB2` and column
3046, the start of the first region of the HIV-1 *pol* DNA alignment at 70%
(`tests/data/HIV1_FLT_2022_pol_DNA.fasta.bz2` in the repository, decompressed):

```sh
awk -v ref=HXB2 -v col=3046 '
  /^>/ { if (seq != "") exit; take = index($0, ref) > 0; next }
  take { gsub(/[ \t\r]/, ""); seq = seq $0 }
  END { n = 0; for (i = 1; i <= col; i++) if (substr(seq, i, 1) !~ /[-._~?]/) n++; print n }
' hiv_pol_dna.fasta
```

```text
2692
```

Column 3046 is the 2,692nd nucleotide of the HXB2 record, which has 354 gap columns before it;
with `col=3077`, the end of the region, the command prints 2723. Set `ref` to a text that only the
reference's header contains, and `col` to each column you need. The result counts from the first
column of the alignment: if the alignment starts inside a gene or genome, add the reference
position of its first column, minus 1, to get the position in the whole gene or genome. The
command reads the alignment as plain text; decompress it first (`bzip2 -dc FILE.bz2 > FILE`).

## Count the sequences that carry a whole region

A region's numbers describe its member positions one by one: at 95%, each member's Index k-mer
is carried by at least 95% of the sequences with a valid k-mer there. A sequence can carry the
Index at most positions and still differ somewhere in the stretch, so the share of sequences
that carry the **whole** region sequence can be much lower. DiMA does not compute it: regions are
built from the results file, which holds counts, not the sequences, and computing it during the
analysis would mean keeping every sequence until the regions are known — gigabytes for millions
of genomes — where DiMA reads the alignment once and keeps only the k-mer counts. Count it from
the alignment instead.

For region 3 of the MERS-CoV spike sample at 95% (columns 195 to 458):

```sh
dima analyze -i mers_spike_aa.fasta -o mers.json
dima hcs -i mers.json --hcs-min-incidence 95 -O tsv | awk -F'\t' '$1 == 3 { print $10 }' > region3.txt
awk -v s=195 -v e=458 -v r="$(cat region3.txt)" '
  function check() {
    if (seq == "") return
    x = toupper(substr(seq, s, e - s + 1))
    if (x !~ /[^ACDEFGHIKLMNPQRSTVWYUO]/) { complete++; if (x == r) carry++ }
    seq = ""
  }
  /^>/ { check(); next }
  { gsub(/[ \t\r]/, ""); seq = seq $0 }
  END { check(); print carry " of " complete }
' mers_spike_aa.fasta
```

```text
107 of 166
```

Of the 166 sequences that are complete over the region — residues only, no gap, missing
character or ambiguity code there — 107 (64.5%) carry the whole region sequence, although its
member positions have an Index incidence of at least 95.0%. Set `s` and `e` to the region's
columns, and the region number in the first command. For a nucleotide alignment, replace the
letters in the brackets by `ACGTU`, so that only sequences with plain nucleotides count as
complete. At 100%, every sequence that is complete over a region carries it, so the count is
then "N of N".

## References

[^ref]
