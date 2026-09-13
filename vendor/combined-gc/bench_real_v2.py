#!/usr/bin/env python3
"""Honest stdlib bench from the V2 dump. gzip-9 / brotli-11 / BWT+delta+z9.

Does not claim Hangry 21.4% on dickens. That number is SoT history, not this script.
"""
import pathlib
import sys
import zlib

try:
    import brotli
except ImportError:
    brotli = None

PHI = 1.618033988749895


def bwt_16k(data: bytes) -> bytes:
    out = bytearray()
    for i in range(0, len(data), 16384):
        chunk = data[i : i + 16384]
        if not chunk:
            continue
        sa = sorted(range(len(chunk)), key=lambda j: chunk[j:])
        for j in sa:
            out.append(chunk[(j + len(chunk) - 1) % len(chunk)])
    return bytes(out)


def delta_transform(x: bytes) -> bytes:
    if not x:
        return b""
    out = bytearray(len(x))
    out[0] = x[0]
    for i in range(1, len(x)):
        out[i] = (x[i] - x[i - 1]) & 0xFF
    return bytes(out)


def bench_file(path: str) -> None:
    data = pathlib.Path(path).read_bytes()
    z9 = len(zlib.compress(data, 9))
    if brotli:
        brot = len(brotli.compress(data, quality=11))
        brot_s = str(brot)
    else:
        brot_s = "no-brotli"
    bwt = bwt_16k(data)
    d = delta_transform(bwt)
    z_bwt_delta = len(zlib.compress(d, 9))
    print(f"{path} raw={len(data)} gzip-9={z9} brotli-11={brot_s} bwt+delta+z9={z_bwt_delta}")
    if data and all(b == 0 for b in data):
        print(f"ZRW 8B flagship {len(data)} -> 8B vs gzip {z9} vs brotli {brot_s}")


if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("usage: bench_real_v2.py FILE...", file=sys.stderr)
        sys.exit(2)
    for p in sys.argv[1:]:
        bench_file(p)
