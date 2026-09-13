# GCR1 Core — Development Result

The first high-ratio core experiment completed on the predeclared 4 × 256 KiB Silesia development slices. Every one of the 12 archives decoded byte-for-byte and produced a matching SHA-256 digest.

| Profile | Archive bytes | Byte-weighted bpb | Encode time | Decode time | Change vs. GCR1-null |
| --- | ---: | ---: | ---: | ---: | ---: |
| GCR1-null (contexts only) | 570,295 | 4.351006 | 0.962 s | 0.945 s | — |
| GCR1-match | 527,694 | 4.025986 | 1.164 s | 1.137 s | −42,601 bytes (−7.470%) |
| **GCR1 (match + fixed-point calibration)** | **506,336** | **3.863037** | **1.278 s** | **1.262 s** | **−63,959 bytes (−11.215%)** |

The exact-match expert improved the context-only null profile on all four development files. Calibration added a further 21,358-byte aggregate gain beyond the match-only profile. The combined GCR1 profile won on `dickens`, `samba`, and `xml`.

However, `nci` is a counterexample: its context-only `gcr1-null` profile reached 62,187 bytes (1.897797 bpb), while full GCR1 used 71,944 bytes (2.195557 bpb). The match and calibration state is therefore **not** made the global default. The next work package must add typed SDF record/field transforms and compare them with the GCR1 null and full profiles under a causal router.

The result validates the shared core as a useful experimental engine, but it remains an early 64 MiB-tier model. It does not yet establish a full-file, held-out, or product-level improvement over current Combined GC.
