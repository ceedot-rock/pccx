# PCCX 0.2.0 — closed crate

One package. No lbr1 overlay. No pulsar path-dep. No Combined GC sibling.

```
cargo test --release
cargo build --release --bin pccx
./target/release/pccx encode IN OUT
./target/release/pccx decode IN OUT
```

PCC1 v2. Seats in this crate: ZERO, MATCH (own hash-chain LZ), STORE.
Handshake: encode only emits if decode(blob) == raw.

This is not the 51.5M PCC board. That board is the multi-crate lab stack.
This crate is the closed one-package codec.
