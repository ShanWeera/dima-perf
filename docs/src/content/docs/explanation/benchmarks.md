---
title: Benchmarks
description: How DiMA's speed and memory use are measured, and where the current measurements are published.
---

DiMA's benchmarks are measured, not estimated, by one script, so that anyone can repeat them on
their own hardware [@weber2019; @mangul2019bench]. `BENCHMARK.md` in the repository records the
last run, with its date, mode and machine:

```sh
scripts/bench/run.sh            # full run: ≥ 10 repetitions per measurement
scripts/bench/run.sh --quick    # fewer repetitions and smaller data, marked as such
```

The script measures wall time with `hyperfine` and peak memory with GNU `time` on the datasets
committed to the repository, on synthetic scale-ups (millions of sequences, a long genome, a
highly diverse alignment), on every input compression format, and across thread counts; it can
also run DiMA v5.0.9 (`--v509`) for comparison. It writes `BENCHMARK.md` at the root of the
repository with the hardware, operating system, software versions and the exact commands.

:::note[Where the numbers are]
The current measurements are in
[`BENCHMARK.md`](https://github.com/ShanWeera/dima-perf/blob/master/BENCHMARK.md) in the
repository. This page does not copy them, so it can never disagree with the report.
:::

For your own runs, `dima analyze -v` prints the time spent in each phase and the peak memory,
and `--run-report` records them in a JSON file (see
[Make results reproducible](/how-to/reproducibility/)).

## References

[^ref]
