#!/usr/bin/env python3
"""Checks that Windows programs start without the Microsoft Visual C++ Redistributable.

The Windows builds link the C runtime statically (`.cargo/config.toml`): neither the Visual C++
runtime (vcruntime140.dll, which only the Redistributable installs) nor the Universal CRT may be
imported. This reads the import and delay-load import tables of PE32/PE32+ images (x86, x86_64,
ARM64; PE format specification, sections "The .idata Section" and "Delay-Load Import Tables") with
the standard library only, and never runs the files, so it works on any host. It also checks the
image's processor type.

Usage: check-windows-crt.py --machine x86_64|aarch64 FILE...
       check-windows-crt.py --self-test
Exit status: 0 when every file passes, 1 when a file fails, 2 on unreadable or malformed input.
"""

import argparse
import re
import struct
import sys

MACHINES = {0x014C: "x86", 0x8664: "x86_64", 0xAA64: "aarch64"}
# Installed by the Visual C++ Redistributable (or copied next to a program), never by Windows.
VC_RUNTIME = re.compile(
    r"(vcruntime|msvcp|msvcr|concrt|vccorlib|vcomp|vcamp|mfc)\d+\w*\.dll|ucrtbased\.dll", re.I
)
# The Universal CRT, reached through the api-ms-win-crt-* API sets: linked in by +crt-static.
UCRT = re.compile(r"api-ms-win-crt-[a-z0-9-]+\.dll|ucrtbase\.dll", re.I)


class PeError(Exception):
    pass


def imported_dlls(data):
    """Returns (machine, DLL names) from the import and delay-load import tables of a PE image."""
    try:
        if data[:2] != b"MZ":
            raise PeError("not a PE image (no MZ header)")
        pe = struct.unpack_from("<I", data, 0x3C)[0]
        if data[pe : pe + 4] != b"PE\0\0":
            raise PeError("not a PE image (no PE signature)")
        machine, nsections = struct.unpack_from("<HH", data, pe + 4)
        opt_size = struct.unpack_from("<H", data, pe + 20)[0]
        opt = pe + 24
        magic = struct.unpack_from("<H", data, opt)[0]
        if magic == 0x20B:  # PE32+
            image_base = struct.unpack_from("<Q", data, opt + 24)[0]
            ndirs, dirs = struct.unpack_from("<I", data, opt + 108)[0], opt + 112
        elif magic == 0x10B:  # PE32
            image_base = struct.unpack_from("<I", data, opt + 28)[0]
            ndirs, dirs = struct.unpack_from("<I", data, opt + 92)[0], opt + 96
        else:
            raise PeError(f"unknown optional header magic {magic:#x}")
        sections = [
            struct.unpack_from("<IIII", data, opt + opt_size + 40 * i + 8) for i in range(nsections)
        ]

        def offset(rva):
            for vsize, va, raw_size, raw_ptr in sections:
                if va <= rva < va + max(vsize, raw_size) and rva - va < raw_size:
                    return raw_ptr + rva - va
            raise PeError(f"RVA {rva:#x} is outside the file's sections")

        def name_at(rva):
            start = offset(rva)
            return data[start : data.index(b"\0", start)].decode("ascii")

        def table(index, entry_size, name_of):
            if index >= ndirs:
                return []
            rva = struct.unpack_from("<I", data, dirs + 8 * index)[0]
            names, at = [], offset(rva) if rva else None
            while at is not None:
                entry = struct.unpack_from(f"<{entry_size // 4}I", data, at)
                if not any(entry):  # a zeroed entry ends the table
                    break
                names.append(name_of(entry))
                at += entry_size
            return names

        dlls = table(1, 20, lambda e: name_at(e[3]))  # IMAGE_IMPORT_DESCRIPTOR: Name RVA
        # IMAGE_DELAYLOAD_DESCRIPTOR: attribute bit 0 set means its fields are RVAs, else VAs.
        dlls += table(13, 32, lambda e: name_at(e[1] if e[0] & 1 else e[1] - image_base))
        return MACHINES.get(machine, f"{machine:#06x}"), dlls
    except (struct.error, ValueError, UnicodeDecodeError) as e:
        raise PeError(f"malformed PE image: {e}") from e


def problems(machine, dlls, expected_machine):
    """What makes an image fail the check: a wrong processor type or a C runtime import."""
    found = []
    if machine != expected_machine:
        found.append(f"processor type is {machine}, expected {expected_machine}")
    found += [f"imports {d} (Visual C++ Redistributable)" for d in dlls if VC_RUNTIME.fullmatch(d)]
    found += [f"imports {d} (Universal CRT, not linked statically)" for d in dlls if UCRT.fullmatch(d)]
    return found


