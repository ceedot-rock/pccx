# PCCX 0.1.0

Lab house picker. One face: `lb pcc` writes **PCC1**.

Seats inside that frame:
- pulsar BWT
- LBR1 MATCH (champ pack included in try_match)
- Combined GC own-path as `Op::Aware` (host gzip/xz/bz dropped)
- CMAQ / LZ only when the winner is still weak

## Routing

- **Text** (Autonoma primary = BWT): BWT and GC both bid. MATCH only if ratio still > 0.35.
- **Binary**: MATCH (+ delta). GC/BWT only if weak and file ≤ 2 MiB.

Build on top of `ceedot-rock/lbr1` + `pulsar-best` + Combined GC sibling. Overlay `overlay/pcc.rs` onto `splb/src/pcc.rs`. Default feature `aware`.

## Measured this freeze (DECODE_OK)

| file | raw | PCCX | op |
|---|---:|---:|---|
| reymont | 6,627,202 | 1,242,663 | aware |
| ooffice | 6,152,192 | 2,665,306 | match |

ooffice official pcc-0.12.1 was 2,670,536 (−5,230 on this binary).

Not a 12-file total. mozilla not run on this tag.

## Version

0.1.0
