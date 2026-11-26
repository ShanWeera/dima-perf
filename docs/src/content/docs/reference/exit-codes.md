---
title: Exit codes
description: The exit status of every dima command, what each means and how scripts and workflow managers can use it.
---

Every `dima` command ends with one of these exit statuses:

| Status | Meaning | Examples |
|---|---|---|
| `0` | Success | Also when a program reading standard output stops early (`dima view … \| head`): the broken pipe is not an error |
| `1` | Internal error — a bug in DiMA | The message asks you to report it; `RUST_BACKTRACE=1` adds a backtrace |
| `2` | Usage error: invalid options | An unknown option; `-k 0`; `--hcs-min-incidence 50`; `-O tsv` without `--table`; `-O` that contradicts the file extension; two outputs to the same file; `k` longer than the alignment; `RAYON_NUM_THREADS=abc`; the input is a directory |
| `3` | Input/output or system error | The input file does not exist or cannot be read; an output cannot be created or written (permissions, full disk); the system refused a thread the run cannot do without (the reader, the checksum thread, the interrupt handler or the first worker thread; refused *additional* worker threads only lower the thread count, with a warning) |
| `4` | Invalid input data | Not FASTA (or FASTQ); unequal sequence lengths; a character outside the alphabet in strict mode; a header with the wrong number of fields in strict mode; corrupt or truncated compressed data; an alphabet that cannot be detected; results of another major schema version, or of a newer minor version with values this version does not know, given to `view`, `hcs` or `cite`; a results file that was edited or damaged, for example a similarity-weighted value at a position without support, given to the same commands |
| `130` | Interrupted by SIGINT (Ctrl-C) | |
| `143` | Terminated by SIGTERM | A batch scheduler's time limit, `kill`, `docker stop` |
| `129` | Terminated by SIGHUP | The terminal or SSH session closed (not under `nohup`, which makes DiMA ignore it) |

Errors are printed on standard error as `error:` followed by the problem, where it occurred
(file, record, line, column and byte when they apply), `caused by:` lines for the underlying
system error, and a `hint:` with the fix. They are printed even with `-q`.

## Interruptions

On the first SIGINT, SIGTERM or SIGHUP, DiMA stops, removes the temporary files of outputs it
had not finished, and ends **by the same signal**, so a shell or workflow manager sees the
conventional status 128 + signal number (130, 143, 129). Each output is written to a temporary
file next to its destination and moved into place only when it is complete, so an interruption
never leaves a partial file: a destination is either left as it was or, if the signal arrived
while the finished outputs were being moved into place at the very end of the run, complete. If
cleaning up takes more than 3 seconds, or a second signal arrives, DiMA removes the temporary
files and ends at once.

When DiMA runs as process 1 of a container (for example `docker run` without an init process),
the kernel does not let a signal end it by default; DiMA then exits with the same status
(128 + signal number) explicitly.

On Unix, the other signals that batch systems send before a limit is enforced are handled like
SIGTERM, ending DiMA by the same signal (128 + its number): SIGUSR1 and SIGUSR2 (Grid Engine
warns with them before a run-time limit), SIGXCPU (a CPU-time limit) and SIGALRM. A file-size
limit (`ulimit -f`) makes the write fail with status 3 instead of killing the process. SIGKILL
(the out-of-memory killer, or a scheduler's hard limit) cannot be handled: it can leave a hidden
temporary file named `.NAME.dima-PID-N.tmp` next to an output, which can be deleted. SIGQUIT
(Ctrl-\\) keeps its default action, a core dump for debugging.

On Windows, Ctrl-C and Ctrl-Break give 130, closing the console window gives 129, and logging off
or shutting down gives 143.

A run report (`--run-report`) is still written when a run fails or is interrupted, with its
status (`failed`, `cancelled`) and exit code.

## In scripts

```sh
dima analyze -i alignment.fasta -o results.json
case $? in
  0) echo "done" ;;
  2) echo "fix the command line" ;;
  3) echo "check the files and the disk" ;;
  4) echo "fix the alignment" ;;
  130|143|129) echo "interrupted" ;;
  *) echo "internal error: please report it" ;;
esac
```

With `set -e` (or `set -euo pipefail`), a non-zero status stops the script. Snakemake, Nextflow
and similar workflow managers treat any non-zero status as a failed job, and DiMA never leaves a
partial output that they could mistake for a result.
