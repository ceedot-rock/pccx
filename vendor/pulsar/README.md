# pulsar

Own-only lossless compressor. **Core-Lite.** Public source for official benches.

Public name is **pulsar**. The GitHub repo is still `ceedot-rock/pulsar-best`.

Copyright (c) 2026 Corey Tasz / Slid Phi Labs.

**Silesia (12 files, individually):** **55,745,438** bytes of 211,938,580 (0.2630).  
DECODE_OK 12/12. Beats gzip-9 12/12. Loses to bzip2-9 (+1.24M) and xz-6 (+6.3M).

Not a rank claim against paq8px, cmix, or zpaq. Combined GC and AWARE are **not in this repo**.

## What this is

Compressor A (demonstrator). BW22 / OZL2 / PZ22 picker. No taught residual core.

## What this is not

- Not Combined GC
- Not AWARE 1.19.2
- Not the Drive-lock encoder (dickens 243,675 is a reference constant, not this output)
- Not Autonoma / Blackjack production

This tree is **GPL-3.0-or-later** from 2.4.0 (MIT through 2.3.1). Closed-source embedding: [COMMERCIAL.md](COMMERCIAL.md). Combined GC / AWARE stay proprietary. Commercial inquiries for the private engines: Slid Phi Labs / Corey Tasz. SoT: https://www.slidphilabs.com/licensing.json

## Official packet

See [OFFICIAL/](OFFICIAL/). 2.5.0 (55,745,438) was emailed to Matt Mahoney for OSCB on 2026-09-02. Do not send another line until a live total beats that. Table name is **pulsar**, not pulsar-best.

Last full Silesia (2.5.0): **55,745,438** / 211,938,580. gzip-9 67,631,990. bzip2-9 54,506,769. xz-6 ~49.4M.

Industry calibration: gzip -9 = 67,631,990; bzip2 = 54,506,769.

## Build

```
cargo test --lib
cargo run --release --bin pulsar -- encode IN -o OUT
cargo run --release --bin pulsar -- decode OUT -o BACK
```

## License

**2.4.0+ : GPL-3.0-or-later.**  
2.3.1 and earlier snapshots remain MIT. See `LICENSE` and `LICENSE-CHANGE.md`.
