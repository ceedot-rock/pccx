#!/usr/bin/env python3
"""Held-out GCR1 validation: fixed 128 KiB prefixes, verified after every row."""
from __future__ import annotations
import csv, hashlib, json, pathlib, subprocess, tempfile, time

ROOT = pathlib.Path('/home/ubuntu/gc_pulsar_bench')
BIN = ROOT / 'gc' / 'target' / 'release' / 'combined_gc'
CORPUS = ROOT / 'silesia'
OUT = ROOT / 'gc' / 'bench' / 'gcr1_validation.csv'
FILES = ['mozilla', 'ooffice', 'osdb', 'sao', 'mr', 'x-ray', 'reymont', 'webster']
PROFILES = ['gcr1-null', 'gcr1']
SLICE = 131_072


def digest(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> None:
    rows = []
    t0 = time.monotonic()
    with tempfile.TemporaryDirectory(prefix='gcr1_validation_') as directory:
        d = pathlib.Path(directory)
        for name in FILES:
            src = CORPUS / name
            sample = d / f'{name}.slice'
            sample.write_bytes(src.read_bytes()[:SLICE])
            expected = digest(sample)
            for profile in PROFILES:
                arc, out = d / f'{name}.{profile}.cgc', d / f'{name}.{profile}.out'
                a = time.monotonic()
                r = subprocess.run([str(BIN), '--input', str(sample), '--output', str(arc), '--profile', profile, '--block', '128K'], stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
                enc = time.monotonic() - a
                if r.returncode: raise RuntimeError(r.stderr)
                a = time.monotonic()
                r = subprocess.run([str(BIN), '--decompress', '--input', str(arc), '--output', str(out)], stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
                dec = time.monotonic() - a
                if r.returncode or digest(out) != expected or subprocess.run(['cmp', '-s', str(sample), str(out)]).returncode:
                    raise RuntimeError(f'verification failure: {name}/{profile}')
                row = {'file': name, 'sample_bytes': sample.stat().st_size, 'profile': profile, 'archive_bytes': arc.stat().st_size, 'bits_per_byte': 8 * arc.stat().st_size / sample.stat().st_size, 'compress_seconds': enc, 'decompress_seconds': dec, 'verified': True, 'sha256': expected}
                rows.append(row)
                with OUT.open('w', newline='') as f:
                    writer = csv.DictWriter(f, fieldnames=row.keys()); writer.writeheader(); writer.writerows(rows)
                print(f'{name:8} {profile:9} {row["archive_bytes"]:7} {row["bits_per_byte"]:.4f} verified', flush=True)
    print(json.dumps({'rows': len(rows), 'elapsed_seconds': time.monotonic() - t0}, indent=2))

if __name__ == '__main__': main()
