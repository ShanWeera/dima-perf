---
title: Testing
description: How DiMA is tested — golden files, an independent reference implementation, the paper's datasets, property-based tests, schema contracts and end-to-end tests of the command line.
---

Scientific software has an oracle problem: the right answer is often not known in advance
[@kanewala2014]. DiMA's tests attack it from several sides; the tests of the library and the
command line run in continuous integration on Linux, macOS and Windows.

## The reference implementation

`scripts/reference/dima_reference.py` is an independent implementation of DiMA in Python,
standard library only. It is deliberately simple and slow, computes in high precision, and
states every rule in the docstring of the function that implements it. It is a test oracle, not
the product.

```sh
python3 scripts/reference/dima_reference.py --self-test
python3 scripts/reference/dima_reference.py analyze -i samples/mers_spike_aa.fasta -o ref.json
```

The differential tests (`crates/dima-core/tests/reference.rs`) run it and the library on the same
inputs. Every number must agree within a relative $10^{-13}$ (so a zero must be exactly zero),
and everything else — labels, k-mers, motif classes, methods, region membership — must be
identical. The test alignments are small, and there the two agree to $2.6 \times 10^{-15}$ at
worst. At millions of sequences per position the library's own rounding is larger, so
differences up to $5 \times 10^{-11}$ are expected there (see
[Numerical accuracy](/explanation/numerical-accuracy/#what-this-means-for-testing)).
The tests need `python3` 3.10 or newer; `DIMA_SKIP_REFERENCE=1` skips them where Python is
unavailable (never in CI).

## The paper's datasets

`crates/dima-core/tests/paper.rs` analyses the four committed datasets (`tests/data/`, with their
sources and checksums in its README) and checks the values of
[Comparison with the paper](/explanation/paper-comparison/): position counts, support classes,
means, extrema with their positions and ties, the MERS-CoV position 25 of Fig. 1C, and the
conserved regions with their flags and bounds. Its numbers are compared with the same rule as
the differential tests (a relative $10^{-13}$), defined once in
`crates/dima-core/tests/common/mod.rs`. The entropies it expects are the reference
implementation's output on those datasets, written to `crates/dima-core/tests/paper_expected/`
by a script and never edited by hand; continuous integration checks that they are current:

```sh
python3 scripts/reference/paper_values.py           # regenerate after changing the reference
python3 scripts/reference/paper_values.py --check   # what CI runs (about 45 s)
```

## Golden files

`crates/dima/tests/golden.rs` runs the `dima` binary on fixed cases and compares **every file it
writes byte for byte** with `tests/golden/<case>/` — JSON (compact and pretty), JSON Lines, the
four tables and the conserved sequences — with two thread counts, on every platform. After an
intended change, re-bless the files and review the difference before committing it:

```sh
DIMA_BLESS=1 cargo test -p dima --test golden
git diff tests/golden/
```

`.gitattributes` keeps these files byte-exact on every platform.

## Invariance and properties

- `invariance.rs`: the results are identical whatever the thread count, batch size, line wrapping,
  line endings, letter case, byte-order mark or compression of the input.
- `properties.rs`: property-based tests [@claessen2000] generate random alignments and check that
  the streaming counter equals a naive counter for every k, thread count and batch split, and that
  every result satisfies the definitions (counts add up to the support, incidences to 100%,
  motif rules).
- Unit tests next to the code check the edge cases of each rule: support labels at $T - 1$, $T$
  and $T + 1$, every entropy fallback, the expected subsample entropy against brute-force
  enumeration of all multinomial outcomes, every byte of every alphabet, header cleaning,
  conserved-region merging, exact thresholds.

## Contracts and end-to-end tests

- `schema.rs`: every output validates against its JSON Schema (`schemas/`), and the schemas
  against the Draft 2020-12 meta-schema.
- `output.rs`: JSON round trips, JSON Lines, the tables, compression.
- `crates/dima/tests/cli.rs`: the binary end to end — exit codes, standard input, output rules,
  run reports, messages.
- Interruption tests send real signals and check that the process ends by the signal and leaves
  no temporary files, including as process 1 of a container.
- The desktop app has its own tests: the pure state machine, and interface flows run headless.

## Documentation

`cargo xtask gen --mode check` fails when a generated page or citation file is out of date. The
documentation build fails on a broken internal link, an unknown citation key or a TeX error;
`pnpm -C docs run test:a11y` scans every page for WCAG 2.2 AA violations in both themes, and
`pnpm -C docs run test:lighthouse` enforces the performance, accessibility, best-practice and
search budgets of `docs/lighthouserc.cjs`.

## References

[^ref]
