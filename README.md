# PCCX 0.3.0

Closed crate. PCC1. In-tree engines:

- `vendor/pulsar` — BW23
- `vendor/combined-gc` — AWARE own-path

```
./vendor.sh
cargo test --release --lib
cargo build --release --bin pccx
./target/release/pccx encode IN OUT
```

Encode never emits a blob ≥ raw. Seats: ZERO, MATCH (own LZ), BWT, AWARE. Host gzip/xz/bz dropped.

256 KiB vs gzip-9 (DECODE_OK): dickens 81305 / 98593 · ooffice 139122 / 142495 · reymont 56555 / 72353 · osdb 88199 / 98799 · mr 59099 / 79021.
