#!/usr/bin/env python3
"""Independent reference implementation of DiMA (a test oracle).

DiMA is described in: Tharanga S, Unlu ES, Hu Y, Sjaugi MF, Celik MA, Hekimoglu H,
Miotto O, Oncel MM, Khan AM. "DiMA: sequence diversity dynamics analyser for viruses".
Brief Bioinform 2025;26(1):bbae607. doi:10.1093/bib/bbae607.

The Rust crate (``crates/dima-core`` and the ``dima`` CLI) is the product. This file is
its *oracle*: the Rust differential tests run it on the same input and compare every
number at a relative tolerance of 1e-13 (zero exactly). It therefore follows three principles:

1. **Simple and obviously correct.** Naive counting (slice every k-mer of every record
   and count it in a dict), one function per rule, the rule stated in the docstring.
   The only optimisations are the ones needed to finish a 3,000 x 3,400 alignment in
   about a minute, or about two minutes with --similarity-weighted-entropy (caches keyed
   by exact integers, a mode-centred binomial walk that stops only once the remaining
   terms are below 2**-997 of the largest one, and each k-mer converted once to an
   integer, so a pair's identical residues are the zero bytes of their XOR).
2. **More accurate than the tolerance it is used with.** Every floating-point result is
   either correctly rounded from exact or 60-80 digit decimal arithmetic, or carries a
   short error analysis next to the code showing it is accurate to a few units in the
   last place (ulp). ``math.fsum`` is used for every floating-point sum.
3. **Deterministic.** No randomness, no timestamps, no hash-order dependence: every
   list that reaches the output is in an explicit order. ``json.dumps`` prints floats
   with ``repr`` (shortest round-trip). NaN and infinities are never written. Only the
   values that go through the platform's ``log``/``log1p``/``pow`` (the dima entropy and
   the Clopper-Pearson bound) may differ in the last bit between operating systems.

Only the Python standard library is used (Python >= 3.10).

Usage::

    python3 dima_reference.py analyze -i FILE [-k K] [-s T] [-a auto|protein|dna|rna]
        [--validation strict|permissive|report] [--header-format A|B|C]
        [--metadata-fields A|B] [--header-fillna Unknown] [--hcs-min-incidence 100]
        [--hcs-eligible-only] [--similarity-weighted-entropy] [-o out.json]
    python3 dima_reference.py --self-test

Exit codes: 0 success, 1 internal error or self-test failure, 2 usage error, 3 I/O error
(unreadable file, corrupt compressed stream), 4 invalid input data.

The specification each function implements is in its docstring; ``README.md`` next to
this file summarises it.
"""

from __future__ import annotations

import argparse
import bz2
import decimal
import gzip
import io
import itertools
import json
import math
import operator
import os
import re
import sys
import tempfile
import time
import unicodedata
import zlib
from collections import Counter
from dataclasses import dataclass
from fractions import Fraction
from functools import lru_cache

REFERENCE_VERSION = 2

EXIT_OK = 0
EXIT_INTERNAL = 1
EXIT_USAGE = 2
EXIT_IO = 3
EXIT_DATA = 4

PROG = "dima_reference"


# ---------------------------------------------------------------------------------------
# Errors
# ---------------------------------------------------------------------------------------


class DimaError(Exception):
    """An error that ends the run with a documented exit code and a message on stderr."""

    exit_code = EXIT_INTERNAL


class UsageError(DimaError):
    """Invalid command-line options (exit code 2)."""

    exit_code = EXIT_USAGE


class InputIOError(DimaError):
    """The input could not be read or decompressed (exit code 3)."""

    exit_code = EXIT_IO


class DataError(DimaError):
    """The input was read but is not a valid alignment for the options (exit code 4)."""

    exit_code = EXIT_DATA


# ---------------------------------------------------------------------------------------
# High-precision arithmetic helpers
# ---------------------------------------------------------------------------------------

#: 60 significant digits: used for the positive Chao-Wang-Jost tail series. Every
#: operation is correctly rounded by libmpdec; ``float(Decimal)`` is correctly rounded
#: (it parses the exact decimal string), so a Decimal result accurate to far more than
#: 17 digits becomes the correctly rounded binary64 value (barring exact ties).
_HP = decimal.Context(
    prec=60,
    rounding=decimal.ROUND_HALF_EVEN,
    Emax=999_999_999,
    Emin=-999_999_999,
    traps=[decimal.InvalidOperation, decimal.DivisionByZero, decimal.Overflow],
)

#: 80 digits: plug-in entropy, Miller-Madow, Chao-Wang-Jost part 1 and the direct
#: Chao-Wang-Jost tail formula (which may cancel up to ~25 digits).
_HP80 = decimal.Context(
    prec=80,
    rounding=decimal.ROUND_HALF_EVEN,
    Emax=999_999_999,
    Emin=-999_999_999,
    traps=[decimal.InvalidOperation, decimal.DivisionByZero, decimal.Overflow],
)

_LN2_DEC = decimal.Decimal(2).ln(_HP80)
_LN2_F = math.log(2.0)


@lru_cache(maxsize=None)
def _ln_int(m: int) -> decimal.Decimal:
    """Natural logarithm of the positive integer ``m`` to 80 significant digits."""
    return decimal.Decimal(m).ln(_HP80)


_HARMONIC: list[decimal.Decimal] = [decimal.Decimal(0)]


def _harmonic(m: int) -> decimal.Decimal:
    """H_m = sum_{j=1..m} 1/j as an 80-digit Decimal (H_0 = 0).

    Built once by prefix accumulation; each addition rounds at 80 digits, so after m
    additions the absolute error is below m * 1e-79, irrelevant for m up to 1e9.
    """
    if m >= len(_HARMONIC):
        with decimal.localcontext(_HP80):
            h = _HARMONIC[-1]
            for j in range(len(_HARMONIC), m + 1):
                h = h + decimal.Decimal(1) / decimal.Decimal(j)
                _HARMONIC.append(h)
    return _HARMONIC[m]


def _clean_float(value: float) -> float:
    """Return ``value`` as a finite float, mapping -0.0 to 0.0 (raises on a non-finite value).

    JSON has no NaN or infinity and DiMA output must never contain them; a negative zero
    would print as ``-0.0`` and make byte comparisons platform dependent. The check raises
    instead of asserting, so that ``python -O`` keeps it.
    """
    value = float(value)
    if not math.isfinite(value):
        raise RuntimeError(f"internal error: non-finite value {value!r} would reach the output")
    if value == 0.0:
        return 0.0
    return value


# ---------------------------------------------------------------------------------------
# Input: decompression and FASTA parsing
# ---------------------------------------------------------------------------------------

_MAGIC_GZIP = b"\x1f\x8b"
_MAGIC_BZIP2 = b"BZh"
_MAGIC_XZ = b"\xfd7zXZ\x00"
_BOM = b"\xef\xbb\xbf"

#: Whitespace bytes removed from sequence lines: space, TAB, CR, VT, FF. (LF is the line
#: separator and never part of a line.)
_SEQ_WHITESPACE = b" \t\r\x0b\x0c"


def _decode_streams(raw: bytes, magic: bytes, new_decoder) -> bytes:
    """Decode a concatenation of complete compressed streams, one decoder per stream.

    After each stream the remaining bytes must be empty, start a new stream (their first
    bytes match the magic), or be zero bytes to the end (padding, ignored as GNU gzip does).
    A truncated stream or any other trailing data is an error -- never silently ignored (the
    stdlib one-shot ``bz2.decompress`` would drop such data).
    """
    out = []
    data = raw
    while True:
        decoder = new_decoder()
        out.append(decoder.decompress(data))
        if not decoder.eof:
            raise EOFError("compressed stream is truncated (no end-of-stream marker)")
        data = decoder.unused_data
        if not data:
            return b"".join(out)
        shared = min(len(data), len(magic))
        if data[:shared] == magic[:shared]:
            continue
        if data.count(0) == len(data):
            return b"".join(out)
        raise ValueError(f"{len(data)} bytes that are not compressed data follow the end of it")


def decompress_bytes(raw: bytes) -> bytes:
    """Return ``raw`` decompressed according to its magic bytes, or unchanged.

    Rule: gzip (``1F 8B``) and bzip2 (``BZh``) are detected by their magic bytes, never by
    the file name. Multi-member gzip and multi-stream bzip2 are fully decoded and their
    outputs concatenated; zero bytes after the last one are padding. Truncated or damaged
    streams and other trailing data are invalid input (exit 4, as in DiMA's exit codes). xz
    (``FD 37 7A 58 5A 00``) is refused, as by DiMA (D116). Anything else is returned as is
    (plain text). (DiMA also reads Zstandard, which the standard library cannot.)
    """
    if raw.startswith(_MAGIC_XZ):
        raise DataError("the input is xz (.xz), which DiMA cannot read")
    try:
        if raw.startswith(_MAGIC_GZIP):
            # wbits 31: one gzip member, header and CRC/size trailer checked.
            return _decode_streams(raw, _MAGIC_GZIP, lambda: zlib.decompressobj(wbits=31))
        if raw.startswith(_MAGIC_BZIP2):
            return _decode_streams(raw, _MAGIC_BZIP2, bz2.BZ2Decompressor)
    except (OSError, EOFError, ValueError, zlib.error) as exc:
        raise DataError(f"cannot decompress input: {exc}") from exc
    return raw


def read_input(path: str) -> bytes:
    """Read ``path`` (``-`` means standard input) and decompress it if needed."""
    try:
        if path == "-":
            raw = sys.stdin.buffer.read()
        else:
            with open(path, "rb") as fh:
                raw = fh.read()
    except OSError as exc:
        raise InputIOError(f"cannot read {path!r}: {exc.strerror or exc}") from exc
    return decompress_bytes(raw)


@dataclass
class Record:
    """One FASTA record.

    ``number`` is 1-based in file order; ``header`` is the raw header bytes (the line
    after '>' without its trailing CR); ``seq`` is the sequence with whitespace removed
    and ASCII letters upper-cased; ``line`` is the 1-based line number of the header;
    ``size`` is the number of bytes of the record as stored, from its '>' to the line feed
    that ends its last line (one is counted for a last line without it).
    """

    number: int
    header: bytes
    seq: bytes
    line: int
    size: int = 0


def parse_fasta(data: bytes) -> list[Record]:
    """Parse FASTA bytes into records.

    Rules:

    * A UTF-8 byte order mark (``EF BB BF``) at the very start of the input is skipped.
    * Lines are separated by LF. A line starting with ``>`` starts a record; its header
      is the rest of the line with one trailing CR removed (CRLF files). Any other CR, or a
      NUL byte, in a header line is an error. Every other line
      is a sequence line of the current record.
    * Sequence lines: a NUL byte anywhere is an error; the whitespace bytes space, TAB,
      CR, VT and FF are removed wherever they occur; ASCII lowercase letters are folded
      to uppercase (bytes >= 0x80 are untouched).
    * A line before the first ``>`` is an error if it contains any byte other than the
      whitespace bytes space, TAB, CR, VT and FF (so an indented ``>`` is an error too);
      blank or whitespace-only lines there carry no data and are ignored, including a final
      one without a line feed.
    * Empty input, or input without any record, is an error.
    """
    if not data:
        raise DataError("input is empty")
    if data.startswith(_BOM):
        data = data[len(_BOM):]
    records: list[Record] = []
    header: bytes | None = None
    header_line = 0
    size = 0
    parts: list[bytes] = []
    lines = data.split(b"\n")
    if data.endswith(b"\n"):
        lines.pop()  # the empty piece after the final line feed is not a line
    for lineno, line in enumerate(lines, start=1):
        if line.startswith(b">"):
            if header is not None:
                records.append(Record(len(records) + 1, header, b"".join(parts), header_line, size))
            size = 0
            header = line[1:]
            if header.endswith(b"\r"):
                header = header[:-1]
            for byte, what in ((b"\r", "carriage return"), (b"\x00", "NUL byte")):
                if byte in header:
                    raise DataError(
                        f"record {len(records) + 1}, line "
                        f"{lineno}: {what} inside a header line"
                    )
            header_line = lineno
            size = len(line) + 1
            parts = []
            continue
        nul = line.find(b"\x00")
        if nul >= 0:
            where = f"record {len(records) + 1}, " if header is not None else ""
            raise DataError(
                f"{where}line {lineno}, byte {nul + 1}: NUL byte in sequence data "
                "(hint: the file may be binary or UTF-16 encoded)"
            )
        stripped = line.translate(None, _SEQ_WHITESPACE)
        if header is None:
            if stripped:
                raise DataError(
                    f"line {lineno}: sequence data before the first '>' header "
                    "(hint: FASTA records must start with a '>' line)"
                )
            continue
        size += len(line) + 1
        parts.append(stripped.upper())
    if header is not None:
        records.append(Record(len(records) + 1, header, b"".join(parts), header_line, size))
    if not records:
        raise DataError("input contains no FASTA records (no line starts with '>')")
    return records


def check_equal_lengths(records: list[Record]) -> int:
    """Return the common column count L; every record must have exactly L columns."""
    length = len(records[0].seq)
    bad = [r for r in records if len(r.seq) != length]
    if bad:
        shown = ", ".join(f"record {r.number} (line {r.line}) has {len(r.seq)}" for r in bad[:20])
        more = f" and {len(bad) - 20} more" if len(bad) > 20 else ""
        raise DataError(
            f"sequences are not aligned: record 1 has {length} columns but {shown}{more} "
            "(hint: DiMA needs a multiple sequence alignment)"
        )
    return length


