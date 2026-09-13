# GCR1 held-out selection record

The predeclared held-out test completed on eight 128 KiB prefixes from real Silesia files. Sixteen archives decoded byte-for-byte and all SHA-256 checks passed.

| Profile | Input bytes | Archive bytes | Byte-weighted bpb | Encode time | Decode time |
| --- | ---: | ---: | ---: | ---: | ---: |
| GCR1-null | 1,048,576 | 750,453 | 5.725502 | 1.219 s | 1.214 s |
| **GCR1 (match + calibration)** | 1,048,576 | **662,625** | **5.055428** | 1.578 s | 1.520 s |

Full GCR1 saved **87,828 bytes (11.703%)** against its capacity-matched null control. It won on `ooffice`, `osdb`, `sao`, `x-ray`, `reymont`, and `webster`. It regressed on `mozilla` by 1,694 bytes and `mr` by 2,522 bytes.

## Selection

**Retain GCR1 as an opt-in experimental generic-model profile.** The causal exact-match plus fixed-point calibration mechanism generalized against its declared null control in aggregate, with a measured 1.30× encode-time and 1.25× decode-time cost on this slice suite.

**Do not make GCR1 the product default.** The test does not compare against current full Combined GC routing or its XZ1 fallback. It also contains two real held-out regressions. The SDF record-position, SDF-field, and tar-header grouping prototypes remain quarantined after their measured regressions.

## Next evidence gate

Before a default-path decision, compare GCR1 and standard Combined GC on complete `ooffice`, `osdb`, and `reymont` files, then investigate a conservative executable/record transform only where the generic core survives that comparison. No 1.0-bpb claim follows from these prefix measurements.
