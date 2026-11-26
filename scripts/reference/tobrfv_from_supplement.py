#!/usr/bin/env python3
"""Write the ToBRFV test alignments from Wu 2025's Supplementary File 2 (stdlib only).

Wu J. "Similarity-weighted entropy for quantifying genetic diversity in viral quasispecies".
Virus Evol 2025;11(1):veaf029, doi:10.1093/ve/veaf029, CC BY 4.0. Supplementary File 2 lists
the unique 120-nt sequences of the 5'-120nts region of tomato brown rugose fruit virus in tm-2
and Tm-2^2 tomato plants, each with its read count. (Wu reports excluding sequences below 0.1% of
the reads; the tm-2 sheet nevertheless lists eight with 10 reads, 0.092%. All are kept: with all
48 the tm-2 values of Wu's Fig. 6d are reproduced.) Europe PMC serves the supplement as a zip archive
(PMC12079039, ``/rest/PMC12079039/supplementaryFiles``) holding ``veaf029_supp.zip``, which
holds ``suppl_data/Supplementary File 2.xlsx``.

Each unique sequence is written once per read, so that the k-mer counts of a run at k = 120
are the read counts: ``tobrfv_tm2.fasta`` (sheet "tm-2") and ``tobrfv_tm22.fasta`` (sheet
"Tm-22"). Every record of a sequence carries the sheet's ID (``SeqN``), so a file compresses to
the size of the list. The output is deterministic.

Usage: python3 tobrfv_from_supplement.py "Supplementary File 2.xlsx" OUTPUT_DIR
"""
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from xlsx_rows import sheet_rows  # noqa: E402  (this directory, also under python3 -I)

SHEETS = {"tm-2": "tobrfv_tm2.fasta", "Tm-22": "tobrfv_tm22.fasta"}


def read_sheets(path: Path) -> dict[str, list[tuple[str, str, int]]]:
    """{sheet name: [(id, sequence, reads), ...]} in the order of the sheet.

    Each row needs an ID, a sequence of A, C, G, T and a positive whole read count, and its
    "Percentage" (column D, seven decimals) must be 100 x reads / (the listed reads' total),
    which confirms that the list is complete.
    """
    out = {}
    for name, rows in sheet_rows(path, set(SHEETS)).items():
        records, percentages = [], []
        for _, row in rows:
            ident, seq, reads = row.get("A", "").strip(), row.get("B", ""), row.get("C", "")
            if not row or ident.startswith("Supplementary File") or ident == "IDs":
                continue  # an empty row, the sheet's title or its header
            if not (re.fullmatch(r"Seq[0-9]+", ident) and re.fullmatch(r"[ACGT]+", seq)
                    and re.fullmatch(r"[1-9][0-9]*", reads)):
                raise ValueError(f"{name}: row {ident!r} is not a sequence of A, C, G, T with a positive whole read count")
            records.append((ident, seq, int(reads)))
            percentages.append(row.get("D", ""))
        total = sum(r for _, _, r in records)
        for (ident, _, reads), shown in zip(records, percentages, strict=True):
            try:
                ok = abs(float(shown) - 100 * reads / total) <= 6e-8
            except ValueError:
                ok = False
            if not ok:
                raise ValueError(f"{name}: row {ident!r}: percentage {shown!r} is not 100 x {reads} / {total}")
        out[name] = records
    return out


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__.strip().splitlines()[-1], file=sys.stderr)
        return 2
    sheets = read_sheets(Path(sys.argv[1]))
    outdir = Path(sys.argv[2])
    for name, file_name in SHEETS.items():
        records = sheets[name]
        lengths = {len(s) for _, s, _ in records}
        if lengths != {120} or len({s for _, s, _ in records}) != len(records):
            print(f"{name}: expected distinct 120-nt sequences", file=sys.stderr)
            return 1
        with open(outdir / file_name, "w", encoding="ascii", newline="\n") as f:
            for ident, seq, reads in records:
                f.write(f">{ident}\n{seq}\n" * reads)
        print(f"{file_name}: {len(records)} sequences, {sum(r for _, _, r in records)} reads")
    return 0


if __name__ == "__main__":
    sys.exit(main())
