# MIX LIVE — 2026-08-27

The two are mixed. One encoder. One decoder. One frame.

## Router

`cheap_stats` → `GatePath` → candidate list → encode each → **decode must equal raw** → keep smallest.

| Path | Candidates |
|---|---|
| all-zero / StructuredInts | ZRW, Cont-1088, Hangry4, Zlib, Store |
| Text | Hangry4, Hangry5, Loomin, Cont-1088, Zlib, Store |
| Delta | Cont-1088, Hangry4, Zlib, Store |
| Floats | Cont-1088, Zlib, Store |
| Random | Zlib, Store |
| Binary | Loomin, Cont-1088, Hangry4, Zlib, Store |

ZRW is skipped unless the block is all zeros.

## Codecs that actually invert

- **ZRW** — 8-byte `ZRW\0` + u32 count. All-zero only.
- **Zlib** — flate2 rust_backend, Compression::best.
- **Hangry O4/O5** — Taylor reciprocal predictor, wrapping residual, zlib payload.
- **Cont-1088** — order-1 wrapping delta, 17 magnitude bins standing in for 1088-plane occupancy, zlib per occupied bin + bin-id stream.
- **Loomin** — 8×8 seed grid, 4-neighbor average predictor, residual zlib, cell update. Invertible walk.

## Measured this box (2026-08-27)

| file | raw | cgc2 | codec | zlib-9 | RT |
|---|---:|---:|---|---:|---|
| zeros | 10000 | 22 | Zrw | 33 | PASS |
| text (repeat phrase) | 104000 | 317 | Zlib | 303 | PASS |
| ramp 8k | 8000 | 356 | Zlib | 352 | PASS |
| walk 8k | 8000 | 1904 | Zlib | 1894 | PASS |
| rand 8k | 8000 | 352 | Zlib | 347 | PASS |

14-byte CGC2 header is why zlib-inside-frame is slightly larger than raw zlib-9. Bake-off still picks zlib on those files because Hangry/Cont/Loomin payloads were bigger.

8 cargo tests PASS (zeros uses ZRW; Hangry/Cont/Loomin alone invert; mixed identities hold).

## Honest remainder

Mixing them together does **not** invent match coding. Dickens-class general text still belongs to BWT+MTF+ANS / brotli / zlib, not to Hangry or ZRW. Historical 1.19.2 freeze numbers stay the last full-corpus stamp. This 1.20.0-mix ships the shared engine so the two SKUs stop being separate codepaths.
