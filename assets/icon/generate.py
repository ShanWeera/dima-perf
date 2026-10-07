#!/usr/bin/env python3
"""Renders the DiMA application icon (dima.svg) to PNG, ICO and ICNS.

Usage:  python3 assets/icon/generate.py

Standard library only (no Cairo, Inkscape or ImageMagick), so the raster files can be
regenerated on any machine and are byte-for-byte reproducible. The geometry below is the same
as in dima.svg; when the SVG changes, change GEOMETRY here and re-run the script. Every shape
is anti-aliased by 8x8 supersampling.

Outputs (next to this script):
  dima-{16,32,48,64,128,256,512,1024}.png   window icon, Linux packages, docs
  dima.ico                                   Windows executable and installer (16-256 px, PNG entries)
  dima.icns                                  macOS bundle (16-1024 px, PNG entries)
"""

import math
import os
import struct
import zlib

HERE = os.path.dirname(os.path.abspath(__file__))
VIEW = 1024.0  # SVG viewBox size

# Shapes in viewBox coordinates (see dima.svg).
BACKGROUND = (0x00, 0x72, 0xB2)  # Okabe-Ito blue (Wong 2011)
TRACE = (0xFF, 0xFF, 0xFF)
BASELINE = (0xFF, 0xFF, 0xFF, 0.55)
PEAK = (0xD5, 0x5E, 0x00)  # Okabe-Ito vermillion
RECT = (64.0, 64.0, 960.0, 960.0, 200.0)  # x0, y0, x1, y1, corner radius
BASELINE_Y, BASELINE_X0, BASELINE_X1, BASELINE_W = 792.0, 192.0, 832.0, 28.0
POLYLINE = [
    (192, 704), (296, 704), (360, 440), (424, 640), (488, 560),
    (560, 296), (632, 664), (704, 600), (768, 704), (832, 704),
]
TRACE_W = 52.0
TRACE_BOX = (
    min(p[0] for p in POLYLINE),
    min(p[1] for p in POLYLINE),
    max(p[0] for p in POLYLINE),
    max(p[1] for p in POLYLINE),
)
PEAK_CENTRE, PEAK_R, PEAK_RING = (560.0, 296.0), 58.0, 22.0


def inside_rounded_rect(x, y):
    x0, y0, x1, y1, r = RECT
    if x < x0 or x > x1 or y < y0 or y > y1:
        return False
    cx = min(max(x, x0 + r), x1 - r)
    cy = min(max(y, y0 + r), y1 - r)
    return (x - cx) ** 2 + (y - cy) ** 2 <= r * r


def dist_to_segment(px, py, ax, ay, bx, by):
    dx, dy = bx - ax, by - ay
    t = ((px - ax) * dx + (py - ay) * dy) / (dx * dx + dy * dy)
    t = min(max(t, 0.0), 1.0)
    return math.hypot(px - (ax + t * dx), py - (ay + t * dy))


def colour_at(x, y):
    """RGBA (0-255, alpha 0-1) of the icon at a viewBox point; None outside."""
    if not inside_rounded_rect(x, y):
        return None
    r, g, b = BACKGROUND
    a = 1.0

    def over(c, alpha):
        nonlocal r, g, b
        r = c[0] * alpha + r * (1 - alpha)
        g = c[1] * alpha + g * (1 - alpha)
        b = c[2] * alpha + b * (1 - alpha)

    if abs(y - BASELINE_Y) <= BASELINE_W / 2 and BASELINE_X0 <= x <= BASELINE_X1:
        over(BASELINE[:3], BASELINE[3])
    half = TRACE_W / 2
    if TRACE_BOX[0] - half <= x <= TRACE_BOX[2] + half and TRACE_BOX[1] - half <= y <= TRACE_BOX[3] + half:
        d = min(
            dist_to_segment(x, y, *POLYLINE[i], *POLYLINE[i + 1])
            for i in range(len(POLYLINE) - 1)
        )
        if d <= half:
            over(TRACE, 1.0)
    dp = math.hypot(x - PEAK_CENTRE[0], y - PEAK_CENTRE[1])
    if dp <= PEAK_R + PEAK_RING / 2:
        over(TRACE, 1.0)
    if dp <= PEAK_R - PEAK_RING / 2:
        over(PEAK, 1.0)
    return (r, g, b, a)


