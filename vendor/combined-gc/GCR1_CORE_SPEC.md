# GCR1 core specification

## Purpose

GCR1 is an optional, byte-history-only high-ratio payload for Combined GC. It is designed as a self-contained terminal codec inside the existing AWAREv6 block framing. It introduces no external model, no floating point, and no future-input dependency.

## Program encoding

A GCR1 block has a `PRG1` program containing `GCR1` with an `extra` flags byte. A decoder that does not recognize the opcode rejects the block. The residual begins with:

| Bytes | Meaning |
| --- | --- |
| `GCR1` | Payload magic |
| `1` | Payload version |
| `flags` | Bit 0: exact-match expert; bit 1: secondary calibration; no other bits permitted |
| remaining | Binary range-coded bit stream |

The AWAREv6 block’s `orig_len` defines the exact number of decoded bytes. The decoder must reject truncated headers, unsupported versions, unsupported flags, invalid arithmetic input, or a nonempty coding-state failure.

## Per-bit prediction order

For every output byte position `i` and bit position from most-significant to least-significant:

1. Form the causal context tag from the last up to four completed decoded bytes, byte position modulo 16, and current partial-byte prefix.
2. Probe a fixed direct-mapped tagged frequency table. On tag mismatch, replace the entry deterministically with a uniform pseudocount state.
3. Convert the zero/one counts to `p_context_zero` in `[64, 65471]`.
4. If the match flag is set, obtain the latest candidate for the exact four-byte suffix. Confirm all four bytes directly. On confirmation, form `p_match_zero = 58982` if the predicted source bit is zero, otherwise `6553`. On failure, the match component is absent.
5. Mix `p_context_zero` and `p_match_zero` with a fixed-point adaptive match weight in `[0, 4096]`. The null profile fixes the match weight at zero. The mixer update increases the weight after a match-bit success and decreases it after a mismatch; it is bounded and causal.
6. If calibration is enabled, quantize the mixed probability into 64 bins and blend it with the bin’s bounded empirical zero/one counts. The blend starts at 25% calibrator influence and becomes stronger only after the bin has sufficient decoded history.
7. Clamp the final probability to `[64, 65471]` and code the bit through the integer binary range coder.
8. Update the context frequency table, match index, mixer, and calibration table after the bit/byte according to the decoded value.

## State bounds

| State | Bound | Determinism rule |
| --- | ---: | --- |
| Context table | 2^20 direct entries | Tag mismatch always resets that one entry. |
| Context counts | 16-bit each | At total 4096, both counts are halved with a +1 floor. |
| Match table | 2^20 positions | The latest eligible four-byte suffix replaces the prior position. |
| History | Current block only | No source position may refer forward or before byte zero. |
| Match mixer weight | 0–4096 | Integer increments/decrements only. |
| Calibration bins | 64 × two 16-bit counts | Counts halve deterministically at total 4096. |

## Profiles

| Name | Flags | Role |
| --- | ---: | --- |
| `gcr1-null` | `0x00` | Context-only null control. |
| `gcr1-match` | `0x01` | Isolates the exact-match expert. |
| `gcr1` | `0x03` | Context, exact match, mixer, and secondary calibration. |

## Invariants and test matrix

Every profile must pass empty, one-byte, random, all-zero, run-length, repeating phrase, cross-byte-pattern, malformed-header, truncated-payload, mutation, repeated-compression, and byte-for-byte decode tests. Invalid flags and missing GCR1 magic must fail before producing output.

## Initial measurement design

The initial development comparison runs the three profiles on `dickens`, `nci`, `samba`, and `xml` in forced GCR1 mode. The `gcr1` profile may only proceed to the declared validation set if it beats `gcr1-null` after its frame overhead on aggregate development bytes. Existing standard Combined GC results remain the product baseline and are not overwritten by this experiment.