# ---------------------------------------------------------------------------------------
# Alphabet: Easel's esl_abc_GuessAlphabet and the symbol classes
# ---------------------------------------------------------------------------------------

PROTEIN = "protein"
DNA = "dna"
RNA = "rna"

#: The first batch, whose letters feed the alphabet guess: records are added until it holds
#: 10,000 records, or 64 MiB once it holds at least 1,000 (DiMA's batch limits, D40).
PROBE_MAX_RECORDS = 10_000
PROBE_MAX_BYTES = 64 << 20
PROBE_MIN_RECORDS = 1_000


def probe_count(records: list[Record]) -> int:
    """Number of records in DiMA's first batch (see ``PROBE_MAX_RECORDS``)."""
    total = 0
    for n, record in enumerate(records, start=1):
        total += record.size
        if n >= PROBE_MAX_RECORDS or (total >= PROBE_MAX_BYTES and n >= PROBE_MIN_RECORDS):
            return n
    return len(records)


def easel_guess_alphabet(ct: list[int]) -> tuple[str | None, str]:
    """Port of Easel's ``esl_abc_GuessAlphabet()`` (easel/esl_alphabet.c, master).

    ``ct[0..25]`` are the observed counts of the letters A..Z (case-insensitive). Returns
    ``(alphabet, rule)`` where ``alphabet`` is ``"protein"``, ``"dna"``, ``"rna"`` or
    ``None`` (Easel's eslUNKNOWN) and ``rule`` names the branch that decided. The C code,
    transcribed branch by branch (same order, same comparisons, same constants)::

        aaonly = "EFIJLOPQZ"; allcanon = "ACG"; aacanon = "DHKMRSVWY"
        n  = total letters;  n1/x1, n2/x2, n3/x3 = count/number-present of each set
        nt, nu, nx, nn = counts of T, U, X, N;  xt, xu, xn = 1 if present
        if      n <= 10                                        -> unknown (small sample)
        else if n > 2000 && nn == n                            -> DNA   (all N)
        else if n1 > 0                                         -> AMINO (aa-only letter)
        else if n-(n2+nt+nn) <= 0.02*n && x2+xt == 4           -> DNA
        else if n-(n2+nu+nn) <= 0.02*n && x2+xu == 4           -> RNA
        else if n-(n1+n2+n3+nn+nt+nx) <= 0.02*n && n3 > n2
                && x1+x2+x3+xn+xt >= 15                        -> AMINO
        else                                                   -> unknown

    ``0.02*n`` is a binary64 product in C and in Python; the int-vs-float comparison is
    exact in both (C converts the int64 to double exactly for n < 2**53). If both T and U
    are present above 2% of the letters, neither nucleotide branch can fire and the U
    letters also break the amino branch, so the result is unknown, as required.

    Note: the last (amino composition) branch can never fire as written in Easel: it is
    reached only when n1 == 0, hence x1 == 0 and x1+x2+x3+xn+xt <= 0+3+9+1+1 = 14 < 15.
    It is ported anyway, for fidelity. Protein alignments are therefore recognised only
    through at least one of the letters E F I J L O P Q Z.
    """
    assert len(ct) == 26

    def count(letter: str) -> int:
        return ct[ord(letter) - ord("A")]

    def tally(letters: str) -> tuple[int, int]:
        total = present = 0
        for letter in letters:
            value = count(letter)
            if value > 0:
                total += value
                present += 1
        return total, present

    n = sum(ct)
    n1, x1 = tally("EFIJLOPQZ")
    n2, x2 = tally("ACG")
    n3, x3 = tally("DHKMRSVWY")
    nt = count("T")
    xt = 1 if nt else 0
    nu = count("U")
    xu = 1 if nu else 0
    nx = count("X")
    nn = count("N")
    xn = 1 if nn else 0

    if n <= 10:
        return None, "too_few_residues"
    if n > 2000 and nn == n:
        return DNA, "all_n"
    if n1 > 0:
        return PROTEIN, "amino_only_letter"
    if n - (n2 + nt + nn) <= 0.02 * n and x2 + xt == 4:
        return DNA, "dna_composition"
    if n - (n2 + nu + nn) <= 0.02 * n and x2 + xu == 4:
        return RNA, "rna_composition"
    if n - (n1 + n2 + n3 + nn + nt + nx) <= 0.02 * n and n3 > n2 and x1 + x2 + x3 + xn + xt >= 15:
        return PROTEIN, "amino_composition"  # unreachable as written in Easel (see docstring)
    return None, "undetermined"


def letter_counts(records: list[Record]) -> list[int]:
    """Counts of the letters A..Z in the records of the first batch (already upper-cased)."""
    ct = [0] * 26
    for record in records[:probe_count(records)]:
        for i in range(26):
            ct[i] += record.seq.count(bytes([ord("A") + i]))
    return ct


RESIDUE = 0
EXCLUDED = 1
UNKNOWN = 2

#: Excluded in every alphabet: gap '-', '.', '_'; missing '~', '?'; stop '*', '$', '^';
#: frameshift '#'.
_COMMON_EXCLUDED = b"-._~?*$^#"
_PROTEIN_RESIDUES = b"ACDEFGHIKLMNPQRSTVWYUO"
_PROTEIN_AMBIGUITY = b"XBJZ"
_NUCLEOTIDE_AMBIGUITY = b"RYKMSWBDHVNXI"


@dataclass(frozen=True)
class SymbolTable:
    """Byte classes for one alphabet (after case folding).

    ``classes[b]`` is RESIDUE, EXCLUDED or UNKNOWN; ``fold`` is a ``bytes.translate``
    table that maps residue bytes to their canonical letter (DNA: U -> T; RNA: T -> U)
    and leaves every other byte unchanged.
    """

    alphabet: str
    classes: tuple[int, ...]
    fold: bytes
    residues: bytes
    known: bytes  # residue + excluded bytes (everything else is UNKNOWN)


def symbol_table(alphabet: str) -> SymbolTable:
    """Build the symbol classes for ``alphabet``.

    * protein residues: ``ACDEFGHIKLMNPQRSTVWY`` plus ``U`` (selenocysteine) and ``O``
      (pyrrolysine); excluded ambiguity codes ``X B J Z``.
    * dna residues: ``A C G T`` and ``U`` (folded to ``T``, counted);
      rna residues: ``A C G U`` and ``T`` (folded to ``U``, counted);
      excluded nucleotide ambiguity codes ``R Y K M S W B D H V N X I``.
    * excluded in every alphabet: ``- . _ ~ ? * $ ^ #``.
    * every other byte (other letters, digits, punctuation, controls, bytes >= 0x80) is
      UNKNOWN.
    """
    classes = [UNKNOWN] * 256
    fold = bytearray(range(256))
    if alphabet == PROTEIN:
        residues = _PROTEIN_RESIDUES
        excluded = _COMMON_EXCLUDED + _PROTEIN_AMBIGUITY
    elif alphabet == DNA:
        residues = b"ACGTU"
        excluded = _COMMON_EXCLUDED + _NUCLEOTIDE_AMBIGUITY
        fold[ord("U")] = ord("T")
    elif alphabet == RNA:
        residues = b"ACGUT"
        excluded = _COMMON_EXCLUDED + _NUCLEOTIDE_AMBIGUITY
        fold[ord("T")] = ord("U")
    else:  # pragma: no cover - guarded by the CLI
        raise UsageError(f"unknown alphabet {alphabet!r}")
    for b in residues:
        classes[b] = RESIDUE
    for b in excluded:
        assert classes[b] == UNKNOWN, f"byte {chr(b)!r} is both residue and excluded"
        classes[b] = EXCLUDED
    return SymbolTable(alphabet, tuple(classes), bytes(fold), residues, residues + excluded)


def _describe_byte(b: int) -> str:
    if 0x21 <= b <= 0x7E:
        return f"'{chr(b)}' (0x{b:02X})"
    return f"byte 0x{b:02X}"


def validate_symbols(
    records: list[Record], table: SymbolTable, mode: str
) -> tuple[list[bytes], list[bytes], Counter, dict[int, tuple[int, int]]]:
    """Classify every byte.

    Returns (folded sequences, residue masks, unknown-byte counts, first (record, column)
    of each unknown byte).

    * strict: the first UNKNOWN byte (in record order, then column order) is an error
      reporting the 1-based record number and 1-based alignment column.
    * permissive / report: UNKNOWN bytes are treated as EXCLUDED and counted.

    The residue mask of a record is a bytes object of the same length with ``1`` where
    the column holds a residue and ``0`` elsewhere (excluded or unknown).
    """
    mask_table = bytearray(b"0" * 256)
    for b in table.residues:
        mask_table[b] = ord("1")
    mask_table = bytes(mask_table)
    unknown: Counter = Counter()
    first_seen: dict[int, tuple[int, int]] = {}
    folded: list[bytes] = []
    masks: list[bytes] = []
    for record in records:
        leftover = record.seq.translate(None, table.known)
        if leftover:
            for column, b in enumerate(record.seq, start=1):
                if table.classes[b] == UNKNOWN:
                    if mode == "strict":
                        raise DataError(
                            f"record {record.number} (line {record.line}), column {column}: "
                            f"{_describe_byte(b)} is not a valid {table.alphabet} symbol "
                            "(hint: use --validation permissive to treat unknown symbols "
                            "as excluded, or check --alphabet)"
                        )
                    first_seen.setdefault(b, (record.number, column))
            unknown.update(leftover)
        folded.append(record.seq.translate(table.fold))
        masks.append(record.seq.translate(mask_table))
    return folded, masks, unknown, first_seen


# ---------------------------------------------------------------------------------------
# Header metadata
# ---------------------------------------------------------------------------------------

#: The Unicode White_Space property (PropList.txt; unchanged since Unicode 6.3).
WHITE_SPACE = frozenset(
    chr(c)
    for c in itertools.chain(
        range(0x0009, 0x000D + 1),
        (0x0020, 0x0085, 0x00A0, 0x1680),
        range(0x2000, 0x200A + 1),
        (0x2028, 0x2029, 0x202F, 0x205F, 0x3000),
    )
)


def _is_removed_char(ch: str) -> bool:
    """True for characters deleted from metadata values.

    Deleted: every character of general category Cf (format), and every C0 (U+0000 to
    U+001F) and C1 (U+0080 to U+009F) control, except TAB (U+0009). DEL (U+007F) is in
    neither range and is kept. Categories come from ``unicodedata`` (Unicode
    ``unicodedata.unidata_version`` of the running Python).
    """
    o = ord(ch)
    if o == 0x09:
        return False
    if o <= 0x1F or 0x80 <= o <= 0x9F:
        return True
    return unicodedata.category(ch) == "Cf"


def clean_metadata_value(raw: str, fill: str) -> str:
    """Clean one header field value.

    1. delete Cf characters and C0/C1 controls except TAB (see ``_is_removed_char``);
    2. strip leading and trailing Unicode White_Space characters (the explicit set
       ``WHITE_SPACE``; ``str.strip()`` differs, e.g. it also strips U+001C..U+001F);
    3. normalise to NFC;
    4. if the result is empty, return ``fill``.
    """
    kept = "".join(ch for ch in raw if not _is_removed_char(ch))
    start, end = 0, len(kept)
    while start < end and kept[start] in WHITE_SPACE:
        start += 1
    while end > start and kept[end - 1] in WHITE_SPACE:
        end -= 1
    value = unicodedata.normalize("NFC", kept[start:end])
    return value if value else fill


def parse_field_list(text: str, option: str) -> list[str]:
    """Split a ``A|B|C`` option value into names (non-empty, unique)."""
    names = text.split("|")
    if any(name == "" for name in names):
        raise UsageError(f"{option}: empty field name in {text!r}")
    duplicates = sorted({n for n in names if names.count(n) > 1})
    if duplicates:
        raise UsageError(f"{option}: duplicate field name(s) {', '.join(duplicates)}")
    return names


def parse_metadata(
    records: list[Record], format_names: list[str], selected: list[str], fill: str, mode: str
) -> tuple[list[tuple[str, ...]], list[tuple[int, int, int]]]:
    """Return, per record, the cleaned values of the ``selected`` fields, and the
    (record number, line, field count) of every record whose field count did not match.

    Rules: the header (bytes after '>' without trailing CR) is decoded as UTF-8 (strict:
    invalid UTF-8 is an error; permissive/report: invalid sequences become U+FFFD), then
    split on every ``|``. If the number of fields differs from ``len(format_names)``:
    strict -> error listing up to 20 offending records; permissive/report -> every field
    of that record is the fill value. Each value is cleaned by ``clean_metadata_value``.
    ``selected`` is a subset of ``format_names`` in header-format order.
    """
    indices = [format_names.index(name) for name in selected]
    out: list[tuple[str, ...]] = []
    mismatched: list[tuple[int, int, int]] = []
    for record in records:
        try:
            text = record.header.decode("utf-8", errors="strict" if mode == "strict" else "replace")
        except UnicodeDecodeError as exc:
            raise DataError(
                f"record {record.number}, line {record.line}, byte {exc.start + 2}: header is not "
                "valid UTF-8 (hint: use --validation permissive to replace invalid bytes)"
            ) from exc
        fields = text.split("|")
        if len(fields) != len(format_names):
            mismatched.append((record.number, record.line, len(fields)))
            out.append(tuple(fill for _ in indices))
            continue
        out.append(tuple(clean_metadata_value(fields[i], fill) for i in indices))
    if mismatched and mode == "strict":
        shown = "; ".join(f"record {num} (line {line}) has {got}" for num, line, got in mismatched[:20])
        more = f"; and {len(mismatched) - 20} more" if len(mismatched) > 20 else ""
        raise DataError(
            f"{len(mismatched)} header(s) do not have {len(format_names)} '|'-separated fields "
            f"({'|'.join(format_names)}): {shown}{more} (hint: use --validation permissive to "
            "fill such records with --header-fillna)"
        )
    return out, mismatched


