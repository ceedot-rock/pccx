# PCCX 0.3.0

**License:** AGPL-3.0-or-later **or** a written commercial grant ([LICENSE](LICENSE), [COMMERCIAL.md](COMMERCIAL.md)). Visibility is not a grant to ship Combined GC inside a closed product.

Lab verb: **squeeze**. Agent POST: `https://spl-lab-agent.fly.dev/v1/squeeze`

Closed crate. PCC1. In-tree engines:

- `vendor/pulsar` — BW23 (GPL-3)
- `vendor/combined-gc` — AWARE own-path (lab dual / commercial)

```
cargo test --release --lib
cargo build --release --bin pccx
./target/release/pccx encode IN OUT
```

Encode never emits a blob ≥ raw. Seats: ZERO, MATCH, BWT, AWARE. Host gzip/xz/bz dropped.

256 KiB vs gzip-9 (DECODE_OK): dickens 81305 / 98593 · ooffice 139122 / 142495 · reymont 56555 / 72353 · osdb 88199 / 98799 · mr 59099 / 79021.

Not an OSCB table row until a tagged rebuild matches the board.
