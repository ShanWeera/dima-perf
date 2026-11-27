---
title: Exit codes
description: The exit status of every dima command, what each means and how scripts and workflow managers can use it.
---

Every `dima` command ends with one of these exit statuses:

| Status | Meaning | Examples |
|---|---|---|
| `0` | Success | Also when a program reading standard output stops early (`dima view … \| head`): the broken pipe is not an error, and the warnings and the summary line are still printed |
| `1` | Internal error — a bug in DiMA | The message asks you to report it; `RUST_BACKTRACE=1` adds a backtrace |
| `2` | Usage error: invalid options | An unknown option; `dima` without arguments (it prints the help on standard error); `-k 0`; `--hcs-min-incidence 50`; `-O tsv` without `--table`; `-O` that contradicts the file extension; an output name DiMA cannot write (`.bz2`, two compressions, a run report named `.tsv`); two outputs to the same file, or an output on the input; a folder given as an output file, or a file as `--tsv-dir`; `k` longer than the alignment; `RAYON_NUM_THREADS=abc` or an invalid `DIMA_PROGRESS`; the input is a directory |
| `3` | Input/output or system error | The input file does not exist or cannot be read; an output cannot be created or written (permissions, full disk); the system refused a thread the run cannot do without (the thread that opens the input, the reader, the checksum thread, the interrupt handler or the first worker thread; refused *additional* worker threads only lower the thread count, with a warning) |
| `4` | Invalid input data | Not FASTA (or FASTQ); unequal sequence lengths; a character outside the alphabet in strict mode; a header with the wrong number of fields in strict mode; corrupt or truncated compressed data; an alphabet that cannot be detected; results of another major schema version, or of a newer minor version with values this version does not know, given to `view`, `hcs` or `cite`; a results file that was edited or damaged, for example a similarity-weighted value at a position without support, given to the same commands |
| 128 + N | Interrupted by signal N | See below |

The interruption statuses are 128 plus the number of the signal, which differs between systems
for some signals:

| Status | Signal | Sent by |
|---|---|---|
| `130` | SIGINT | Ctrl-C |
| `143` | SIGTERM | A batch scheduler's time limit, `kill`, `docker stop` |
| `129` | SIGHUP | The terminal or SSH session closed (not under `nohup`, which makes DiMA ignore it) |
| `138` on Linux, `158` on macOS | SIGUSR1 | Grid Engine, ahead of a run-time limit |
| `140` on Linux, `159` on macOS | SIGUSR2 | Grid Engine, ahead of a run-time limit |
| `142` | SIGALRM | A timer, or `kill -ALRM` |
| `152` | SIGXCPU | A CPU-time limit (`ulimit -t`, a scheduler's CPU limit) |

Errors are printed on standard error as `error:` followed by the problem, where it occurred
(file, record, line, column and byte when they apply), `caused by:` lines for the underlying
system error, and a `hint:` with the fix. They are printed even with `-q`, and so is the message
of an interrupted run. [Troubleshooting](/how-to/troubleshooting/) explains each message.

## Interruptions

On the first SIGINT, SIGTERM or SIGHUP, DiMA prints a notice that names it
(`cancelling... (press Ctrl-C again to stop immediately)` for Ctrl-C on a terminal,
`cancelling on SIGTERM... (a second signal stops immediately)` for a signal another program
sent, and `cancelling on SIGINT...` for a SIGINT sent where no terminal is attached; not with
`-q`), stops, removes the temporary files of outputs it had not finished, prints
what became of the outputs, and ends **by the same signal**, so a shell or workflow manager sees
the conventional status 128 + signal number. Each file output is written to a temporary file
next to its destination and moved into place only when it is complete, so an interruption never
leaves a partial file: a destination is either left as it was or, if the signal arrived while
the finished outputs were being moved into place at the very end of the run, complete. The last
message says which:

- `error: cancelled; no results were written` — every file is as it was;
- `error: cancelled; complete: …` (since 1.2.0) — the outputs named are complete, the others
  were not written.

Standard output is the exception: it is written as the output is produced, so an interruption
while DiMA writes to it can leave a partial document for the program reading it, and the message
then ends with `standard output may hold a partial document`. Write to a file with `-o FILE`
rather than `> FILE` when a partial file could be mistaken for a result. If cleaning up takes more
than 3 seconds after the first signal, or a second signal arrives, DiMA removes the temporary
files and ends at once, without the last message.

When DiMA runs as process 1 of a container (for example `docker run` without an init process),
the kernel does not let a signal end it by default; DiMA then exits with the same status
(128 + signal number) explicitly.

On Unix, the other signals that batch systems send before a limit is enforced are handled like
SIGTERM, ending DiMA by the same signal (128 + its number, in the table above): SIGUSR1 and
SIGUSR2 (Grid Engine warns with them before a run-time limit), SIGXCPU (a CPU-time limit) and
SIGALRM. A file-size limit (`ulimit -f`) makes the write fail with status 3 instead of killing
the process. Two signals are not handled and can leave a hidden temporary file named
`.NAME.dima-PID-N.tmp` next to an output, which can be deleted: SIGKILL (the out-of-memory
killer, or a scheduler's hard limit), which cannot be handled, and SIGQUIT (Ctrl-\\, status 131),
which keeps its default action, a core dump for debugging. The run report's temporary file is
created when the run starts, so it is the one most likely to be left.

On Windows, Ctrl-C and Ctrl-Break give 130 (the notice for Ctrl-Break reads
`cancelling on Ctrl-Break... (press Ctrl-Break again to stop immediately)`), closing the
console window gives 129, and logging off or shutting down gives 143.

A run report (`--run-report`) is still written when a run fails or is interrupted, with its
status (`failed`, `cancelled`), its exit code and, after an interruption, the outputs that are
complete. It is not written when its own name is refused, or when it would replace the input or
another output: those are checked first.

## In scripts

```sh
dima analyze -i alignment.fasta -o results.json
status=$?
case $status in
  0) echo "done" ;;
  2) echo "fix the command line" ;;
  3) echo "check the files and the disk" ;;
  4) echo "fix the alignment" ;;
  1) echo "internal error: please report it" ;;
  *) if [ "$status" -gt 128 ]; then
       echo "interrupted by signal $((status - 128))"
     fi ;;
esac
```

With `set -e` (or `set -euo pipefail`), a non-zero status stops the script. Snakemake, Nextflow
and similar workflow managers treat any non-zero status as a failed job, and DiMA never leaves a
partial output file that they could mistake for a result.
