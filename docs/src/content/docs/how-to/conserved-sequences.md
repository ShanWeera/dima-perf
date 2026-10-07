---
title: Find conserved sequences
description: Report historically conserved sequences during an analysis or from existing results, choose the threshold, and interpret flagged regions and their confidence bounds.
---

Historically conserved sequences (HCS) are stretches of the alignment whose most common k-mer is
carried by (almost) every sequence; see
[Historically conserved sequences](/explanation/conserved-sequences/) for the method.

## During an analysis

```sh
dima analyze -i spike.fasta -o spike.json --hcs-output spike.hcs.json
```

The regions are also written as `<name>.hcs.tsv` by `--tsv-dir`, and as a single table by
`-o FILE.tsv --table hcs`. `--hcs-output` takes `.json` or `.tsv`, optionally followed by `.gz`
or `.zst`.

The default threshold is the paper's, 100%: positions where every sequence with a valid k-mer
carries the Index [@tharanga2025]. Set another with
`--hcs-min-incidence`, a value above 50 and at most 100 (decimals allowed, compared exactly):

```sh
dima analyze -i hiv.fasta -o hiv.json --hcs-output hiv.hcs.tsv --hcs-min-incidence 70
```

## From existing results

Regions are computed from the results, so you can explore thresholds without reanalysing the
alignment:

```sh
dima hcs -i spike.json                                # 100%, JSON on standard output
dima hcs -i spike.json --hcs-min-incidence 95 -o spike.hcs95.tsv
dima hcs -i spike.json.zst --hcs-min-incidence 90 --hcs-eligible-only -O tsv
```

`dima hcs` prints how many regions it found on standard error.

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
the lowest support among its member positions; the lowest exact 95% Clopper–Pearson lower bound
of their conservation [@clopper1934]; and whether it is **flagged** because positions in its span
have support at or below $T$. The JSON output adds every member position with its k-mer, count,
support, incidence and bound, and the list of low-support positions; see
[JSON outputs](/reference/json/#conserved-sequences).

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
  every member position, which is why lower values are refused.
- Report the threshold and whether eligible-only was used; both are recorded in the output
  (`parameters`).

Examples on the paper's datasets are in
[Historically conserved sequences](/explanation/conserved-sequences/#examples).

## References

[^ref]