# ---------------------------------------------------------------------------------------
# Counting
# ---------------------------------------------------------------------------------------

_RESIDUE_RUN = re.compile(rb"1+")


def valid_starts(mask: bytes, k: int, positions: int) -> bytes:
    """Return a 0/1 byte per start position p (0 <= p < positions = L - k + 1).

    Rule: the k-mer starting at p (columns p..p+k-1) is counted only if none of its
    bytes is excluded or unknown, i.e. all k columns are residues. Equivalently, p is
    valid iff [p, p+k) lies inside a maximal run of residue columns [s, e), i.e.
    s <= p <= e - k.
    """
    valid = bytearray(positions)
    for run in _RESIDUE_RUN.finditer(mask):
        s, e = run.span()
        if e - s >= k:
            valid[s:e - k + 1] = b"\x01" * (e - k + 1 - s)
    return bytes(valid)


# ---------------------------------------------------------------------------------------
# Labels and diversity motifs
# ---------------------------------------------------------------------------------------


def support_label(support: int, threshold: int) -> str | None:
    """NS: support 0; LS: 0 < support < T; ELS: support == T; eligible (None): support > T."""
    if support == 0:
        return "NS"
    if support < threshold:
        return "LS"
    if support == threshold:
        return "ELS"
    return None


MOTIF_LONG = {"I": "Index", "Ma": "Major", "Mi": "Minor", "U": "Unique"}


def classify_motifs(counts: list[int]) -> list[str]:
    """Motif code for each variant count (identical to DiMA v5.0.9).

    Let max = the largest count.

    * Index ``I``: count == max and count > 1 (ties give several Index variants);
    * Unique ``U``: count == 1;
    * of the remaining variants (neither Index nor Unique): Major ``Ma`` = count equal to
      the largest count among them (ties give several), Minor ``Mi`` = the rest.
    """
    if not counts:
        return []
    top = max(counts)
    index = [c == top and c > 1 for c in counts]
    rest = [c for c, is_index in zip(counts, index, strict=True) if not is_index and c != 1]
    major = max(rest) if rest else None
    codes = []
    for c, is_index in zip(counts, index, strict=True):
        if is_index:
            codes.append("I")
        elif c == 1:
            codes.append("U")
        elif c == major:
            codes.append("Ma")
        else:
            codes.append("Mi")
    return codes


def sort_variants(counter: Counter) -> list[tuple[bytes, int]]:
    """Variants sorted by count descending, then k-mer ascending (byte order)."""
    return sorted(counter.items(), key=lambda item: (-item[1], item[0]))


def multiplicities(counts) -> dict[int, int]:
    """f_c: number of distinct k-mers seen exactly c times, keyed by c ascending."""
    return dict(sorted(Counter(counts).items()))


# ---------------------------------------------------------------------------------------
# Entropy estimators
# ---------------------------------------------------------------------------------------


def plug_in_entropy_decimal(mult: dict[int, int], n_obs: int) -> decimal.Decimal:
    """Plug-in (maximum-likelihood) Shannon entropy in bits, as an 80-digit Decimal.

    H = -sum_i (c_i/N) log2(c_i/N) = (1 / (N ln 2)) * sum_c f_c * c * (ln N - ln c).
    Requires N >= 1. Variants with c == N contribute exactly 0.
    """
    assert n_obs >= 1
    with decimal.localcontext(_HP80):
        total = decimal.Decimal(0)
        ln_n = _ln_int(n_obs)
        for c, f in mult.items():
            if c != n_obs:
                total += decimal.Decimal(f * c) * (ln_n - _ln_int(c))
        return total / (decimal.Decimal(n_obs) * _LN2_DEC)


def plug_in_entropy(mult: dict[int, int], n_obs: int) -> float | None:
    """Plug-in entropy in bits, correctly rounded to binary64; None when N == 0."""
    if n_obs == 0:
        return None
    return _clean_float(plug_in_entropy_decimal(mult, n_obs))


def miller_madow(mult: dict[int, int], n_obs: int) -> float | None:
    """Miller-Madow entropy H + (D - 1) / (2 N ln 2) in bits.

    D = number of distinct k-mers. N == 0 -> None; N == 1 -> 0.0 (``single_observation``;
    the formula also gives 0 because D = 1 and H = 0).
    """
    if n_obs == 0:
        return None
    if n_obs == 1:
        return 0.0
    distinct = sum(mult.values())
    with decimal.localcontext(_HP80):
        value = plug_in_entropy_decimal(mult, n_obs) + decimal.Decimal(distinct - 1) / (
            decimal.Decimal(2 * n_obs) * _LN2_DEC
        )
    return _clean_float(value)


def matches(x: int, y: int, k: int) -> int:
    """Number of identical residues of two k-mers of length k, given as the big-endian
    integers of their bytes: the zero bytes of their XOR (the self-test checks it against
    counting equal positions directly)."""
    return (x ^ y).to_bytes(k, "big").count(0)


def similarity_numerators(variants: list[tuple[bytes, int]]) -> list[int]:
    """Wu 2025 (Virus Evol 11:veaf029), Section 2.1: for each variant i, the exact integer
    k*N - A_i with A_i = sum_{j != i} c_j * m_ij, where m_ij is the number of identical
    residues of k-mers i and j, so that the similarity is s_ij = m_ij / k and the weight is
    1 - S_i = (k*N - A_i) / (k*N). Every unordered pair of distinct k-mers is compared once,
    as Wu defines it (never the per-site counting of the Rust library)."""
    k = len(variants[0][0])
    n_obs = sum(c for _, c in variants)
    ints = [int.from_bytes(kmer, "big") for kmer, _ in variants]
    a = [0] * len(variants)
    for i in range(len(variants)):
        ci = variants[i][1]
        xi = ints[i]
        for j in range(i + 1, len(variants)):
            m = matches(xi, ints[j], k)
            a[i] += variants[j][1] * m
            a[j] += ci * m
    return [k * n_obs - ai for ai in a]


def similarity_weighted_entropy_decimal(variants: list[tuple[bytes, int]], n_obs: int) -> decimal.Decimal:
    """Similarity-weighted entropy H_sim = sum_i (-f_i log2 f_i)(1 - S_i) in bits (Wu 2025,
    Section 2.1 and Supplementary File 1) as an 80-digit Decimal; f_i = c_i / N and the
    weights are exact rationals (``similarity_numerators``). Requires N >= 1; one distinct
    k-mer gives exactly 0."""
    assert n_obs >= 1
    if len(variants) == 1:
        return decimal.Decimal(0)
    k = len(variants[0][0])
    numerators = similarity_numerators(variants)
    with decimal.localcontext(_HP80):
        ln_n = _ln_int(n_obs)
        total = decimal.Decimal(0)
        for (_, c), num in zip(variants, numerators, strict=True):
            weight = Fraction(num, k * n_obs)
            total += decimal.Decimal(c) * (ln_n - _ln_int(c)) * decimal.Decimal(weight.numerator) / decimal.Decimal(
                weight.denominator
            )
        return total / (decimal.Decimal(n_obs) * _LN2_DEC)


def normalised_entropy(value: decimal.Decimal, distinct: int) -> float | None:
    """value / log2(D), capped at 1 (Wu 2025, Section 2.1: H_n and H_nsim, D the number of
    distinct k-mers, Index included); None when D <= 1."""
    if distinct <= 1:
        return None
    with decimal.localcontext(_HP80):
        x = value * _LN2_DEC / _ln_int(distinct)
    return _clean_float(min(x, decimal.Decimal(1)))


def chao_coverage_a(n_obs: int, f1: int, f2: int) -> Fraction:
    """The A parameter of Chao, Wang & Jost (2013), as an exact rational.

    A = 2 f2 / ((N-1) f1 + 2 f2) if f2 > 0; else 2 / ((N-1)(f1-1) + 2) if f1 > 0; else 1.
    """
    if f2 > 0:
        return Fraction(2 * f2, (n_obs - 1) * f1 + 2 * f2)
    if f1 > 0:
        return Fraction(2, (n_obs - 1) * (f1 - 1) + 2)
    return Fraction(1)


def _cwj_tail_series(n_obs: int, z: decimal.Decimal) -> decimal.Decimal:
    """S = sum_{j>=0} z^j / (N + j) by direct summation (60 digits), for 0 < z < 1.

    Terms are positive and decrease at least geometrically (ratio < z), so after adding
    the terms j < J the remainder is at most z^J / (N + J) / (1 - z); summation stops once
    that bound is below 1e-50 of the partial sum.
    """
    with decimal.localcontext(_HP):
        one_minus_z = 1 - z
        bound = decimal.Decimal("1e-50")
        total = decimal.Decimal(0)
        power = decimal.Decimal(1)
        j = 0
        while True:
            total += power / (n_obs + j)
            power *= z
            j += 1
            if power / (n_obs + j) / one_minus_z <= bound * total:
                return total


def _cwj_tail_direct(n_obs: int, a_value: decimal.Decimal, z: decimal.Decimal) -> decimal.Decimal:
    """S = z^(-N) * (-ln A - sum_{r=1}^{N-1} z^r / r) (80 digits), for 0 < z < 1.

    This is the paper's form. It subtracts two nearly equal numbers when z^N is tiny;
    the caller only uses it when z^N >= 1e-15, where at most ~log10(-ln A * N / z^N) <= 25
    digits are lost, leaving >= 55 correct digits.
    """
    with decimal.localcontext(_HP80):
        partial = decimal.Decimal(0)
        power = decimal.Decimal(1)
        for r in range(1, n_obs):
            power *= z
            partial += power / r
        return (-a_value.ln() - partial) / (z ** n_obs)


def chao_wang_jost_decimal(mult: dict[int, int], n_obs: int, force_regime: str | None = None) -> decimal.Decimal:
    """Chao, Wang & Jost (2013) entropy estimator in nats as a Decimal (requires N >= 2).

    Chao A, Wang YT, Jost L. Entropy and the species accumulation curve: a novel entropy
    estimator via discovery rates of new species. Methods Ecol Evol 2013;4:1091-1100,
    doi:10.1111/2041-210X.12108. The estimator, as specified for DiMA 1.0.0::

        H = sum_{i: 0 < c_i < N} (c_i/N) (H_{N-1} - H_{c_i - 1})
            + (f1/N) (1-A)^(1-N) (-ln A - sum_{r=1}^{N-1} (1-A)^r / r)

    with H_m the m-th harmonic number, f1/f2 the numbers of singletons/doubletons and A
    from ``chao_coverage_a``. Part 2 is 0 when f1 == 0 or A == 1. Because
    -ln A = sum_{r>=1} (1-A)^r / r, part 2 equals (f1/N) (1-A) S with
    S = sum_{j>=0} (1-A)^j / (N+j), a sum of positive terms. S is evaluated with the
    paper's form when z^N >= 1e-15 (z = 1-A; little cancellation, N terms) and with the
    positive series otherwise (fast geometric convergence, no cancellation). Both paths
    agree to ~1e-45 where they overlap (checked by --self-test).
    """
    assert n_obs >= 2
    f1 = mult.get(1, 0)
    f2 = mult.get(2, 0)
    with decimal.localcontext(_HP80):
        h_last = _harmonic(n_obs - 1)
        part1 = decimal.Decimal(0)
        for c, f in mult.items():
            if 0 < c < n_obs:
                part1 += decimal.Decimal(f * c) * (h_last - _harmonic(c - 1))
        part1 = part1 / n_obs
        a_frac = chao_coverage_a(n_obs, f1, f2)
        if f1 == 0 or a_frac == 1:
            return part1
        a_value = decimal.Decimal(a_frac.numerator) / decimal.Decimal(a_frac.denominator)
        z_frac = 1 - a_frac
        z = decimal.Decimal(z_frac.numerator) / decimal.Decimal(z_frac.denominator)
        regime = force_regime
        if regime is None:
            regime = "direct" if z ** n_obs >= decimal.Decimal("1e-15") else "series"
        if regime == "direct":
            tail = _cwj_tail_direct(n_obs, a_value, z)
        else:
            tail = _cwj_tail_series(n_obs, z)
        part2 = decimal.Decimal(f1) / n_obs * z * tail
        return part1 + part2


def chao_wang_jost(mult: dict[int, int], n_obs: int) -> float | None:
    """CWJ entropy in bits (nats / ln 2). N == 0 -> None; N == 1 -> 0.0 (single_observation)."""
    if n_obs == 0:
        return None
    if n_obs == 1:
        return 0.0
    with decimal.localcontext(_HP80):
        value = chao_wang_jost_decimal(mult, n_obs) / _LN2_DEC
    return _clean_float(value)


