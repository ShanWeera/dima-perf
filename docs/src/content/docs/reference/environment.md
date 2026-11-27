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
| `DIMA_LOG` | Log filter in the [`tracing`](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html) syntax, for debugging: for example `DIMA_LOG=dima_core=debug` or `DIMA_LOG=debug`. When set, even to an empty value, it replaces the log level chosen by `-q`/`-v` (warnings, diagnostics, errors and the summary line are not logs and still follow `-q`). A part of the filter that is not valid is skipped with a line such as ``ignoring `=[`: invalid filter directive``, and the run goes on. `RUST_LOG` is not read. |
| `DIMA_PROGRESS` | `auto`, `plain` or `off`: how `dima analyze` shows progress when `--progress` is not given (see [the order](#progress-the-order) below). `auto` shows one line redrawn in place on a terminal that can redraw a line (`TERM` set and not `dumb` on Unix), and elsewhere nothing, or plain lines with `-v`. `plain` appends a line per phase and the progress at most every 10 seconds and at least once a minute; set `DIMA_PROGRESS=plain` in your shell profile if you use a screen reader, which reads every update of a redrawn line. `off` shows nothing. The value is lower case, as option values are. An empty value counts as unset; any other value stops the run with a [usage error](/reference/exit-codes/) (exit 2). Only `dima analyze` shows progress, so the other commands ignore the variable. |
| `DIMA_NO_PROGRESS` | Any value except an empty one, `0`, `false`, `no`, `off`, `n` or `f` (in any letter case) means `off`, unless `--progress` is given (the older spelling, still accepted). |

### Progress: the order

`dima analyze` decides how to show progress in this order; the first that applies wins:

1. `--progress auto|plain|off`, or the older `--no-progress` (the same as `--progress off`):
   whichever of the two comes last on the command line;
2. `DIMA_NO_PROGRESS` set to a true value: `off`;
3. `DIMA_PROGRESS`, unless it is empty;
4. `auto`.

`-q` hides progress in every case. With `-vv` (debugging messages), `auto` prints plain lines
on a terminal too, because debugging messages would break a line that is redrawn in place.

Since 1.2.0 an explicit `--progress` wins over `DIMA_NO_PROGRESS` (before, the variable won), an
empty `DIMA_PROGRESS` counts as unset (before, it was an error), and an invalid `DIMA_PROGRESS`
gives the message below and exit 2:

```text
error: DIMA_PROGRESS is set, but not to a progress mode (auto, plain or off)
  hint: set it to auto, plain or off, in lower case, or unset it
```

The message does not repeat the value: messages are also recorded in
[run reports](/reference/glossary/#run-report), which never hold the values of environment
variables. Run `echo "$DIMA_PROGRESS"` to see it.

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
the results. Besides the worker threads, DiMA runs a thread that opens the input (and ends once
its first bytes are read, so that an interruption is noticed even while the input is still
empty), a reader thread and a checksum thread, which mostly wait for input, and the command line
runs an interrupt handler. Those start first. If the
system then refuses some of the worker threads (a limit on processes and threads, such as
`ulimit -u` or a container's pids limit), DiMA continues with as many workers as the system
allows and warns (`threads_refused`); it stops, with [exit status 3](/reference/exit-codes/),
only when it cannot start the thread that opens the input, the reader, the checksum thread,
the interrupt handler or a single worker.

## Colour

Colour in messages and help texts is decided as follows:

1. `--color always` or `--color never`, if given;
2. otherwise (`auto`): no colour if `NO_COLOR` is set to a non-empty value; colour if
   `FORCE_COLOR` or `CLICOLOR_FORCE` is set to a non-empty value (any value, `0` included); no
   colour if `CLICOLOR` is `0`;
3. otherwise colour only when the output is a terminal that supports it (`TERM` set and not
   `dumb`).

Colour is never the only signal: errors, warnings and notes are also labelled in text
(`error:`, `warning[…]:`, `note:`).

## Others

| Variable | Effect |
|---|---|
| `RUST_BACKTRACE` | `1` adds a backtrace to the report of an internal error (exit 1). |
| `TMPDIR` | Not used for outputs: temporary files are created next to their destination, so they can be moved into place atomically. |

The desktop app does not read DiMA's variables or the scheduler variables: its thread count is
the **Worker threads** setting under **Advanced** (default: all CPUs but one).
