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

`--mem=8G` is an example, not a requirement: DiMA's memory grows with the diversity of the
alignment, not with its size alone. Run the analysis once with `-v` (or `--run-report`), read its
peak memory, and request that with a margin; see
[Memory](/how-to/large-alignments/#memory) for what it depends on and typical values.

### Many alignments

DiMA parallelises within one alignment, so run alignments one after another with all threads, or
several at once with fewer threads each (`-t`). Only `--tsv-dir` creates its folder, so create
the folder of the other outputs first:

```bash
mkdir -p results
for f in alignments/*.fasta.zst; do
  name=$(basename "$f" .fasta.zst)
  dima analyze -i "$f" -o "results/$name.json.zst" --run-report "results/$name.run.json" -q
done
```

The loop goes on after a failed alignment; every run, failed or not, leaves its run report. To
list the ones that did not complete, with their exit status and error:

```bash
jq -r 'select(.status != "completed") | "\(.command[3]): \(.status), exit \(.exit_code): \(.error)"' results/*.run.json
```

```text
broken.fasta.zst: failed, exit 4: broken.fasta.zst: only 3 residue letters in the first record; that is too few to tell protein from nucleotide sequences
hint: pass --alphabet protein, --alphabet dna or --alphabet rna
```

### A Slurm array job

An array job runs one task per alignment, each with its own CPUs, which DiMA reads from the
task's allocation. Set `--array` to `0-` the number of alignments minus one:

```bash
#!/bin/bash
#SBATCH --job-name=dima
#SBATCH --array=0-2
#SBATCH --cpus-per-task=4
#SBATCH --mem=2G
#SBATCH --time=01:00:00
set -euo pipefail

files=(alignments/*.fasta.zst)
f=${files[$SLURM_ARRAY_TASK_ID]}
name=$(basename "$f" .fasta.zst)
mkdir -p results
dima analyze -i "$f" -o "results/$name.json.zst" --run-report "results/$name.run.json" -v
```

Each task's log (Slurm's `slurm-JOB_TASK.out`) then ends with its summary line, and the `jq`
command above lists the tasks that failed.

## Logs

Messages go to standard error and data to standard output or files, so `2> dima.log` captures
the messages alone. The redrawn progress line appears only on terminals; in a batch log, `-v`
(or `--progress plain`) prints one line per phase and the progress at most every 10 seconds and
at least once a minute, with the elapsed time, and `-q` limits the output to errors. The
[summary line](/reference/glossary/#summary-line) is always the last line, after the `-v` report.
Warnings are printed once per kind of problem, with counts and examples. Those about the data
(the library's diagnostics) are also stored in the results (`run.diagnostics`); the command
line's own warnings — `threads_exceed_cpus`, `threads_refused`, `flag_has_no_effect`,
`output_format_assumed` and, from `dima view`, `newer_schema` — are only printed (and counted
in the summary line of `dima analyze`), so keep the log if you need them.

## Exit codes and interruptions

| Status | Meaning |
|---|---|
| 0 | success |
| 2 | invalid options (fix the command) |
| 3 | a file could not be read or written |
| 4 | invalid input data |
| 1 | internal error (please report it) |
| 128 + N | interrupted by signal N: 143 SIGTERM, 130 SIGINT, 129 SIGHUP; on Linux 138 SIGUSR1, 140 SIGUSR2, 142 SIGALRM, 152 SIGXCPU (on macOS 158 SIGUSR1, 159 SIGUSR2) |

When a scheduler stops a job — at its time limit Slurm sends SIGTERM and, after a grace period,
SIGKILL; Grid Engine can warn first with SIGUSR1 or SIGUSR2 — DiMA removes its unfinished
temporary files within a few seconds and ends by the same signal, so the job ends with the
conventional status and no partial output file is left. Re-run the job to produce the results.
Standard output is the exception: it is written as it is produced, so with `> results.json` an
interrupted run can leave a partial file. Use `-o results.json`, which is written to a temporary
file and moved into place only when complete. See [Exit codes](/reference/exit-codes/).

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

A Snakemake rule that keeps the run report and the messages as logs, which Snakemake keeps when
a job fails:

```python
rule dima:
    input:
        "alignments/{name}.fasta.zst"
    output:
        results="results/{name}.json.zst",
        tables=directory("results/{name}_tables"),
    log:
        report="logs/{name}.run.json",
        messages="logs/{name}.log",
    threads: 4
    shell:
        "dima analyze -i {input} -o {output.results} --tsv-dir {output.tables} "
        "--run-report {log.report} -t {threads} -v 2> {log.messages}"
```

Snakemake creates the `results` and `logs` folders; DiMA creates the tables folder, and removes
it again when the run fails. `snakemake -c 8 results/mers.json.zst` runs it for
`alignments/mers.fasta.zst`; for a job that failed, `logs/NAME.run.json` holds its status, exit
code and error, and `logs/NAME.log` its messages.

## References

[^ref]