def sample_coverage(mult: dict[int, int], n_obs: int) -> float | None:
    """Sample coverage estimate (Chao & Jost 2012), computed as an exact rational.

    Chao A, Jost L. Coverage-based rarefaction and extrapolation: standardizing samples
    by completeness rather than size. Ecology 2012;93:2533-2547, doi:10.1890/11-1952.1.
    As specified for DiMA 1.0.0: N < 2 -> None; else with f1/f2 the numbers of
    singletons/doubletons:
    f2 > 0: 1 - (f1/N) ((N-1) f1 / ((N-1) f1 + 2 f2));
    f2 == 0 < f1: 1 - (f1/N) ((N-1)(f1-1) / ((N-1)(f1-1) + 2)); otherwise 1.
    """
    if n_obs < 2:
        return None
    f1 = mult.get(1, 0)
    f2 = mult.get(2, 0)
    if f2 > 0:
        value = 1 - Fraction(f1, n_obs) * Fraction((n_obs - 1) * f1, (n_obs - 1) * f1 + 2 * f2)
    elif f1 > 0:
        value = 1 - Fraction(f1, n_obs) * Fraction((n_obs - 1) * (f1 - 1), (n_obs - 1) * (f1 - 1) + 2)
    else:
        value = Fraction(1)
    return _clean_float(float(value))


# --- Binomial machinery ----------------------------------------------------------------

#: Walk cut-off: weights (relative to the starting term, the mode) below this are not
#: generated. The binomial pmf is log-concave, so beyond the cut-off the terms decrease
#: geometrically with ratio r <= exp(-997 ln 2 / d) < 1, d <= n being the distance walked;
#: all omitted terms together are at most w_cut / (1 - r) <= 2**-997 (1 + d / 691), i.e.
#: below 2**-966 of the sum for any n < 10**9. They cannot change a binary64 result
#: (unit roundoff 2**-53): the sum is, for all practical purposes, the full sum over
#: x = 0..n -- it keeps every term the textbook exp(lgamma(...)) form would not
#: underflow to zero.
_WALK_CUTOFF = 2.0 ** -997


def binomial_weights(n: int, a, b) -> tuple[int, list[float]]:
    """Unnormalised pmf of X ~ Binomial(n, p), p = a / (a + b), a > 0, b > 0.

    Returns ``(lo, w)`` with w[i] proportional to P(X = lo + i); the pmf is
    w[i] / fsum(w). ``a`` and ``b`` are either exact positive integers (p = c/N with
    a = c, b = N - c) or positive floats (a = p, b = 1 - p).

    Method: start at the mode m = floor((n+1) p) with weight 1 and walk outwards with
    the exact pmf ratios

        P(X=x+1)/P(X=x) = ((n-x) a) / ((x+1) b),   P(X=x-1)/P(X=x) = (x b) / ((n-x+1) a).

    With integer a, b each ratio is a single correctly rounded division of exact
    integers, and each step adds at most ~1 ulp of relative error, so a weight d steps
    from the mode is accurate to ~d ulp (d <= a few hundred for the terms that matter).
    Normalising by the sum of the weights removes any common factor exactly. This is
    deliberately *not* ``exp(lgamma(n+1) - lgamma(x+1) - lgamma(n-x+1) + ...)``: for
    n ~ 3,000 lgamma(n+1) ~ 2e4 carries an absolute rounding error of several 1e-12, which
    would put a ~1e-11 relative error on every pmf -- a hundred times the 1e-13
    tolerance this oracle is used with. --self-test checks this routine against exact
    rational pmfs, brute-force multinomial enumeration and the lgamma form.
    """
    if isinstance(a, int) and isinstance(b, int):
        mode = ((n + 1) * a) // (a + b)
    else:
        mode = int((n + 1) * (a / (a + b)))
    mode = max(0, min(n, mode))
    cutoff = _WALK_CUTOFF
    up: list[float] = []
    w = 1.0
    x = mode
    while x < n:
        w *= ((n - x) * a) / ((x + 1) * b)
        x += 1
        if w < cutoff:
            break
        up.append(w)
    down: list[float] = []
    w = 1.0
    x = mode
    while x > 0:
        w *= (x * b) / ((n - x + 1) * a)
        x -= 1
        if w < cutoff:
            break
        down.append(w)
    down.reverse()
    return mode - len(down), down + [1.0] + up


def neg_q_log2_q(x: int, n: int) -> float:
    """h(x) = -(x/n) log2(x/n) for 0 <= x <= n, with h(0) = h(n) = 0.

    For x/n >= 1/2 the logarithm is taken as log1p((x - n)/n): (x - n) is exact and the
    quotient is correctly rounded, so ln(x/n) is accurate to ~1 ulp even when x/n is
    close to 1 (log(x/n) would amplify the rounding of x/n by 1/|ln(x/n)|). Overall
    accuracy is a few ulp.
    """
    if x == 0 or x == n:
        return 0.0
    q = x / n
    if 2 * x >= n:
        ln_q = math.log1p((x - n) / n)
    else:
        ln_q = math.log(q)
    return -(q * ln_q) / _LN2_F


@lru_cache(maxsize=64)
def _h_table(n: int) -> tuple[float, ...]:
    """(h(0), h(1), ..., h(n)) for the sample size n.

    Bit-identical to ``neg_q_log2_q(x, n)`` for every x (same operations in the same
    order, written as two comprehensions only for speed; --self-test checks equality):
    x in 1..ceil(n/2)-1 uses log(x/n), x in ceil(n/2)..n-1 uses log1p((x-n)/n).
    """
    half = (n + 1) // 2  # smallest x with 2x >= n (for x >= 1)
    log, log1p, ln2 = math.log, math.log1p, _LN2_F
    low = [-(q * log(q)) / ln2 for q in (x / n for x in range(1, half))]
    high = [-((x / n) * log1p((x - n) / n)) / ln2 for x in range(max(half, 1), n)]
    return tuple([0.0] + low + high + ([0.0] if n >= 1 else []))


@lru_cache(maxsize=1 << 19)
def expected_h(n: int, c: int, n_obs: int) -> float:
    """E[h(X)] for X ~ Binomial(n, c/N), h(x) = -(x/n) log2(x/n), h(0) = 0 (0 < c < N).

    Equals sum_x P(X=x) h(x) = fsum(w_x h(x)) / fsum(w_x) with the weights of
    ``binomial_weights`` (all x in 0..n except terms below 2**-997 of the largest one;
    see ``_WALK_CUTOFF``).
    """
    assert 0 < c < n_obs and n >= 1
    lo, weights = binomial_weights(n, c, n_obs - c)
    h = _h_table(n)[lo:lo + len(weights)]
    return math.fsum(map(operator.mul, weights, h)) / math.fsum(weights)


def expected_plugin_entropy(mult: dict[int, int], n_obs: int, n: int) -> float:
    """E[H_n]: expected plug-in entropy (bits) of n draws with replacement.

    Drawing n k-mers with replacement from the N observed ones, the number of copies of
    a k-mer seen c times is X ~ Binomial(n, c/N), and by linearity of expectation
    E[H_n] = sum_c f_c E[h(X)]. A k-mer with c == N contributes 0 (h(n) = 0).
    """
    return math.fsum(f * expected_h(n, c, n_obs) for c, f in mult.items() if c < n_obs)


def ols_fit(xs: list[float], ys: list[float]) -> tuple[float, float] | None:
    """Ordinary least squares y = intercept + slope x, two-pass centred, all sums fsum.

    mx, my = means; sxx = sum (x - mx)^2; sxy = sum (x - mx)(y - my); slope = sxy/sxx;
    intercept = my - slope mx. Returns None when sxx == 0 (degenerate: all x equal).
    """
    m = len(xs)
    mx = math.fsum(xs) / m
    my = math.fsum(ys) / m
    dx = [x - mx for x in xs]
    sxx = math.fsum(d * d for d in dx)
    if sxx == 0.0 or max(xs) == min(xs):
        return None
    sxy = math.fsum(d * (y - my) for d, y in zip(dx, ys, strict=True))
    slope = sxy / sxx
    return slope, my - slope * mx


