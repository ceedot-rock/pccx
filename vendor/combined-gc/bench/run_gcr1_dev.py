#!/usr/bin/env python3
"""Checkpointed development comparison for the three predeclared GCR1 profiles."""
from __future__ import annotations

import csv
import hashlib
import json
import os
import pathlib
import subprocess
import tempfile
import time

ROOT = pathlib.Path('/home/ubuntu/gc_pulsar_bench')
GC = ROOT / 'gc'
BIN = GC / 'target' / 'release' / 'combined_gc'
CORPUS = ROOT / 'silesia'
OUT = GC / 'bench' / 'gcr1_dev.jsonl'
CSV = GC / 'bench' / 'gcr1_dev.csv'
SLICE_BYTES = 262_144
FILES = ['dickens', 'nci', 'samba', 'xml']
PROFILES = ['gcr1-null', 'gcr1-match', 'gcr1']
FIELDS = ['file', 'slice_bytes', 'profile', 'archive_bytes', 'bits_per_byte', 'compress_seconds', 'decompress_seconds', 'verified', 'input_sha256', 'output_sha256']


def sha256(path: pathlib.Path) -> str:
    h = hashlib.sha256()
    with path.open('rb') as f:
        for block in iter(lambda: f.read(1024 * 1024), b''):
            h.update(block)
    return h.hexdigest()


def to_csv() -> None:
    rows = [json.loads(x) for x in OUT.read_text().splitlines() if x]
    with CSV.open('w', newline='') as f:
        w = csv.DictWriter(f, fieldnames=FIELDS)
        w.writeheader(); w.writerows(rows)


def main() -> None:
    if not BIN.is_file(): raise SystemExit('missing release binary')
    OUT.unlink(missing_ok=True); CSV.unlink(missing_ok=True)
    start = time.monotonic()
    with tempfile.TemporaryDirectory(prefix='gcr1_dev_', dir='/tmp') as temp:
        temp = pathlib.Path(temp)
        for name in FILES:
            source = CORPUS / name
            if not source.is_file(): raise SystemExit(f'missing corpus file: {source}')
            sample = temp / f'{name}.slice'
            sample.write_bytes(source.read_bytes()[:SLICE_BYTES])
            expected = sha256(sample)
            for profile in PROFILES:
                archive, restored = temp / f'{name}.{profile}.cgc', temp / f'{name}.{profile}.out'
                t0 = time.monotonic()
                r = subprocess.run([str(BIN), '--input', str(sample), '--output', str(archive), '--profile', profile, '--block', '256K'], text=True, capture_output=True)
                csec = time.monotonic() - t0
                if r.returncode: raise RuntimeError(r.stderr[-1000:])
                t0 = time.monotonic()
                r = subprocess.run([str(BIN), '--decompress', '--input', str(archive), '--output', str(restored)], text=True, capture_output=True)
                dsec = time.monotonic() - t0
                if r.returncode: raise RuntimeError(r.stderr[-1000:])
                observed = sha256(restored)
                verified = expected == observed and subprocess.run(['cmp', '-s', str(sample), str(restored)]).returncode == 0
                if not verified: raise RuntimeError(f'verification failed: {name}/{profile}')
                row = {'file': name, 'slice_bytes': sample.stat().st_size, 'profile': profile, 'archive_bytes': archive.stat().st_size, 'bits_per_byte': archive.stat().st_size * 8 / sample.stat().st_size, 'compress_seconds': csec, 'decompress_seconds': dsec, 'verified': True, 'input_sha256': expected, 'output_sha256': observed}
                with OUT.open('a') as f:
                    f.write(json.dumps(row, sort_keys=True) + '\n'); f.flush(); os.fsync(f.fileno())
                to_csv()
                print(f'{name:8} {profile:11} {row["archive_bytes"]:8} B {row["bits_per_byte"]:.4f} bpb verified', flush=True)
    print(json.dumps({'status': 'complete', 'elapsed_seconds': time.monotonic() - start, 'rows': len(FILES) * len(PROFILES)}, indent=2))

if __name__ == '__main__': main()
