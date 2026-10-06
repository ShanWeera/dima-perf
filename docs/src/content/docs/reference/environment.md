---
title: Environment variables
description: Every environment variable that changes what dima does — logging, progress, colour, threads and batch-scheduler variables — and the ones it deliberately ignores.
---

Command-line options always take precedence over environment variables. No environment variable
changes the results of an analysis; they change how many threads compute them and what is
printed on standard error.

## DiMA's own

| Variable | Effect |
|---|---|
| `DIMA_LOG` | Log filter in the [`tracing`](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html) syntax, for debugging: for example `DIMA_LOG=dima_core=debug` or `DIMA_LOG=debug`. When set, it replaces the log level chosen by `-q`/`-v` (warnings, diagnostics, errors and the summary line are not logs and still follow `-q`). `RUST_LOG` is not read. |
| `DIMA_PROGRESS` | `auto`, `plain` or `off`: the default of `--progress`. `auto` shows one line redrawn in place on a terminal that can redraw a line (`TERM` set and not `dumb` on Unix), and elsewhere nothing, or plain lines with `-v`. `plain` appends a line per phase and the progress at most every 10 seconds and at least once a minute; set `DIMA_PROGRESS=plain` in your shell profile if you use a screen reader, which reads every update of a redrawn line. `off` shows nothing. |
| `DIMA_NO_PROGRESS` | Any value except an empty one, `0`, `false`, `no`, `off`, `n` or `f` means `--progress off` (the older spelling, still accepted). |

## Threads

The number of worker threads of `dima analyze` is, in order:

1. `--threads N`;
2. `RAYON_NUM_THREADS`, if set: it must be a whole number of at least 1, otherwise the run stops
   with a usage error (exit 2);
3. otherwise the CPUs this process may use (respecting CPU affinity and container limits),
   **limited to the CPUs granted on this host by a batch scheduler**:

| Scheduler | Detected by | CPUs granted |
|---|---|---|
| Slurm, in the batch script or an `salloc` shell | `SLURM_JOB_ID` without `SLURM_STEP_ID` | `SLURM_CPUS_ON_NODE` |
| Slurm, in a job step (for example inside `srun`) | `SLURM_JOB_ID` and `SLURM_STEP_ID` | `SLURM_CPUS_PER_TASK`, else `SLURM_CPUS_ON_NODE` divided by this node's entry of `SLURM_STEP_TASKS_PER_NODE` (node index `SLURM_NODEID`) |
| PBS Pro, Torque, OpenPBS | `PBS_JOBID` | `PBS_NUM_PPN`, else `NCPUS` |
| SGE, Univa/Altair Grid Engine | `PE_HOSTFILE` | the slots of this host in the hostfile (`NSLOTS` when the job runs on one host) |
| LSF | `LSB_MCPU_HOSTS` | this host's count in the list (`LSB_DJOB_NUMPROC` when the job runs on one host) |

The host is identified by `HOSTNAME` or the system host name. A scheduler value that is missing or
malformed is ignored; a grant larger than the CPUs available does not raise the count.
`SLURM_JOB_CPUS_PER_NODE`, `SLURM_NTASKS` and `OMP_NUM_THREADS` are deliberately not used (the
first two describe the whole job, not this task; DiMA does not use OpenMP). With `-v`, DiMA
prints the thread count and where it came from, and notes when `OMP_NUM_THREADS` is lower.

An explicit count above the available CPUs is obeyed, with a warning. The count never changes
the results. Besides the worker threads, DiMA runs a reader thread and a checksum thread, which
mostly wait for input, and the command line runs an interrupt handler. Those start first. If the
system then refuses some of the worker threads (a limit on processes and threads, such as
`ulimit -u` or a container's pids limit), DiMA continues with as many workers as the system
allows and warns (`threads_refused`); it stops, with [exit status 3](/reference/exit-codes/),
only when it cannot start the reader, the checksum thread, the interrupt handler or a single
worker.

## Colour

Colour in messages and help texts is decided as follows:

1. `--color always` or `--color never`, if given;
2. otherwise (`auto`): no colour if `NO_COLOR` is set to a non-empty value; colour if
   `FORCE_COLOR` or `CLICOLOR_FORCE` is set to a non-empty value (any value, `0` included); no
   colour if `CLICOLOR=0`;
3. otherwise colour only when the output is a terminal that supports it (`TERM` set and not
   `dumb`).

Colour is never the only signal: errors, warnings and notes are also labelled in text
(`error:`, `warning[…]:`, `note:`).

## Others

| Variable | Effect |
|---|---|
| `RUST_BACKTRACE` | `1` adds a backtrace to the report of an internal error (exit 1). |
| `TMPDIR` | Not used for outputs: temporary files are created next to their destination, so they can be moved into place atomically. |

The desktop app does not read DiMA's variables or the scheduler variables: its thread count is a
setting (default: all CPUs but one).
