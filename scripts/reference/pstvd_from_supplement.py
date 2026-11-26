#!/usr/bin/env python3
"""Write the PSTVd loop-27 test alignments from Wu et al. 2024's S2 Table (stdlib only).

Wu J, Zhang Y, Nie Y, Yan F, Zirbel CL, Bisaro DM. "RNA three-dimensional structure drives the
sequence organization of potato spindle tuber viroid quasispecies". PLoS Pathog
2024;20(4):e1012142, doi:10.1371/journal.ppat.1012142, CC BY 4.0. Its S2 Table
(``journal.ppat.1012142.s002.xlsx``; Europe PMC PMC11020406, supplementary files) lists the
unique sequences of each sequencing library with their read counts. Sheet "Loop27" holds the
saturation-mutagenesis pool of loop 27 (wild type UUUUCA, mutated to UNNNNA; written in DNA
letters, TTTTCA) and the populations it gave in three plants each: the inoculated region
(IR-1..3), the margin of the inoculated leaves (LM-1..3) and systemic leaves (Sys-1..3). Wu 2025
(Virus Evol 11(1):veaf029, Section 3.2 and Fig. 5c) computes similarity-weighted entropy on these
libraries.

The sheet has one block of five columns per library: the library's name in row 1, then "Unique
sequences", the read numbers (headed "Reads per thousand" from IR-3 on, although they are whole
read numbers), "Reads per thousand" and "Cutoff Score"; rows 2 onwards list one sequence each.
The converter checks every row: each sequence is 6 letters of A, C, G, T and appears once in its
library, each read number is a positive whole number, and each per-thousand value equals
1000 x reads / T for one whole number T per library, which confirms that the column holds the
read numbers. T is the library's total of S1 Table (for example 73,628 for the pool); the reads
the sheet lists add up to between 3 fewer and 2 more (73,627 for the pool). The converter prints
both. Any other content fails the conversion.

Each unique sequence is written once per read, so that the 6-mer counts of a run at k = 6 are
the read counts: ``pstvd_loop27_<library>.fasta`` (library pool, ir1 .. ir3, lm1 .. lm3,
sys1 .. sys3). Every record of a sequence is named ``SeqN``, N being its place in the library's
list (1 for row 2), so a file compresses to the size of the list. The output is deterministic.

Usage: python3 pstvd_from_supplement.py journal.ppat.1012142.s002.xlsx OUTPUT_DIR
"""
import math
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from xlsx_rows import column_index, column_letters, sheet_rows  # noqa: E402  (this directory, also under python3 -I)

SHEET = "Loop27"
LIBRARIES = {
    "Pool": "pool", "IR-1": "ir1", "IR-2": "ir2", "IR-3": "ir3", "LM-1": "lm1", "LM-2": "lm2",
    "LM-3": "lm3", "Sys-1": "sys1", "Sys-2": "sys2", "Sys-3": "sys3",
}
LENGTH = 6
# The headers of the four columns after a library's name; the read numbers are mislabelled
# "Reads per thousand" from IR-3 on.
HEADERS = ("Unique sequences", ("Reads number", "Reads per thousand"), "Reads per thousand", "Cutoff Score")


def read_libraries(path: Path) -> dict[str, tuple[list[tuple[str, int]], int]]:
    """{library name: ([(sequence, reads), ...], T)} in the order of the sheet, every row checked.

    T is the total the per-thousand values are relative to (see the module docstring).
    """
    rows = sheet_rows(path, {SHEET})[SHEET]
    if not rows or rows[0][0] != 1:
        raise ValueError(f"{SHEET}: row 1 (the library names) is missing")
    header = rows[0][1]
    out = {}
    for name in LIBRARIES:
        found = [col for col, text in header.items() if text.strip() == name]
        if len(found) != 1:
            raise ValueError(f"{SHEET}: row 1 names library {name!r} {len(found)} times, expected once")
        first = column_index(found[0])
        for i, expected in enumerate(HEADERS, start=1):
            text = header.get(column_letters(first + i), "").strip()
            if text not in ((expected,) if isinstance(expected, str) else expected):
                raise ValueError(f"{SHEET}: column {column_letters(first + i)}1 of {name} reads {text!r}, expected {expected!r}")
        seq_col, reads_col, thousand_col = (column_letters(first + i) for i in (1, 2, 3))
        records, totals, ended = [], [], False
        for number, row in rows[1:]:
            cells = [row.get(c) for c in (seq_col, reads_col, thousand_col)]
            if not any(cells):
                ended = True
                continue
            seq, reads, thousand = cells
            where = f"{SHEET}: {name}, row {number}"
            if ended or number != len(records) + 2:
                raise ValueError(f"{where}: the list of {name} has a gap")
            if not (seq and re.fullmatch(f"[ACGT]{{{LENGTH}}}", seq) and reads and re.fullmatch(r"[1-9][0-9]*", reads)):
                raise ValueError(f"{where}: expected a {LENGTH}-letter sequence of A, C, G, T and a positive whole read number")
            try:
                per_thousand = float(thousand)
            except (TypeError, ValueError):
                per_thousand = math.nan
            if not (math.isfinite(per_thousand) and per_thousand > 0):
                raise ValueError(f"{where}: the reads per thousand {thousand!r} is not a positive number")
            records.append((seq, int(reads)))
            totals.append(1000 * int(reads) / per_thousand)
        if not records:
            raise ValueError(f"{SHEET}: library {name} lists no sequences")
        if len({s for s, _ in records}) != len(records):
            raise ValueError(f"{SHEET}: library {name} lists a sequence more than once")
        total = round(totals[0])
        for number, implied in enumerate(totals, start=2):
            if not math.isclose(implied, total, rel_tol=1e-12):
                raise ValueError(
                    f"{SHEET}: {name}, row {number}: the reads per thousand are relative to {implied:.6f} reads, "
                    f"row 2's to {total}"
                )
        out[name] = (records, total)
    return out


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__.strip().splitlines()[-1], file=sys.stderr)
        return 2
    libraries = read_libraries(Path(sys.argv[1]))
    outdir = Path(sys.argv[2])
    for name, stem in LIBRARIES.items():
        records, total = libraries[name]
        file_name = f"pstvd_loop27_{stem}.fasta"
        with open(outdir / file_name, "w", encoding="ascii", newline="\n") as f:
            for n, (seq, reads) in enumerate(records, start=1):
                f.write(f">Seq{n}\n{seq}\n" * reads)
        print(f"{file_name}: {len(records)} sequences, {sum(r for _, r in records)} reads "
              f"(reads per thousand relative to {total})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