def synthetic_pe(machine, imports, delayed=()):
    """A minimal PE32+ image with the given import and delay-load tables (for --self-test)."""
    names, blob = {}, bytearray()
    for dll in (*imports, *delayed):
        names[dll] = len(blob)
        blob += dll.encode() + b"\0"
    va, raw = 0x1000, 0x400
    imp = bytearray(20 * (len(imports) + 1))
    dly = bytearray(32 * (len(delayed) + 1))
    names_rva = va + len(imp) + len(dly)
    for i, dll in enumerate(imports):
        struct.pack_into("<I", imp, 20 * i + 12, names_rva + names[dll])
    for i, dll in enumerate(delayed):
        struct.pack_into("<II", dly, 32 * i, 1, names_rva + names[dll])
    section = bytes(imp + dly + blob)
    image = bytearray(raw + len(section))
    image[:2], image[0x3C:0x40] = b"MZ", struct.pack("<I", 0x40)
    struct.pack_into("<4sHHIIIHH", image, 0x40, b"PE\0\0", machine, 1, 0, 0, 0, 240, 0x22)
    opt = 0x58
    struct.pack_into("<H", image, opt, 0x20B)
    struct.pack_into("<Q", image, opt + 24, 0x140000000)
    struct.pack_into("<I", image, opt + 108, 16)
    struct.pack_into("<II", image, opt + 112 + 8, va, len(imp))
    if delayed:
        struct.pack_into("<II", image, opt + 112 + 8 * 13, va + len(imp), len(dly))
    struct.pack_into("<8sIIII", image, opt + 240, b".idata", len(section), va, len(section), raw)
    image[raw:] = section
    return bytes(image)


def self_test():
    failures = []

    def check(condition, what):
        if not condition:
            failures.append(what)

    cases = [  # (machine, imports, delay-loaded, expected machine, expected to pass)
        (0x8664, ["KERNEL32.dll", "ntdll.dll"], [], "x86_64", True),
        (0x8664, ["KERNEL32.dll", "VCRUNTIME140.dll"], [], "x86_64", False),
        (0xAA64, ["KERNEL32.dll", "vcruntime140_1.dll"], [], "aarch64", False),
        (0xAA64, ["KERNEL32.dll", "api-ms-win-crt-runtime-l1-1-0.dll"], [], "aarch64", False),
        (0xAA64, ["KERNEL32.dll", "api-ms-win-core-synch-l1-2-0.dll"], [], "aarch64", True),
        (0xAA64, ["KERNEL32.dll"], ["msvcp140.dll"], "aarch64", False),
        (0xAA64, ["KERNEL32.dll"], ["bcrypt.dll"], "aarch64", True),
        (0xAA64, ["KERNEL32.dll"], [], "x86_64", False),
    ]
    for machine, imports, delayed, expected_machine, passes in cases:
        got_machine, dlls = imported_dlls(synthetic_pe(machine, imports, delayed))
        check(got_machine == MACHINES[machine], f"processor type of {imports + delayed}")
        check(dlls == imports + delayed, f"imports read back as {dlls}")
        verdict = not problems(got_machine, dlls, expected_machine)
        check(verdict == passes, f"verdict for {dlls}")
    for bad in (b"", b"MZ" + bytes(100), b"\x7fELF" + bytes(100)):
        try:
            imported_dlls(bad)
            failures.append(f"malformed input {bad[:4]!r} was accepted")
        except PeError:
            pass
    if failures:
        for failure in failures:
            print(f"self-test failed: {failure}", file=sys.stderr)
        return 1
    print("self-test passed")
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--machine", choices=sorted(MACHINES.values()))
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("files", nargs="*")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    if not args.machine or not args.files:
        ap.error("--machine and at least one FILE are required")
    status = 0
    for path in args.files:
        try:
            with open(path, "rb") as f:
                machine, dlls = imported_dlls(f.read())
        except (OSError, PeError) as e:
            print(f"{path}: error: {e}", file=sys.stderr)
            return 2
        print(f"{path}: {machine}, imports {', '.join(dlls) or 'nothing'}")
        for problem in problems(machine, dlls, args.machine):
            print(f"{path}: error: {problem}", file=sys.stderr)
            status = 1
    return status


if __name__ == "__main__":
    sys.exit(main())
