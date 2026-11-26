"""Read the cells of sheets of an Excel workbook (.xlsx), standard library only.

Used by the converters that write test data from published supplementary spreadsheets
(tobrfv_from_supplement.py, pstvd_from_supplement.py). An .xlsx file is a zip archive of XML
parts (ECMA-376 Part 1, SpreadsheetML): the workbook lists the sheets, its relationships give each
sheet's part, and text cells refer to the shared-string table.
"""
import re
import xml.etree.ElementTree as ET
import zipfile
from pathlib import Path

NS = {"m": "http://schemas.openxmlformats.org/spreadsheetml/2006/main"}
REL_ID = "{http://schemas.openxmlformats.org/officeDocument/2006/relationships}id"


def sheet_rows(path: Path, names: set[str]) -> dict[str, list[tuple[int, dict[str, str]]]]:
    """{sheet name: [(row number, {column letters: text}), ...]} for the named sheets.

    Rows are in the order of the sheet; a cell without a value is left out, so a row of empty
    cells is an empty dict. Fails if a named sheet is missing.
    """
    with zipfile.ZipFile(path) as book:
        try:
            strings = ET.fromstring(book.read("xl/sharedStrings.xml")).findall("m:si", NS)
        except KeyError:  # a workbook without text cells has no shared-string table
            strings = []
        shared = ["".join(t.text or "" for t in si.iter(f"{{{NS['m']}}}t")) for si in strings]
        workbook = ET.fromstring(book.read("xl/workbook.xml"))
        target = {r.get("Id"): r.get("Target") for r in ET.fromstring(book.read("xl/_rels/workbook.xml.rels"))}
        out = {}
        for sheet in workbook.find("m:sheets", NS):
            name = sheet.get("name")
            if name in names:
                part = "xl/" + target[sheet.get(REL_ID)].lstrip("/").removeprefix("xl/")
                out[name] = _rows(ET.fromstring(book.read(part)), shared)
    missing = names - out.keys()
    if missing:
        raise ValueError(f"{path}: no sheet named {', '.join(sorted(missing))}")
    return out


def column_index(letters: str) -> int:
    """The 0-based index of a column: A is 0, Z is 25, AA is 26."""
    n = 0
    for ch in letters:
        n = n * 26 + ord(ch) - ord("A") + 1
    return n - 1


def column_letters(index: int) -> str:
    """The letters of the column with this 0-based index (the inverse of column_index)."""
    letters = ""
    index += 1
    while index:
        index, r = divmod(index - 1, 26)
        letters = chr(ord("A") + r) + letters
    return letters


def _rows(sheet: ET.Element, shared: list[str]) -> list[tuple[int, dict[str, str]]]:
    rows = []
    for row in sheet.iter(f"{{{NS['m']}}}row"):
        if row.get("r") is None:
            raise ValueError("a row without its number (attribute r), which this reader needs")
        cells = {}
        for c in row.findall("m:c", NS):
            v = c.find("m:v", NS)
            if v is None or not v.text:
                continue
            column = re.match(r"[A-Z]+", c.get("r") or "")
            if column is None:
                raise ValueError(f"row {row.get('r')}: a cell without its reference (attribute r), which this reader needs")
            cells[column.group()] = shared[int(v.text)] if c.get("t") == "s" else v.text
        rows.append((int(row.get("r")), cells))
    return rows