def dima_points(n_obs: int, threshold: int) -> list[int]:
    """Sample sizes n of the resampled points (the 100% point N is not included).

    x = ceil(100 T / N) exactly, (100 T + N - 1) // N; start = 50 if x < 50 else x rounded
    up to a multiple of 5; R = start, start+5, ..., <= 95; n = ceil(R N / 100) =
    (R N + 99) // 100 (duplicates kept).
    """
    x = (100 * threshold + n_obs - 1) // n_obs
    start = 50 if x < 50 else ((x + 4) // 5) * 5
    return [(r * n_obs + 99) // 100 for r in range(start, 100, 5)]


def dima_entropy(
    mult: dict[int, int], n_obs: int, threshold: int, plug_in: float | None
) -> tuple[float | None, str, str | None]:
    """DiMA's sample-size-corrected entropy: exact expectation of the Fig. 2 procedure.

    Returns ``(entropy, method, fallback)``.

    * N == 0: (None, "not_estimable", None).
    * N <= T: plug-in, fallback "support_at_or_below_threshold".
    * Points: for each n of ``dima_points`` (x = 1/n, y = E[H_n]) plus the 100% point
      (x = 1/N, y = plug-in H, not resampled). Fewer than 5 points: plug-in, fallback
      "too_few_points".
    * OLS of y on x (``ols_fit``); sxx == 0: plug-in, "degenerate_regression"; otherwise
      (intercept, "dima", None). A negative intercept cannot occur for real count profiles
      (E[H_n] never falls as n grows and never exceeds the plug-in value, so the intercept is
      at least the mean of the points); it raises an internal error, as in DiMA.
    """
    if n_obs == 0:
        return None, "not_estimable", None
    assert plug_in is not None
    if n_obs <= threshold:
        return plug_in, "plug_in", "support_at_or_below_threshold"
    sizes = dima_points(n_obs, threshold)
    if len(sizes) + 1 < 5:  # resampled points + the 100% point
        return plug_in, "plug_in", "too_few_points"
    xs = [1.0 / n for n in sizes] + [1.0 / n_obs]
    ys = [expected_plugin_entropy(mult, n_obs, n) for n in sizes] + [plug_in]
    fit = ols_fit(xs, ys)
    if fit is None:
        return plug_in, "plug_in", "degenerate_regression"
    _, intercept = fit
    if intercept < 0:
        raise RuntimeError(f"internal error: negative expected intercept {intercept!r}")
    return _clean_float(intercept), "dima", None


# ---------------------------------------------------------------------------------------
# Clopper-Pearson lower bound
# ---------------------------------------------------------------------------------------


def binomial_upper_tail(n: int, x: int, p: float) -> float:
    """P(X >= x) for X ~ Binomial(n, p), 0 < p < 1, from ``binomial_weights``."""
    lo, weights = binomial_weights(n, p, 1.0 - p)
    upper = [w for i, w in enumerate(weights) if lo + i >= x]
    return math.fsum(upper) / math.fsum(weights)


@lru_cache(maxsize=None)
def clopper_pearson_lower(x: int, n: int) -> float:
    """Two-sided 95% Clopper-Pearson lower bound for x successes out of n (1 <= x <= n).

    Clopper CJ, Pearson ES. Biometrika 1934;26:404-413, doi:10.1093/biomet/26.4.404.
    x == n: 0.025 ** (1/n) (closed form). Otherwise the p solving P(Bin(n, p) >= x) =
    0.025, by bisection on [0, 1]: the tail is increasing in p; the bracket is halved
    until it consists of two adjacent doubles (at most 1,100 halvings -- more than the
    200 the specification asks for; further halvings cannot change it), and the upper
    end (the smallest double whose tail is >= 0.025) is returned.
    """
    assert 1 <= x <= n
    if x == n:
        return 0.025 ** (1 / n)
    lo, hi = 0.0, 1.0
    for _ in range(1100):
        mid = (lo + hi) / 2
        if mid <= lo or mid >= hi:
            break
        if binomial_upper_tail(n, x, mid) >= 0.025:
            hi = mid
        else:
            lo = mid
    return hi


# ---------------------------------------------------------------------------------------
# Analysis
# ---------------------------------------------------------------------------------------


@dataclass
class Options:
    """Analysis options (already validated)."""

    kmer_length: int | None = None
    threshold: int = 100
    alphabet: str = "auto"
    validation: str = "strict"
    header_format: list[str] | None = None
    metadata_fields: list[str] | None = None
    header_fillna: str = "Unknown"
    hcs_percent_text: str = "100"
    hcs_percent: Fraction = Fraction(100)
    hcs_eligible_only: bool = False
    similarity_weighted_entropy: bool = False


@dataclass
class PositionSummary:
    """What the summaries and HCS need from a position after its JSON is written."""

    support: int
    label: str | None
    entropy: float | None
    top_kmer: str | None
    top_count: int
    similarity_weighted_entropy: float | None = None
    observed_entropy: float | None = None


def parse_hcs_percent(text: str) -> tuple[str, Fraction]:
    """Parse --hcs-min-incidence: a plain decimal P with 50 < P <= 100.

    Returns (canonical text, exact value). The canonical text has no exponent, no
    leading zeros and no trailing fractional zeros ("070.50" -> "70.5").
    """
    if not re.fullmatch(r"[0-9]+(\.[0-9]+)?", text):
        raise UsageError(f"--hcs-min-incidence: {text!r} is not a plain decimal number")
    value = Fraction(text)
    if not (50 < value <= 100):
        raise UsageError(f"--hcs-min-incidence: {text} is outside (50, 100]")
    canonical = format(decimal.Decimal(text).normalize(), "f")
    if "." in canonical:
        canonical = canonical.rstrip("0").rstrip(".")
    return canonical, value


def _summary_entry(values: list[tuple[int, float]], pick) -> dict:
    """{position, entropy, tied_position_count} for the extreme chosen by ``pick``.

    position is the FIRST (1-based) position achieving the extreme; ties are counted by
    exact float equality.
    """
    extreme = pick(v for _, v in values)
    first = next(pos for pos, v in values if v == extreme)
    ties = sum(1 for _, v in values if v == extreme)
    return {"position": first, "entropy": _clean_float(extreme), "tied_position_count": ties}


def entropy_summary(values: list[tuple[int, float]]) -> tuple[float | None, dict | None, dict | None]:
    """(average via fsum / count, lowest entry, highest entry); all None when empty."""
    if not values:
        return None, None, None
    average = math.fsum(v for _, v in values) / len(values)
    return _clean_float(average), _summary_entry(values, min), _summary_entry(values, max)


def build_metadata(record_indices: list[int], metadata: list[tuple[str, ...]], fields: list[str]) -> dict:
    """{field: {value: count}} for the records carrying one variant.

    Fields in header-format order; values sorted by Python ``str`` order (code points).
    """
    out = {}
    for j, field in enumerate(fields):
        tally = Counter(metadata[r][j] for r in record_indices)
        out[field] = {value: tally[value] for value in sorted(tally)}
    return out


def analyse_position(
    p: int,
    seqs: list[bytes],
    valids: list[bytes],
    k: int,
    threshold: int,
    metadata: list[tuple[str, ...]] | None,
    fields: list[str] | None,
    similarity: bool = False,
) -> tuple[dict, PositionSummary]:
    """Count the k-mers starting at 0-based column p and compute every per-position value.

    support = number of records whose k-mer at p is counted (see ``valid_starts``).
    Variant incidence = 100 count / support. distinct_variants_count = number of
    non-Index variants; total_variants_incidence = 100 (sum of non-Index counts) /
    support (None when support == 0); distinct_variants_incidence = 100
    distinct_variants_count / (sum of non-Index counts) (None when that sum is 0).
    Integer quotients use Python's correctly rounded int / int division. With
    ``similarity``, also similarity-weighted entropy and the observed and similarity-weighted
    entropies normalised by log2 of the number of distinct k-mers (None without support;
    the normalised values None with one distinct k-mer); without it the three keys are None.
    """
    if metadata is None:
        kmers = [s[p:p + k] for s, v in zip(seqs, valids, strict=True) if v[p]]
        members = None
    else:
        idx = [r for r, v in enumerate(valids) if v[p]]
        kmers = [seqs[r][p:p + k] for r in idx]
        members = {}
        for r, kmer in zip(idx, kmers, strict=True):
            members.setdefault(kmer, []).append(r)
    support = len(kmers)
    variants = sort_variants(Counter(kmers))
    counts = [c for _, c in variants]
    codes = classify_motifs(counts)
    label = support_label(support, threshold)

    non_index = [c for c, code in zip(counts, codes, strict=True) if code != "I"]
    non_index_sum = sum(non_index)
    distinct_variants_count = len(non_index)
    total_variants_incidence = None if support == 0 else _clean_float(100 * non_index_sum / support)
    distinct_variants_incidence = (
        None if non_index_sum == 0 else _clean_float(100 * distinct_variants_count / non_index_sum)
    )

    mult = multiplicities(counts)
    observed = plug_in_entropy(mult, support)
    entropy, method, fallback = dima_entropy(mult, support, threshold, observed)
    weighted = normalised_observed = normalised_weighted = None
    if similarity and support > 0:
        weighted_dec = similarity_weighted_entropy_decimal(variants, support)
        weighted = _clean_float(weighted_dec)
        normalised_observed = normalised_entropy(plug_in_entropy_decimal(mult, support), len(variants))
        normalised_weighted = normalised_entropy(weighted_dec, len(variants))

    motifs = []
    for (kmer, count), code in zip(variants, codes, strict=True):
        motifs.append(
            {
                "sequence": kmer.decode("ascii"),
                "count": count,
                "incidence": _clean_float(100 * count / support),
                "motif_short": code,
                "motif_long": MOTIF_LONG[code],
                "metadata": None if members is None else build_metadata(members[kmer], metadata, fields),
            }
        )

    obj = {
        "position": p + 1,
        "low_support": label,
        "support": support,
        "distinct_variants_count": distinct_variants_count,
        "distinct_variants_incidence": distinct_variants_incidence,
        "total_variants_incidence": total_variants_incidence,
        "entropy": entropy,
        "entropy_method": method,
        "entropy_fallback": fallback,
        "observed_entropy": observed,
        "miller_madow": miller_madow(mult, support),
        "chao_wang_jost": chao_wang_jost(mult, support),
        "sample_coverage": sample_coverage(mult, support),
        "similarity_weighted_entropy": weighted,
        "normalised_observed_entropy": normalised_observed,
        "normalised_similarity_weighted_entropy": normalised_weighted,
        "diversity_motifs": motifs,
    }
    top_kmer = variants[0][0].decode("ascii") if variants else None
    top_count = variants[0][1] if variants else 0
    return obj, PositionSummary(
        support, label, entropy, top_kmer, top_count, similarity_weighted_entropy=weighted, observed_entropy=observed
    )


def find_hcs(
    summaries: list[PositionSummary],
    k: int,
    threshold: int,
    percent_text: str,
    percent: Fraction,
    eligible_only: bool,
) -> dict:
    """Historically conserved sequences (HCS).

    Candidate position p: an Index exists (top count > 1) and top_count / support >=
    P / 100 exactly (rational comparison; unique because P > 50). Default: every
    position may be a candidate; with eligible_only only positions with support > T.

    Merge, candidates in position order (0-based starts internally, 1-based output):
    p joins the current region iff p <= last + k (overlap or abut; ``last`` = the
    region's last member), its k-mer agrees with the region's sequence on every
    overlapping column, and (eligible_only only) no position q with last < q < p has
    support <= T. Otherwise p starts a new region; ``split_from_previous`` is true only
    when p <= last + k but the sequences disagree.

    Region: start_column = first + 1, end_column = last + k (1-based, inclusive),
    length = end - start + 1, sequence over those columns, members (position, kmer,
    count, support, incidence, conservation_lower_bound_95 = Clopper-Pearson lower
    bound), min_incidence, min_support, low_support_positions = every q in
    [first, last] with support <= T (members or not, 1-based), flagged = non-empty,
    conservation_lower_bound_95 = minimum over members.
    """
    threshold_fraction = percent / 100
    candidates = []
    for p, s in enumerate(summaries):
        if s.top_count > 1 and Fraction(s.top_count, s.support) >= threshold_fraction:
            if eligible_only and s.support <= threshold:
                continue
            candidates.append(p)

    groups: list[dict] = []
    for p in candidates:
        kmer = summaries[p].top_kmer
        if groups:
            group = groups[-1]
            last = group["members"][-1]
            if p <= last + k:
                first = group["members"][0]
                region_seq = group["sequence"]
                agree = all(region_seq[col - first] == kmer[col - p] for col in range(p, last + k))
                low_gap = eligible_only and any(summaries[q].support <= threshold for q in range(last + 1, p))
                if agree and not low_gap:
                    group["sequence"] = region_seq + kmer[last + k - p:]
                    group["members"].append(p)
                    continue
                groups.append({"members": [p], "sequence": kmer, "split": not agree})
                continue
        groups.append({"members": [p], "sequence": kmer, "split": False})

    regions = []
    for number, group in enumerate(groups, start=1):
        first = group["members"][0]
        last = group["members"][-1]
        members = []
        for q in group["members"]:
            s = summaries[q]
            members.append(
                {
                    "position": q + 1,
                    "kmer": s.top_kmer,
                    "count": s.top_count,
                    "support": s.support,
                    "incidence": _clean_float(100 * s.top_count / s.support),
                    "conservation_lower_bound_95": _clean_float(clopper_pearson_lower(s.top_count, s.support)),
                }
            )
        assert len(group["sequence"]) == last + k - first
        low = [q + 1 for q in range(first, last + 1) if summaries[q].support <= threshold]
        regions.append(
            {
                "id": number,
                "start_column": first + 1,
                "end_column": last + k,
                "length": last + k - first,
                "sequence": group["sequence"],
                "min_incidence": min(m["incidence"] for m in members),
                "min_support": min(m["support"] for m in members),
                "members": members,
                "low_support_positions": low,
                "flagged": bool(low),
                "conservation_lower_bound_95": min(m["conservation_lower_bound_95"] for m in members),
                "split_from_previous": group["split"],
            }
        )
    return {
        "threshold_percent": percent_text,
        "eligibility": "support_gt_threshold" if eligible_only else "all_positions",
        "region_count": len(regions),
        "flagged_region_count": sum(1 for r in regions if r["flagged"]),
        "regions": regions,
    }


def _dumps(obj) -> str:
    """The one JSON serialisation used everywhere (NaN/inf rejected)."""
    return json.dumps(obj, sort_keys=False, indent=None, separators=(",", ":"), allow_nan=False)


def analyse(data: bytes, options: Options, out, log=None) -> None:
    """Run the whole analysis on decompressed FASTA bytes and write JSON text to ``out``.

    The output is byte-identical to ``json.dumps(result, sort_keys=False, indent=None,
    separators=(",", ":"))`` of the full result object followed by one newline; it is
    assembled in pieces (positions are serialised one at a time into a temporary file)
    so that the motif lists of large alignments never have to be held in memory at once.
    ``out`` is a binary file object; ``log`` (optional) receives progress messages.
    """
    records = parse_fasta(data)
    length = check_equal_lengths(records)

    if options.alphabet == "auto":
        resolved, rule = easel_guess_alphabet(letter_counts(records))
        if resolved is None:
            raise DataError(
                f"cannot determine the alphabet from the first {probe_count(records)} "
                f"record(s) (Easel rule: {rule}); pass -a protein, -a dna or -a rna"
            )
        alphabet_obj = {"resolved": resolved, "method": "guessed", "rule": rule}
    else:
        resolved = options.alphabet
        alphabet_obj = {"resolved": resolved, "method": "explicit", "rule": None}

    k = options.kmer_length if options.kmer_length is not None else (9 if resolved == PROTEIN else 27)
    if not (1 <= k <= length):
        raise UsageError(f"k-mer length {k} must satisfy 1 <= k <= alignment length ({length})")
    threshold = options.threshold

    table = symbol_table(resolved)
    seqs, masks, unknown, first_seen = validate_symbols(records, table, options.validation)
    if unknown and log is not None:
        total = sum(unknown.values())
        log(f"{total} unknown symbol occurrence(s) treated as excluded")
        if options.validation == "report":
            for b, n in sorted(unknown.items()):
                rec, col = first_seen[b]
                log(f"  {_describe_byte(b)}: {n} occurrence(s), first at record {rec}, column {col}")

    metadata = None
    fields = None
    if options.header_format is not None:
        fields = options.metadata_fields if options.metadata_fields is not None else list(options.header_format)
        metadata, mismatched = parse_metadata(
            records, options.header_format, fields, options.header_fillna, options.validation
        )
        if mismatched and log is not None:
            log(f"{len(mismatched)} header(s) with a wrong field count filled with {options.header_fillna!r}")
            if options.validation == "report":
                for number, line, got in mismatched[:1000]:
                    log(f"  record {number} (line {line}): {got} field(s), expected {len(options.header_format)}")
                if len(mismatched) > 1000:
                    log(f"  ... and {len(mismatched) - 1000} more")

    positions = length - k + 1
    valids = [valid_starts(mask, k, positions) for mask in masks]
    del masks

    summaries: list[PositionSummary] = []
    with tempfile.TemporaryFile() as spool:
        for p in range(positions):
            obj, summary = analyse_position(
                p, seqs, valids, k, threshold, metadata, fields, options.similarity_weighted_entropy
            )
            if p:
                spool.write(b",")
            spool.write(_dumps(obj).encode("ascii"))
            summaries.append(summary)
            if log is not None and positions >= 1000 and (p + 1) % 1000 == 0:
                log(f"position {p + 1}/{positions}")

        counts = Counter(s.label for s in summaries)
        label_counts = {
            "NS": counts.get("NS", 0),
            "LS": counts.get("LS", 0),
            "ELS": counts.get("ELS", 0),
            "eligible": counts.get(None, 0),
        }
        eligible = [(p + 1, s.entropy) for p, s in enumerate(summaries) if s.label is None]
        average, lowest, highest = entropy_summary(eligible)
        everything = [(p + 1, 0.0 if s.entropy is None else s.entropy) for p, s in enumerate(summaries)]
        all_average, all_lowest, all_highest = entropy_summary(everything)
        similarity_summary = None
        if options.similarity_weighted_entropy:
            weighted = [(p + 1, s.similarity_weighted_entropy) for p, s in enumerate(summaries) if s.label is None]
            observed = [(p + 1, s.observed_entropy) for p, s in enumerate(summaries) if s.label is None]
            w_average, w_lowest, w_highest = entropy_summary(weighted)
            similarity_summary = {
                "position_count": len(weighted),
                "average_entropy": w_average,
                "lowest_entropy": w_lowest,
                "highest_entropy": w_highest,
                "average_observed_entropy": entropy_summary(observed)[0],
            }

        head = {
            "reference_version": REFERENCE_VERSION,
            "sequence_count": len(records),
            "alignment_length": length,
            "kmer_length": k,
            "support_threshold": threshold,
            "alphabet": alphabet_obj,
            "support_label_counts": label_counts,
            "low_support_count": label_counts["NS"] + label_counts["LS"] + label_counts["ELS"],
            "average_entropy": average,
            "lowest_entropy": lowest,
            "highest_entropy": highest,
            "eligible_position_count": len(eligible),
            "all_positions_summary": {
                "position_count": positions,
                "average_entropy": all_average,
                "lowest_entropy": all_lowest,
                "highest_entropy": all_highest,
            },
            "similarity_weighted_entropy_summary": similarity_summary,
        }
        hcs = find_hcs(
            summaries, k, threshold, options.hcs_percent_text, options.hcs_percent, options.hcs_eligible_only
        )
        head_text = _dumps(head)
        assert head_text.endswith("}")
        out.write(head_text[:-1].encode("ascii"))
        out.write(b',"positions":[')
        spool.seek(0)
        while True:
            chunk = spool.read(1 << 20)
            if not chunk:
                break
            out.write(chunk)
        out.write(b'],"hcs":')
        out.write(_dumps(hcs).encode("ascii"))
        out.write(b"}\n")


def analyse_to_string(data: bytes, options: Options) -> str:
    """Convenience wrapper used by the self-test: the JSON text (with trailing newline)."""
    buffer = io.BytesIO()
    analyse(data, options, buffer)
    return buffer.getvalue().decode("ascii")


# ---------------------------------------------------------------------------------------
# Command line
# ---------------------------------------------------------------------------------------


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog=PROG,
        description="Independent stdlib-only reference implementation of DiMA (test oracle).",
    )
    parser.add_argument("--self-test", action="store_true", help="run the built-in checks and exit")
    sub = parser.add_subparsers(dest="command")
    a = sub.add_parser("analyze", help="analyse an alignment and print JSON")
    a.add_argument("-i", "--input", required=True, help="FASTA alignment (plain, .gz or .bz2; - = stdin)")
    a.add_argument("-k", "--kmer-length", type=int, default=None, help="k (default 9 protein, 27 dna/rna)")
    a.add_argument("-s", "--support-threshold", type=int, default=100, help="support threshold T (default 100)")
    a.add_argument("-a", "--alphabet", choices=["auto", PROTEIN, DNA, RNA], default="auto")
    a.add_argument("--validation", choices=["strict", "permissive", "report"], default="strict")
    a.add_argument("--header-format", default=None, help="'|'-separated field names, e.g. accession|host|country")
    a.add_argument("--metadata-fields", default=None, help="'|'-separated subset of --header-format")
    a.add_argument("--header-fillna", default="Unknown", help="fill value for missing metadata (default Unknown)")
    a.add_argument("--hcs-min-incidence", default="100", help="HCS index incidence threshold P, 50 < P <= 100")
    a.add_argument("--hcs-eligible-only", action="store_true", help="HCS only from positions with support > T")
    a.add_argument(
        "--similarity-weighted-entropy",
        action="store_true",
        help="also similarity-weighted entropy and the normalised entropies (Wu 2025)",
    )
    a.add_argument("-o", "--output", default=None, help="write JSON here (atomically) instead of stdout")
    return parser


def options_from_args(args: argparse.Namespace) -> Options:
    if args.kmer_length is not None and args.kmer_length < 1:
        raise UsageError(f"-k must be >= 1 (got {args.kmer_length})")
    if args.support_threshold < 0:
        raise UsageError(f"-s must be >= 0 (got {args.support_threshold})")
    header_format = None
    metadata_fields = None
    if args.header_format is not None:
        header_format = parse_field_list(args.header_format, "--header-format")
    if args.metadata_fields is not None:
        if header_format is None:
            raise UsageError("--metadata-fields requires --header-format")
        wanted = parse_field_list(args.metadata_fields, "--metadata-fields")
        unknown = [w for w in wanted if w not in header_format]
        if unknown:
            raise UsageError(f"--metadata-fields: {', '.join(unknown)} not in --header-format")
        metadata_fields = [name for name in header_format if name in wanted]
    percent_text, percent = parse_hcs_percent(args.hcs_min_incidence)
    return Options(
        kmer_length=args.kmer_length,
        threshold=args.support_threshold,
        alphabet=args.alphabet,
        validation=args.validation,
        header_format=header_format,
        metadata_fields=metadata_fields,
        header_fillna=args.header_fillna,
        hcs_percent_text=percent_text,
        hcs_percent=percent,
        hcs_eligible_only=args.hcs_eligible_only,
        similarity_weighted_entropy=args.similarity_weighted_entropy,
    )


def _log(message: str) -> None:
    print(f"{PROG}: {message}", file=sys.stderr, flush=True)


def run_analyze(args: argparse.Namespace) -> int:
    options = options_from_args(args)
    data = read_input(args.input)
    started = time.monotonic()
    if args.output is None:
        analyse(data, options, sys.stdout.buffer, _log)
        sys.stdout.buffer.flush()
    else:
        directory = os.path.dirname(os.path.abspath(args.output))
        fd, tmp_path = tempfile.mkstemp(prefix=".dima_reference.", suffix=".tmp", dir=directory)
        try:
            umask = os.umask(0)
            os.umask(umask)
            os.chmod(tmp_path, 0o666 & ~umask)  # mkstemp creates 0600; use normal file mode
            with os.fdopen(fd, "wb") as fh:
                analyse(data, options, fh, _log)
                fh.flush()
                os.fsync(fh.fileno())
            os.replace(tmp_path, args.output)
        except BaseException:
            try:
                os.unlink(tmp_path)
            except OSError:
                pass
            raise
    _log(f"done in {time.monotonic() - started:.1f} s")
    return EXIT_OK


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        if args.self_test:
            return self_test()
        if args.command == "analyze":
            return run_analyze(args)
        parser.print_usage(sys.stderr)
        _log("error: give a command (analyze) or --self-test")
        return EXIT_USAGE
    except DimaError as exc:
        _log(f"error: {exc}")
        return exc.exit_code
    except BrokenPipeError:
        return EXIT_IO


# ---------------------------------------------------------------------------------------
# Self-test
# ---------------------------------------------------------------------------------------


class _Checker:
    def __init__(self) -> None:
        self.failures: list[str] = []
        self.count = 0

    def check(self, condition: bool, what: str) -> None:
        self.count += 1
        if not condition:
            self.failures.append(what)
            print(f"FAIL: {what}", file=sys.stderr)

    def close(self, what: str, got: float, want: float, rel: float, abs_tol: float = 0.0) -> None:
        ok = got == want or abs(got - want) <= max(rel * abs(want), abs_tol)
        self.check(ok, f"{what}: got {got!r}, want {want!r} (rel {rel})")


def _brute_force_expected_entropy(counts: list[int], n: int) -> float:
    """E[H_n] by enumerating every ordered sample of n draws (itertools.product)."""
    total = sum(counts)
    probs = [Fraction(c, total) for c in counts]
    terms = []
    for draw in itertools.product(range(len(counts)), repeat=n):
        weight = Fraction(1)
        for i in draw:
            weight *= probs[i]
        tally = Counter(draw)
        h = math.fsum(neg_q_log2_q(m, n) for m in tally.values())
        terms.append(float(weight) * h)
    return math.fsum(terms)


def _exact_expected_h(n: int, c: int, n_obs: int) -> float:
    """E[h(X)] with exact rational pmf C(n,x) c^x (N-c)^(n-x) / N^n (each correctly rounded)."""
    denominator = n_obs ** n
    terms = []
    for x in range(n + 1):
        pmf = Fraction(math.comb(n, x) * c ** x * (n_obs - c) ** (n - x), denominator)
        terms.append(float(pmf) * neg_q_log2_q(x, n))
    return math.fsum(terms)


def _lgamma_expected_h(n: int, c: int, n_obs: int) -> float:
    """E[h(X)] with the textbook log-space lgamma pmf (cross-check, ~1e-12 accurate)."""
    p = c / n_obs
    lp, lq = math.log(p), math.log1p(-p)
    terms = []
    for x in range(n + 1):
        log_pmf = math.lgamma(n + 1) - math.lgamma(x + 1) - math.lgamma(n - x + 1) + x * lp + (n - x) * lq
        terms.append(math.exp(log_pmf) * neg_q_log2_q(x, n))
    return math.fsum(terms)


def _exact_upper_tail(n: int, x: int, p: float) -> Fraction:
    q = Fraction(p)
    return sum(math.comb(n, j) * q ** j * (1 - q) ** (n - j) for j in range(x, n + 1))


def _cwj_float_direct(counts: list[int]) -> float:
    """CWJ in bits with the paper's formula in plain floats (small N only)."""
    n = sum(counts)
    f1 = counts.count(1)
    f2 = counts.count(2)
    part1 = math.fsum((c / n) * math.fsum(1 / j for j in range(c, n)) for c in counts if c < n)
    a = float(chao_coverage_a(n, f1, f2))
    if f1 == 0 or a == 1.0:
        part2 = 0.0
    else:
        part2 = (f1 / n) * (1 - a) ** (1 - n) * (-math.log(a) - math.fsum((1 - a) ** r / r for r in range(1, n)))
    return (part1 + part2) / math.log(2)


def _cwj_exact_part1(counts: list[int]) -> Fraction:
    n = sum(counts)
    return sum(
        (Fraction(c, n) * sum(Fraction(1, j) for j in range(c, n)) for c in counts if c < n),
        Fraction(0),
    )


def self_test() -> int:
    """Run the built-in checks; return 0 when all pass, 1 otherwise."""
    started = time.monotonic()
    t = _Checker()

    # 1. E[H_n] (binomial walk) == brute-force multinomial enumeration.
    cases = [
        ([1, 1], 2), ([1, 1], 3), ([2, 1], 4), ([3, 2, 1], 5), ([4, 2, 1, 1], 6),
        ([5, 3], 6), ([1, 1, 1, 1, 1, 1, 1, 1], 4), ([6, 1, 1], 6), ([7, 1], 6), ([2, 2, 2, 2], 5),
        ([3, 3, 1, 1], 1), ([1, 2], 6), ([4, 4], 2),
    ]
    for counts, n in cases:
        mult = multiplicities(counts)
        got = expected_plugin_entropy(mult, sum(counts), n)
        want = _brute_force_expected_entropy(counts, n)
        t.close(f"E[H_{n}] counts={counts} vs brute force", got, want, 1e-13, 1e-16)

    # 1b. expected_h against exact rational pmfs and the lgamma form, larger n.
    for n, c, n_obs in [(30, 3, 37), (50, 1, 97), (85, 60, 170), (120, 129, 170), (200, 499, 500), (300, 2, 499)]:
        got = expected_h(n, c, n_obs)
        t.close(f"expected_h({n},{c},{n_obs}) vs exact rational", got, _exact_expected_h(n, c, n_obs), 1e-14)
        t.close(f"expected_h({n},{c},{n_obs}) vs lgamma", got, _lgamma_expected_h(n, c, n_obs), 1e-10)
    for n, c, n_obs in [(2400, 1300, 2637), (2000, 1, 2637), (3000, 3755, 3756)]:
        t.close(f"expected_h({n},{c},{n_obs}) vs lgamma", expected_h(n, c, n_obs),
                _lgamma_expected_h(n, c, n_obs), 1e-9)
    for n in (1, 2, 3, 4, 5, 10, 99, 100, 101, 2637):
        t.check(_h_table(n) == tuple(neg_q_log2_q(x, n) for x in range(n + 1)), f"h table n={n} bit-identical")
    lo, weights = binomial_weights(10, 3, 7)
    exact = [Fraction(math.comb(10, x) * 3 ** x * 7 ** (10 - x), 10 ** 10) for x in range(11)]
    total = math.fsum(weights)
    for i, w in enumerate(weights):
        t.close(f"binomial_weights pmf x={lo + i}", w / total, float(exact[lo + i]), 1e-14)
    t.check(lo == 0 and len(weights) == 11, "binomial_weights covers 0..n for small n")

    # 2. dima estimator: points, fallbacks.
    t.check(dima_points(170, 100) == [102, 111, 119, 128, 136, 145, 153, 162], "dima_points(170,100)")
    t.check(dima_points(170, 30) == [85, 94, 102, 111, 119, 128, 136, 145, 153, 162],
            "dima_points(170,30) uses R=50..95")
    t.check(dima_points(101, 100) == [], "dima_points(101,100) has no resampled points")
    t.check(dima_entropy({}, 0, 100, None) == (None, "not_estimable", None), "dima N=0")
    t.check(dima_entropy({1: 3}, 3, 100, 1.5)[1:] == ("plug_in", "support_at_or_below_threshold"), "dima N<T")
    t.check(dima_entropy({100: 1}, 100, 100, 0.0)[1:] == ("plug_in", "support_at_or_below_threshold"), "dima N=T")
    mult = multiplicities([50, 30, 20])
    t.check(dima_entropy(mult, 100, 95, plug_in_entropy(mult, 100))[1:] == ("plug_in", "too_few_points"),
            "dima T=95 N=100 too few points")
    t.check(dima_entropy(mult, 100, 85, plug_in_entropy(mult, 100))[1:] == ("plug_in", "too_few_points"),
            "dima T=85 N=100: R=85,90,95 + 100% = 4 points")
    t.check(dima_entropy(mult, 100, 80, plug_in_entropy(mult, 100))[1:] == ("dima", None),
            "dima T=80 N=100: R=80..95 + 100% = 5 points")
    t.check(dima_entropy({1: 1}, 1, 0, 0.0)[1:] == ("plug_in", "degenerate_regression"), "dima N=1 T=0 degenerate")
    t.check(ols_fit([0.5, 0.25, 0.125, 0.1, 0.05], [5.0, 3.0, 2.0, 1.9, 1.5]) is not None, "ols fit")
    slope, intercept = ols_fit([1.0, 2.0, 3.0], [1.0, 3.0, 5.0])
    t.close("ols slope", slope, 2.0, 0.0)
    t.close("ols intercept", intercept, -1.0, 0.0)
    t.check(ols_fit([0.5, 0.5, 0.5], [1.0, 2.0, 3.0]) is None, "ols degenerate")
    # Real count profiles never reach this branch (E[H_n] never falls as n grows towards
    # the plug-in H, which bounds it by Jensen's inequality, so the slope on 1/n is <= 0
    # and the intercept >= the mean y >= 0); exercise it with a hand-made regression.
    xs_up = [0.02, 0.015, 0.012, 0.011, 0.01]
    t.check(ols_fit(xs_up, [10 * x - 0.05 for x in xs_up])[1] < 0, "ols negative intercept example")
    # Through dima_entropy itself: an (artificially) zero 100% point under positive
    # resampled points makes the slope positive and the intercept negative.
    try:
        dima_entropy(mult, 100, 80, 0.0)
        raised = False
    except RuntimeError:
        raised = True
    t.check(raised, "dima negative intercept is an internal error")
    one = multiplicities([7])
    t.check(dima_entropy(one, 7, 0, 0.0) == (0.0, "dima", None), "dima single variant -> 0")

    # 3. Chao-Wang-Jost.
    small_cases = [[1, 1], [2, 1], [3, 1, 1], [2, 2, 1], [1, 1, 1, 1, 1], [5, 2, 2], [4, 3], [6, 1, 1, 1, 2],
                   [9, 1], [3, 3, 3], [1, 2, 3, 4, 5], [10, 2, 1, 1, 1, 1, 1]]
    for counts in small_cases:
        n = sum(counts)
        mult = multiplicities(counts)
        got = chao_wang_jost(mult, n)
        t.close(f"CWJ {counts} vs float formula", got, _cwj_float_direct(counts), 1e-12)
        part1 = float(_cwj_exact_part1(counts)) / math.log(2)
        if mult.get(1, 0) == 0:
            t.close(f"CWJ {counts} f1=0 equals part 1", got, part1, 1e-14)
    t.check(chao_wang_jost({}, 0) is None and chao_wang_jost({1: 1}, 1) == 0.0, "CWJ N=0/N=1")
    # A == 1 (f1 = 1, f2 = 0): part 2 vanishes.
    t.check(chao_coverage_a(10, 1, 0) == 1, "A==1 when f1=1,f2=0")
    t.close("CWJ A==1", chao_wang_jost(multiplicities([9, 1]), 10),
            float(_cwj_exact_part1([9, 1])) / math.log(2), 1e-14)
    # All singletons (f1 = N): f2 = 0 branch.
    for n in (2, 5, 40, 300):
        counts = [1] * n
        got = chao_wang_jost(multiplicities(counts), n)
        if n <= 40:
            t.close(f"CWJ all singletons N={n} vs float", got, _cwj_float_direct(counts), 1e-10)
        t.check(got > math.log2(n), f"CWJ all singletons N={n} exceeds log2 N")
    # The two tail regimes agree where both are accurate.
    for counts in ([2] * 30 + [1] * 3, [1] * 20 + [2] * 40 + [5], [3] * 100 + [1] * 2 + [2] * 50, [1, 2, 2, 2, 7]):
        n = sum(counts)
        mult = multiplicities(counts)
        with decimal.localcontext(_HP80):
            direct = chao_wang_jost_decimal(mult, n, "direct")
            series = chao_wang_jost_decimal(mult, n, "series")
            t.check(abs(direct - series) <= decimal.Decimal("1e-40") * abs(series),
                    f"CWJ regimes agree for {sorted(mult.items())}: {direct} vs {series}")

    # Sample coverage.
    t.check(sample_coverage({}, 0) is None and sample_coverage({1: 1}, 1) is None, "coverage N<2")
    t.close("coverage f2>0", sample_coverage(multiplicities([5, 2, 1, 1]), 9),
            float(1 - Fraction(2, 9) * Fraction(8 * 2, 8 * 2 + 2)), 0.0)
    t.close("coverage f2=0", sample_coverage(multiplicities([5, 1, 1, 1]), 8),
            float(1 - Fraction(3, 8) * Fraction(7 * 2, 7 * 2 + 2)), 0.0)
    t.check(sample_coverage(multiplicities([5, 5]), 10) == 1.0, "coverage no singletons")

    # Plug-in and Miller-Madow.
    t.close("plug-in [1,1]", plug_in_entropy({1: 2}, 2), 1.0, 0.0)
    t.close("plug-in [2,1,1]", plug_in_entropy(multiplicities([2, 1, 1]), 4), 1.5, 0.0)
    t.check(plug_in_entropy({5: 1}, 5) == 0.0, "plug-in single variant")
    t.close("Miller-Madow [2,1,1]", miller_madow(multiplicities([2, 1, 1]), 4), 1.5 + 2 / (8 * math.log(2)), 1e-15)
    t.check(miller_madow({1: 1}, 1) == 0.0 and miller_madow({}, 0) is None, "Miller-Madow N=1/N=0")

    # 4. Clopper-Pearson.
    for n in (1, 3, 18, 499):
        t.close(f"CP x=n={n}", clopper_pearson_lower(n, n), 0.025 ** (1 / n), 0.0)
    t.close("CP 3/3 value", clopper_pearson_lower(3, 3), 0.2924017738212866, 1e-15)
    for x, n in ((1, 2), (2, 3), (5, 10), (9, 10), (80, 100), (129, 170), (350, 499), (498, 499)):
        p = clopper_pearson_lower(x, n)
        tail = _exact_upper_tail(n, x, p)
        t.check(abs(float(tail) - 0.025) <= 1e-12, f"CP tail({x},{n}) at p={p!r}: {float(tail)!r}")
        below = _exact_upper_tail(n, x, math.nextafter(p, 0.0)) if hasattr(math, "nextafter") else Fraction(0)
        t.check(below <= Fraction(1, 40) + Fraction(1, 10 ** 12), f"CP({x},{n}) is the smallest root")
    # Known value (R: binom.test(5, 10)$conf.int[1] = 0.187086028).
    t.close("CP 5/10", clopper_pearson_lower(5, 10), 0.187086028, 1e-8)

    # 5. Motif rules.
    t.check(classify_motifs([5, 5, 2, 1]) == ["I", "I", "Ma", "U"], "motifs: tied index")
    t.check(classify_motifs([1, 1, 1]) == ["U", "U", "U"], "motifs: all unique")
    t.check(classify_motifs([3, 2, 2, 1]) == ["I", "Ma", "Ma", "U"], "motifs: tied major")
    t.check(classify_motifs([4, 3, 2, 2, 1]) == ["I", "Ma", "Mi", "Mi", "U"], "motifs: minors")
    t.check(classify_motifs([7]) == ["I"], "motifs: single conserved")
    t.check(classify_motifs([1]) == ["U"], "motifs: single observation is unique")
    t.check(classify_motifs([2, 2]) == ["I", "I"], "motifs: two tied doubletons")
    t.check(classify_motifs([6, 1, 1]) == ["I", "U", "U"], "motifs: index + uniques, no major")
    t.check(classify_motifs([]) == [], "motifs: empty")

    # 6. Similarity-weighted entropy (Wu 2025).
    _self_test_similarity(t)

    # Parsing, alphabet, symbols, metadata, HCS and the JSON assembly.
    _self_test_pipeline(t)

    elapsed = time.monotonic() - started
    if t.failures:
        print(f"{PROG}: self-test FAILED: {len(t.failures)} of {t.count} checks ({elapsed:.1f} s)", file=sys.stderr)
        return EXIT_INTERNAL
    print(f"{PROG}: self-test passed: {t.count} checks ({elapsed:.1f} s)", file=sys.stderr)
    return EXIT_OK


def _self_test_similarity(t: _Checker) -> None:
    """Similarity-weighted entropy against direct counting and closed forms."""
    # matches() (zero bytes of the XOR) == counting equal positions directly.
    pairs = [
        (b"A", b"A"), (b"A", b"C"), (b"ACGT", b"ACGT"), (b"ACGT", b"TGCA"), (b"ACGT", b"ACGA"),
        (b"MKTAYIAKQR", b"MKTAYLAKQR"), (b"A" * 1000, b"A" * 999 + b"C"), (b"AC" * 500, b"CA" * 500),
    ]
    for x, y in pairs:
        k = len(x)
        direct = sum(a == b for a, b in zip(x, y, strict=True))
        t.check(matches(int.from_bytes(x, "big"), int.from_bytes(y, "big"), k) == direct, f"matches {x[:8]!r}..")

    def h_sim(variants):
        return _clean_float(similarity_weighted_entropy_decimal(variants, sum(c for _, c in variants)))

    # Saturated pools (Wu 2025, Figs 4 and 5c): m free sites of k at equal frequencies give
    # weights 4^-m + 3m/(4k) and H = 2m bits, so H_sim = 2m (4^-m + 3m/(4k)).
    def saturated(m, k, letters):
        return [
            (bytes(letters[(x // len(letters) ** s) % len(letters)] for s in range(m)) + letters[0:1] * (k - m), 1)
            for x in range(len(letters) ** m)
        ]

    for k in range(1, 6):
        want = 2 * k * (Fraction(1, 4 ** k) + Fraction(3, 4))
        t.check(h_sim(saturated(k, k, b"ACGT")) == float(want), f"saturated pool k={k}")
    t.check(h_sim(saturated(4, 6, b"ACGU")) == 129 / 32, "Wu Fig. 5c pool: 129/32")
    # k = 1: every weight is 1, so H_sim equals the plug-in entropy.
    singles = [(b"A", 5), (b"C", 3), (b"G", 1)]
    t.check(h_sim(singles) == plug_in_entropy(multiplicities([5, 3, 1]), 9), "k = 1 gives plug-in entropy")
    # One distinct k-mer: 0, and the normalised values are undefined.
    t.check(h_sim([(b"ACG", 7)]) == 0.0, "one k-mer gives 0")
    t.check(normalised_entropy(decimal.Decimal(0), 1) is None, "normalised undefined at D = 1")
    # Two equal-count variants at distance d: (k + d) / (2k).
    for k, d, c in [(3, 1, 4), (9, 2, 7), (27, 27, 1), (10, 5, 1000)]:
        x = b"A" * k
        y = b"C" * d + b"A" * (k - d)
        t.check(h_sim([(x, c), (y, c)]) == (k + d) / (2 * k), f"two variants k={k} d={d}")
    # A uniform population: H_n is exactly 1 (capped), H_nsim below 1 when similar.
    uniform = [(b"AA", 1), (b"AC", 1), (b"CA", 1), (b"CC", 1)]
    t.check(normalised_entropy(plug_in_entropy_decimal(multiplicities([1, 1, 1, 1]), 4), 4) == 1.0, "H_n = 1")
    hn_sim = normalised_entropy(similarity_weighted_entropy_decimal(uniform, 4), 4)
    t.check(hn_sim is not None and 0 < hn_sim < 1, "H_nsim below 1")


def _expect_error(t: _Checker, what: str, func, error_type=DimaError, fragment: str = "") -> None:
    try:
        func()
    except error_type as exc:
        t.check(fragment in str(exc), f"{what}: message {str(exc)!r} lacks {fragment!r}")
        return
    except Exception as exc:  # pragma: no cover - reported as a failure
        t.check(False, f"{what}: raised {type(exc).__name__}: {exc}")
        return
    t.check(False, f"{what}: no error raised")


def _self_test_pipeline(t: _Checker) -> None:
    # Parsing.
    recs = parse_fasta(_BOM + b"\n  \r\n>a x|y\r\nac gt\r\n\tAC\x0b\x0cGT\r\n>b\nACGTACGT\n")
    t.check([r.header for r in recs] == [b"a x|y", b"b"], "parse: headers, CR removed")
    t.check([r.seq for r in recs] == [b"ACGTACGT", b"ACGTACGT"], "parse: whitespace removed, folded")
    t.check([r.size for r in recs] == [24, 12], "parse: record sizes as stored")
    t.check([r.size for r in parse_fasta(b">a\nAC")] == [6], "parse: a final line feed is counted")
    # The first batch: 10,000 records, or 64 MiB once it has 1,000.
    small = [Record(i + 1, b"", b"", 1, 10) for i in range(12_000)]
    t.check(probe_count(small) == 10_000, "probe: record limit")
    big = [Record(i + 1, b"", b"", 1, 1 << 20) for i in range(2_000)]
    t.check(probe_count(big) == 1_000, "probe: at least 1,000 records")
    t.check(probe_count(big[:5]) == 5, "probe: short input")
    _expect_error(t, "parse: empty", lambda: parse_fasta(b""), DataError, "empty")
    _expect_error(t, "parse: no records", lambda: parse_fasta(b"\n \n"), DataError, "no FASTA records")
    _expect_error(t, "parse: white space only", lambda: parse_fasta(b"\t\x0b\x0c\r\n \r"), DataError,
                  "no FASTA records")
    t.check([r.seq for r in parse_fasta(b" \x0b\n\x0c\t\r\n>a\nAC\n")] == [b"AC"],
            "parse: white-space lines before the first record are skipped")
    _expect_error(t, "parse: data first", lambda: parse_fasta(b"AC\n>a\nAC\n"), DataError, "before the first")
    _expect_error(t, "parse: indented header", lambda: parse_fasta(b"\n  >a\nAC\n"), DataError,
                  "before the first")
    _expect_error(t, "parse: no-break space first", lambda: parse_fasta(b"\xc2\xa0\n>a\nAC\n"), DataError,
                  "before the first")
    _expect_error(t, "parse: NUL", lambda: parse_fasta(b">a\nA\x00C\n"), DataError, "NUL")
    _expect_error(t, "parse: unaligned", lambda: check_equal_lengths(parse_fasta(b">a\nACG\n>b\nAC\n")),
                  DataError, "not aligned")
    t.check(decompress_bytes(gzip.compress(b">a\nAC\n") + gzip.compress(b">b\nAC\n")) == b">a\nAC\n>b\nAC\n",
            "gzip multi-member")
    t.check(decompress_bytes(bz2.compress(b">a\n") + bz2.compress(b"AC\n")) == b">a\nAC\n", "bzip2 multi-stream")
    _expect_error(t, "xz refused", lambda: decompress_bytes(b"\xfd7zXZ\x00\x00\x04"), DataError, "xz (.xz)")
    _expect_error(t, "bzip2 trailing garbage", lambda: decompress_bytes(bz2.compress(b">a\n") + b"junk"),
                  DataError, "4 bytes that are not compressed data")
    _expect_error(t, "gzip trailing garbage", lambda: decompress_bytes(gzip.compress(b">a\n") + b"\0junk"),
                  DataError, "5 bytes that are not compressed data")
    t.check(decompress_bytes(gzip.compress(b">a\n") + b"\0" * 100) == b">a\n", "gzip zero padding")
    t.check(decompress_bytes(bz2.compress(b">a\n") + b"\0") == b">a\n", "bzip2 zero padding")
    _expect_error(t, "bzip2 truncated", lambda: decompress_bytes(bz2.compress(b">a\nACGT\n" * 50)[:-9]),
                  DataError, "truncated")
    _expect_error(t, "gzip truncated", lambda: decompress_bytes(gzip.compress(b">a\nACGT\n" * 50)[:-9]),
                  DataError, "truncated")
    _expect_error(t, "parse: CR in a header", lambda: parse_fasta(b">a\nAC\n>b\rc\nAC\n"), DataError,
                  "record 2, line 3: carriage return")
    _expect_error(t, "parse: NUL in a header", lambda: parse_fasta(b">a\x00\nAC\n"), DataError, "NUL byte inside")

    # Alphabet guess (Easel).
    def ct_of(text: str) -> list[int]:
        return [text.upper().count(chr(ord("A") + i)) for i in range(26)]

    t.check(easel_guess_alphabet(ct_of("ACGTACGTAC")) == (None, "too_few_residues"), "easel: n <= 10")
    t.check(easel_guess_alphabet(ct_of("N" * 2001)) == (DNA, "all_n"), "easel: all N")
    t.check(easel_guess_alphabet(ct_of("ACGTACGTACGL")) == (PROTEIN, "amino_only_letter"), "easel: L")
    t.check(easel_guess_alphabet(ct_of("acgtacgtacgtn")) == (DNA, "dna_composition"), "easel: dna")
    t.check(easel_guess_alphabet(ct_of("ACGUACGUACGU")) == (RNA, "rna_composition"), "easel: rna")
    t.check(easel_guess_alphabet(ct_of("ACGTACGTACGTACGTUUUUU")) == (None, "undetermined"), "easel: T and U")
    # Easel's last (amino composition) branch is unreachable as written: it needs
    # x1+x2+x3+xn+xt >= 15 but x1 == 0 there, so the sum is at most 0+3+9+1+1 = 14.
    t.check(easel_guess_alphabet(ct_of("ACDGHKMNRSTVWYACDGHKMNRSTVWY" + "DHKMRSVWY")) == (None, "undetermined"),
            "easel: amino composition branch cannot fire without E/F/I/J/L/O/P/Q/Z")
    t.check(easel_guess_alphabet(ct_of("ACGTRYACGTRY")) == (None, "undetermined"), "easel: ambiguous")

    # Symbol classes.
    prot = symbol_table(PROTEIN)
    dna = symbol_table(DNA)
    rna = symbol_table(RNA)
    t.check(all(prot.classes[b] != UNKNOWN for b in range(ord("A"), ord("Z") + 1)), "protein: every letter known")
    t.check(dna.classes[ord("U")] == RESIDUE and b"U".translate(dna.fold) == b"T", "dna: U folded to T")
    t.check(rna.classes[ord("T")] == RESIDUE and b"T".translate(rna.fold) == b"U", "rna: T folded to U")
    t.check(dna.classes[ord("E")] == UNKNOWN and dna.classes[ord("N")] == EXCLUDED, "dna: E unknown, N excluded")
    for sym in b"-._~?*$^#":
        t.check(prot.classes[sym] == EXCLUDED and dna.classes[sym] == EXCLUDED, f"excluded {chr(sym)!r}")
    t.check(valid_starts(b"1101111011", 2, 9) == bytes([1, 0, 0, 1, 1, 1, 0, 0, 1]), "valid_starts")

    # Metadata cleaning.
    fill = "Unknown"
    t.check(clean_metadata_value("  Homo​ sapiens　", fill) == "Homo sapiens", "clean: Cf + White_Space")
    t.check(clean_metadata_value("\t\x01\x85 ", fill) == fill, "clean: controls and blanks -> fill")
    t.check(clean_metadata_value("a\tb", fill) == "a\tb", "clean: inner TAB kept")
    t.check(clean_metadata_value("Café", fill) == "Café", "clean: NFC")
    t.check(clean_metadata_value("\x1cX\x1f", fill) == "X", "clean: C0 separators removed")
    t.check(clean_metadata_value(" ", fill) == fill, "clean: U+2000 stripped")
    t.check(clean_metadata_value("x\x7f", fill) == "x\x7f", "clean: DEL kept")

    # Whole pipeline on a small protein alignment.
    fasta = (
        ">s1|Human|2020\nMKTAYIAKQR\n"
        ">s2|Human|2021\nMKTAYIAKQR\n"
        ">s3|Bat|2020\nMKTAYLAKQR\n"
        ">s4|​|2019\nmkta-iakqr\n"
        ">s5|Camel\nMKTAYIAKQX\n"
    ).encode("utf-8")
    opts = Options(kmer_length=3, threshold=4, alphabet=PROTEIN, validation="permissive",
                   header_format=["id", "host", "year"], metadata_fields=["host", "year"])
    text = analyse_to_string(fasta, opts)
    result = json.loads(text)
    t.check(text.endswith("}\n") and text.count("\n") == 1, "json: single line with trailing newline")
    t.check(result["sequence_count"] == 5 and result["alignment_length"] == 10 and result["kmer_length"] == 3,
            "pipeline: sizes")
    pos = result["positions"]
    supports = [p["support"] for p in pos]
    t.check(supports == [5, 5, 4, 4, 4, 5, 5, 4], f"pipeline: supports {supports}")
    t.check([p["low_support"] for p in pos] == [None, None, "ELS", "ELS", "ELS", None, None, "ELS"],
            "pipeline: labels")
    first = pos[0]["diversity_motifs"][0]
    t.check(first["sequence"] == "MKT" and first["count"] == 5 and first["motif_short"] == "I", "pipeline: index")
    t.check(first["metadata"] == {"host": {"Bat": 1, "Human": 2, "Unknown": 2}, "year": {"2019": 1, "2020": 2,
                                  "2021": 1, "Unknown": 1}}, f"pipeline: metadata {first['metadata']}")
    last = pos[7]
    t.check([m["sequence"] for m in last["diversity_motifs"]] == ["KQR"], "pipeline: X excluded")
    t.check(result["support_label_counts"] == {"NS": 0, "LS": 0, "ELS": 4, "eligible": 4}, "pipeline: label counts")
    t.check(result["alphabet"] == {"resolved": PROTEIN, "method": "explicit", "rule": None}, "pipeline: alphabet")
    # Streaming assembly is byte-identical to one json.dumps of the parsed object.
    t.check(text == _dumps(result) + "\n", "json: streamed output equals json.dumps of the whole object")

    # Strict mode errors.
    strict = Options(kmer_length=3, threshold=3, alphabet=PROTEIN, validation="strict",
                     header_format=["id", "host", "year"])
    _expect_error(t, "strict: header field count", lambda: analyse_to_string(fasta, strict), DataError,
                  "record 5 (line 9) has 2")
    _expect_error(t, "strict: unknown symbol",
                  lambda: analyse_to_string(b">a\nMKT1A\n", Options(kmer_length=2, alphabet=PROTEIN)),
                  DataError, "record 1 (line 1), column 4")
    _expect_error(t, "strict: invalid UTF-8 header",
                  lambda: analyse_to_string(b">a|\xff\nMKTA\n", Options(kmer_length=2, alphabet=PROTEIN,
                                                                         header_format=["a", "b"])),
                  DataError, "not valid UTF-8")
    _expect_error(t, "k > L", lambda: analyse_to_string(b">a\nMKTA\n", Options(kmer_length=5, alphabet=PROTEIN)),
                  UsageError, "1 <= k <= alignment length")
    _expect_error(t, "undetermined alphabet", lambda: analyse_to_string(b">a\nACGT\n", Options()),
                  DataError, "too_few_residues")

    # HCS merging: overlap, abut, conflict split, low-support gaps.
    seqs = [b"ACDEFGHIKLMN"] * 4
    fasta2 = b"".join(b">r%d\n%s\n" % (i, s) for i, s in enumerate(seqs))
    hcs = json.loads(analyse_to_string(fasta2, Options(kmer_length=3, threshold=2, alphabet=PROTEIN)))["hcs"]
    t.check(hcs["region_count"] == 1 and hcs["regions"][0]["sequence"] == "ACDEFGHIKLMN"
            and hcs["regions"][0]["start_column"] == 1 and hcs["regions"][0]["end_column"] == 12
            and hcs["regions"][0]["length"] == 12 and not hcs["regions"][0]["flagged"], f"hcs: one region {hcs}")
    t.close("hcs: CP bound 4/4", hcs["regions"][0]["conservation_lower_bound_95"], 0.025 ** 0.25, 0.0)

    summaries = [PositionSummary(10, None, 0.0, kmer, 10) for kmer in ("ACD", "CDE", "XEF", "EFG")]
    regions = find_hcs(summaries, 3, 5, "100", Fraction(100), False)["regions"]
    t.check([(r["start_column"], r["end_column"], r["split_from_previous"]) for r in regions]
            == [(1, 4, False), (3, 6, True)], f"hcs: conflict split {regions}")
    t.check([r["sequence"] for r in regions] == ["ACDE", "XEFG"], "hcs: region sequences")
    summaries = [PositionSummary(10, None, 0.0, "ACD", 10), PositionSummary(0, "NS", None, None, 0),
                 PositionSummary(0, "NS", None, None, 0), PositionSummary(10, None, 0.0, "EFG", 10),
                 PositionSummary(10, None, 0.0, "QQQ", 10), PositionSummary(4, "LS", 0.0, "QQR", 4)]
    out_all = find_hcs(summaries, 3, 5, "100", Fraction(100), False)
    t.check([(r["start_column"], r["end_column"]) for r in out_all["regions"]] == [(1, 6), (5, 8)],
            f"hcs: abut joins, conflict splits {out_all['regions']}")
    t.check(out_all["regions"][0]["sequence"] == "ACDEFG" and out_all["regions"][0]["low_support_positions"] == [2, 3]
            and out_all["regions"][0]["flagged"], "hcs: low-support positions inside region")
    t.check(out_all["regions"][1]["split_from_previous"] and out_all["regions"][1]["low_support_positions"] == [6]
            and out_all["regions"][1]["min_support"] == 4, "hcs: low-support member flagged")
    out_elig = find_hcs(summaries, 3, 5, "100", Fraction(100), True)
    t.check([(r["start_column"], r["end_column"], r["split_from_previous"]) for r in out_elig["regions"]]
            == [(1, 3, False), (4, 6, False), (5, 7, True)], f"hcs: eligible-only gap split {out_elig['regions']}")
    t.check(out_elig["eligibility"] == "support_gt_threshold" and out_all["eligibility"] == "all_positions",
            "hcs: eligibility names")
    t.check(parse_hcs_percent("070.50") == ("70.5", Fraction(141, 2)) and parse_hcs_percent("100")[0] == "100",
            "hcs: percent parsing")
    summaries = [PositionSummary(10, None, 0.0, "ACD", 7), PositionSummary(10, None, 0.0, "CDE", 6)]
    t.check(find_hcs(summaries, 3, 5, "70", Fraction(70), False)["region_count"] == 1, "hcs: 70% exact boundary")
    t.check(find_hcs(summaries, 3, 5, "60.5", Fraction(121, 2), False)["regions"][0]["members"][-1]["position"] == 1,
            "hcs: 60% < 60.5%")
    _expect_error(t, "hcs: 50 rejected", lambda: parse_hcs_percent("50"), UsageError, "outside")


if __name__ == "__main__":
    sys.exit(main())
