# combined-gc 1.19.2 freeze

SlidPhiLabs Combined GC. Private. UNLICENSED. `publish = false`.

```
cargo test --lib
cargo test --lib --features experimental
cargo run --release -- --input FILE --block 900K --bench
cargo run --release -- --input silesia/dickens --block 2M --bench
```

Locks: 243,675 / 2,765,585 / 10,322 / 27,445 / 55,552 / 395,630.

Default `pack_size()` caps at 2M. match_maker* only with `--features experimental`.
See STATUS_2026-08-26.md.
