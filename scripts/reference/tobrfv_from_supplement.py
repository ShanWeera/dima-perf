#!/usr/bin/env python3
"""Write the ToBRFV test alignments from Wu 2025's Supplementary File 2 (stdlib only).

Wu J. "Similarity-weighted entropy for quantifying genetic diversity in viral quasispecies".
Virus Evol 2025;11(1):veaf029, doi:10.1093/ve/veaf029, CC BY 4.0. Supplementary File 2 lists
the unique 120-nt sequences of the 5'-120nts region of tomato brown rugose fruit virus in tm-2
and Tm-2^2 tomato plants, each with its read count (sequences below 0.1% of the reads were
excluded by the author). Europe PMC serves the supplement as a zip archive
(PMC12079039, ``/rest/PMC12079039/supplementaryFiles``) holding ``veaf029_supp.zip``, which
holds ``suppl_data/Supplementary File 2.xlsx``.

Each unique sequence is written once per read, so that the k-mer counts of a run at k = 120
are the read counts: ``tobrfv_tm2.fasta`` (sheet "tm-2") and ``tobrfv_tm22.fasta`` (sheet
"Tm-22"), records named ``SeqN_r`` (r = 1 .. reads). The output is deterministic.

Usage: python3 tobrfv_from_supplement.py "Supplementary File 2.xlsx" OUTPUT_DIR
"""
import re
import sys
import xml.etree.ElementTree as ET
import zipfile
from pathlib import Path

NS = {"m": "http://schemas.openxmlformats.org/spreadsheetml/2006/main"}
SHEETS = {"tm-2": "tobrfv_tm2.fasta", "Tm-22": "tobrfv_tm22.fasta"}


def cells(book: zipfile.ZipFile, sheet_path: str, shared: list[str]) -> list[dict[str, str]]:
    """The rows of one sheet as {column letter: text}."""
    rows = []
    for row in ET.fromstring(book.read(sheet_path)).iter(f"{{{NS['m']}}}row"):
        out = {}
        for c in row.findall("m:c", NS):
            v = c.find("m:v", NS)
            if v is None:
                continue
            text = shared[int(v.text)] if c.get("t") == "s" else v.text
            out[re.match(r"[A-Z]+", c.get("r")).group()] = text
        rows.append(out)
    return rows


def read_sheets(path: Path) -> dict[str, list[tuple[str, str, int]]]:
    """{sheet name: [(id, sequence, reads), ...]} in the order of the sheet."""
    book = zipfile.ZipFile(path)
    shared = [
        "".join(t.text or "" for t in si.iter(f"{{{NS['m']}}}t"))
        for si in ET.fromstring(book.read("xl/sharedStrings.xml")).findall("m:si", NS)
    ]
    workbook = ET.fromstring(book.read("xl/workbook.xml"))
    rels = ET.fromstring(book.read("xl/_rels/workbook.xml.rels"))
    target = {r.get("Id"): r.get("Target") for r in rels}
    rel_ns = "{http://schemas.openxmlformats.org/officeDocument/2006/relationships}id"
    out = {}
    for sheet in workbook.find("m:sheets", NS):
        name = sheet.get("name")
        if name not in SHEETS:
            continue
        records = []
        for row in cells(book, "xl/" + target[sheet.get(rel_ns)].lstrip("/").removeprefix("xl/"), shared):
            seq, reads = row.get("B", ""), row.get("C", "")
            if re.fullmatch(r"[ACGT]+", seq) and re.fullmatch(r"[0-9]+", reads):
                records.append((row["A"].strip(), seq, int(reads)))
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
                for r in range(1, reads + 1):
                    f.write(f">{ident}_{r}\n{seq}\n")
        print(f"{file_name}: {len(records)} sequences, {sum(r for _, _, r in records)} reads")
    return 0


if __name__ == "__main__":
    sys.exit(main())
