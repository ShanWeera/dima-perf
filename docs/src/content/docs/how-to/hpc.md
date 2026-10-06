---
title: Run on clusters and in pipelines
description: Use DiMA in batch jobs and workflows — thread counts under Slurm, PBS, SGE and LSF, logs, exit codes, interruptions, run reports, nohup and containers.
---

DiMA is a single binary with no runtime dependencies, so it runs on login and compute nodes as it
does on a laptop. This guide covers what changes in batch jobs and pipelines.

## Install on a cluster

Use the Linux archive for your architecture; the `x86_64-unknown-linux-gnu` build runs on systems
as old as CentOS 7 (glibc 2.17), and the static `musl` builds run anywhere. Put `dima` in a
directory on your `PATH`, for example `~/.local/bin`. See [Install DiMA](/getting-started/install/).
No root access is needed.

## Threads

By default DiMA uses the CPUs your job was granted on the node, not all the CPUs of the node, so
it does not overload shared nodes:

| Scheduler | Request | DiMA uses |
|---|---|---|
| Slurm | `#SBATCH --cpus-per-task=8` | 8 (`SLURM_CPUS_ON_NODE` in the batch script, `SLURM_CPUS_PER_TASK` inside `srun`) |
| PBS Pro | `#PBS -l select=1:ncpus=8` | 8 (`NCPUS`) |
| Torque | `#PBS -l nodes=1:ppn=8` | 8 (`PBS_NUM_PPN`) |
| SGE | `#$ -pe smp 8` | 8 (`NSLOTS`; on several hosts, this host's slots in `PE_HOSTFILE`) |
| LSF | `#BSUB -n 8` with `-R "span[hosts=1]"` | 8 (`LSB_DJOB_NUMPROC`) |

`--threads N` or `RAYON_NUM_THREADS=N` override this. `OMP_NUM_THREADS` is ignored (DiMA does not
use OpenMP). With `-v`, DiMA prints the thread count and the variable it came from. The exact
rules are in [Environment variables](/reference/environment/#threads). The thread count never
changes the results, and memory use does not grow with it. If the node limits processes and
threads below what you asked for (`ulimit -u`, a container's pids limit), DiMA runs with the
worker threads it can start and warns (`threads_refused`); the run report records how many
started.

### A Slurm job

```bash
#!/bin/bash
#SBATCH --job-name=dima
#SBATCH --cpus-per-task=8
#SBATCH --mem=8G
#SBATCH --time=02:00:00
set -euo pipefail

dima analyze -i alignment.fasta.zst -o results.json.zst --tsv-dir tables \
    --hcs-output conserved.tsv --run-report run-report.json -v
```

### Many alignments

DiMA parallelises within one alignment, so run alignments one after another with all threads, or
several at once with fewer threads each (`-t`):

```bash
for f in alignments/*.fasta.zst; do
  name=$(basename "$f" .fasta.zst)
  dima analyze -i "$f" -o "results/$name.json.zst" --run-report "results/$name.run.json" -q
done
```

In a Slurm array job, give each task its own alignment and let DiMA read the task's CPUs.

## Logs

Messages go to standard error and data to standard output or files, so `2> dima.log` captures
the messages alone. The redrawn progress line appears only on terminals; in a batch log, `-v`
(or `--progress plain`) prints one line per phase and the progress at most every 10 seconds and
at least once a minute, with the elapsed time, and `-q` limits the output to errors. Warnings are
printed once per kind of problem, with counts and examples, and are also stored in the results
(`run.diagnostics`).

## Exit codes and interruptions

| Status | Meaning |
|---|---|
| 0 | success |
| 2 | invalid options (fix the command) |
| 3 | a file could not be read or written |
| 4 | invalid input data |
| 1 | internal error (please report it) |
| 143, 130, 129 | interrupted by SIGTERM, SIGINT, SIGHUP |

When a scheduler stops a job — at its time limit Slurm sends SIGTERM and, after a grace period,
SIGKILL — DiMA removes its unfinished temporary files within a few seconds and ends by the same
signal, so the job ends with the conventional status and no partial output is left. Re-run the
job to produce the results. See [Exit codes](/reference/exit-codes/).

## Run reports

`--run-report FILE` writes a small JSON file describing the run: version and build, the
equivalent command, status and exit code, start and end time, threads and their source, time per
phase, CPU time, peak memory, and the size and SHA-256 checksum of the input and every output.
It is written even when the run fails or is interrupted, so it is useful for accounting and
debugging jobs. It contains no user or host names, no directories and no environment values. See
[Make results reproducible](/how-to/reproducibility/#keep-a-run-report).

## nohup, screen and background jobs

Under `nohup` (or any parent that ignores SIGHUP), DiMA keeps ignoring SIGHUP, so closing the
terminal does not stop it:

```sh
nohup dima analyze -i big.fasta.zst -o big.json.zst -v > dima.log 2>&1 &
```

`tmux` and `screen` work as usual.

## Containers

DiMA needs nothing but the binary, so it fits in any image, including minimal ones (use the
static `musl` build for Alpine or `scratch`-like images). Two details:

- When `dima` is process 1 of the container (`docker run image dima …` without `--init`), a
  SIGTERM from `docker stop` still ends it promptly, with status 143, after removing its temporary
  files.
- Mount the input and output directories; outputs are written next to their destination before
  being renamed into place, so the output directory must be writable.

A Bioconda package, and with it BioContainers images for Docker and Apptainer/Singularity
[@gruning2018bioconda; @daveigaleprevost2017; @kurtzer2017], is planned; until the recipe is
accepted, use the release archives.

## Workflow managers

In Snakemake or Nextflow rules, give DiMA the task's CPUs explicitly (`-t {threads}` or
`-t ${task.cpus}`) — workflow managers do not always run tasks under a scheduler allocation that
DiMA can read — and use `-q` or `-v` as you prefer for the task log. Outputs are complete or
absent, never partial, so a failed task never leaves a file a later rule could mistake for a
result.

## References

[^ref]
