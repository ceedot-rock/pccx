# Combined GC High-Ratio Core Charter

## Objective

Build an optional, versioned **GCR1 high-ratio payload** for Combined GC. Its first purpose is to replace simple fallback behavior on selected structured streams with a deterministic context, exact-match, fixed-point mixing, and calibration stack. The objective is lower archive bytes, not a premature claim of 1.0 bits per byte.

## Scope of the first experiment

The first increment is deliberately narrow. It contains a fixed-size byte-context table, a verified exact-match expert, a fixed-point two-expert mixer, and an optional secondary probability calibrator. It does **not** include tar parsing, executable transforms, DICOM parsing, schema-aware XML tokenization, external dictionaries, floating-point inference, or any future-input preprocessing.

## Archive and decoder invariants

| Requirement | Frozen rule |
| --- | --- |
| Profile identity | The GCR1 profile and every coding-path parameter are encoded in the frame header. |
| Causality | Prediction consumes only frame configuration and bytes already encoded or decoded. |
| Match validity | A hash candidate is used only after exact seed-byte confirmation; the expert clears on the first mismatch. |
| Determinism | Integer arithmetic, fixed table sizes, fixed replacement order, fixed tie breaking, and fixed probability clamps only. |
| Compatibility | Existing Combined GC frames continue to decode unchanged; unknown GCR1 parameters fail closed. |
| Verification | Each test and benchmark decodes to byte equality and matching SHA-256. |

## Resource tier

The development implementation is capped at **64 MiB** incremental model memory: 32 MiB tagged context state, 24 MiB match/history state, and no more than 8 MiB for mixer and calibration state. The initial target is a ratio experiment, not a low-memory replacement for the current fast paths.

## Dataset split

The declared development set is `dickens`, `nci`, `samba`, and `xml` from Silesia. The declared untouched validation set is `mozilla`, `ooffice`, `osdb`, `sao`, `mr`, `x-ray`, `reymont`, and `webster`. Selection constants may not be changed after validation has been viewed. Any new design motivated by validation becomes exploratory and requires a new held-out split.

## Selection gates

A candidate must pass all existing unit, corruption, truncation, determinism, and byte-round-trip tests before measurement. It is retained only if it reduces aggregate archive bytes on the declared validation set relative to a capacity-matched GCR1-null control and does not exceed the stated memory tier. A gain below 0.25% on the declared regime is not a default-path justification; it remains experimental or is removed.

## Non-goals

This increment cannot establish a world-leading compressor, guarantee 1.0 bpb, or replace domain-specific work. It is the common inference engine needed before adding validated tar, executable, record, image, and markup transforms.