def render(size, ss=8):
    """RGBA bytes of the icon at size x size pixels.

    Small sizes are supersampled everywhere; from 128 px, pixels whose corners and centre agree
    are taken as uniform and only the others are supersampled (the result differs from full
    supersampling by less than one 8-bit step on edge pixels only).
    """
    out = bytearray()
    scale = VIEW / size
    adaptive = size >= 128

    def sample(x, y):
        return colour_at(x * scale, y * scale)

    for py in range(size):
        for px in range(size):
            if adaptive:
                probes = [
                    sample(px + 0.001, py + 0.001),
                    sample(px + 0.999, py + 0.001),
                    sample(px + 0.001, py + 0.999),
                    sample(px + 0.999, py + 0.999),
                    sample(px + 0.5, py + 0.5),
                ]
                if all(p == probes[0] for p in probes):
                    c = probes[0]
                    if c is None:
                        out += b"\x00\x00\x00\x00"
                    else:
                        out += bytes([round(c[0]), round(c[1]), round(c[2]), round(c[3] * 255)])
                    continue
            acc = [0.0, 0.0, 0.0, 0.0]
            for sy in range(ss):
                for sx in range(ss):
                    c = sample(px + (sx + 0.5) / ss, py + (sy + 0.5) / ss)
                    if c is not None:
                        acc[0] += c[0] * c[3]
                        acc[1] += c[1] * c[3]
                        acc[2] += c[2] * c[3]
                        acc[3] += c[3]
            alpha = acc[3] / (ss * ss)
            rgb = [round(acc[i] / acc[3]) for i in range(3)] if acc[3] > 0 else [0, 0, 0]
            out += bytes([min(255, v) for v in rgb] + [round(alpha * 255)])
    return bytes(out)


def png(size, rgba):
    def chunk(kind, data):
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))

    raw = b"".join(
        b"\x00" + rgba[y * size * 4:(y + 1) * size * 4] for y in range(size)
    )
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )


def ico(images):
    """Windows icon with PNG-compressed entries (supported since Windows Vista)."""
    header = struct.pack("<HHH", 0, 1, len(images))
    offset = 6 + 16 * len(images)
    entries, blobs = b"", b""
    for size, data in images:
        dim = 0 if size >= 256 else size
        entries += struct.pack("<BBBBHHII", dim, dim, 0, 0, 1, 32, len(data), offset)
        blobs += data
        offset += len(data)
    return header + entries + blobs


def icns(entries):
    """Apple icon image with PNG entries (OSType, PNG bytes)."""
    body = b"".join(kind + struct.pack(">I", len(data) + 8) + data for kind, data in entries)
    return b"icns" + struct.pack(">I", len(body) + 8) + body


def main():
    sizes = [16, 32, 48, 64, 128, 256, 512, 1024]
    pngs = {}
    for s in sizes:
        pngs[s] = png(s, render(s))
        with open(os.path.join(HERE, f"dima-{s}.png"), "wb") as f:
            f.write(pngs[s])
    with open(os.path.join(HERE, "dima.ico"), "wb") as f:
        f.write(ico([(s, pngs[s]) for s in (16, 32, 48, 64, 128, 256)]))
    with open(os.path.join(HERE, "dima.icns"), "wb") as f:
        f.write(
            icns(
                [
                    (b"icp4", pngs[16]),
                    (b"icp5", pngs[32]),
                    (b"icp6", pngs[64]),
                    (b"ic07", pngs[128]),
                    (b"ic08", pngs[256]),
                    (b"ic09", pngs[512]),
                    (b"ic10", pngs[1024]),
                    (b"ic11", pngs[32]),
                    (b"ic12", pngs[64]),
                    (b"ic13", pngs[256]),
                    (b"ic14", pngs[512]),
                ]
            )
        )


if __name__ == "__main__":
    main()
