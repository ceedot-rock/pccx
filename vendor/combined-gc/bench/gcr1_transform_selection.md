# GCR1 transform selection after first prototypes

## Retained

The GCR1 context-plus-exact-match-plus-calibration core is retained as an opt-in research profile. On the declared 1 MiB development set it used 506,336 bytes, a 63,959-byte (11.215%) gain over its same-memory context-only null control. All profiles passed byte equality and SHA-256 verification.

## Rejected and quarantined

| Prototype | Evidence | Decision |
| --- | --- | --- |
| Full SDF record-position context | `nci` 256 KiB: 116,573 bytes versus 62,195 for the GCR1-null control after restricting position to 32 buckets. | Reject. Coarse record offset fragments contexts without identifying a semantic field. |
| POSIX tar header grouping | Full `samba`: 8,898,176 bytes versus 8,896,101 for plain GCR1; 2,075 bytes (0.023%) worse. | Reject. Grouping headers alone does not create enough useful redundancy for the current byte model. |

The rejected prototypes remain source-controlled as experiments but are not promoted to an automatic profile or product claim.

## Selected next path: bounded SDF field identity

The next typed experiment is a deliberately narrower SDF state feature. It does not parse or reorder records. It observes only decoded history: the beginning of a `>` property-name line; a deterministic 8-bit rolling hash of the property name; and the subsequent property-value lines until the next property name or `$$$$` record terminator. This small field identity becomes an additional context key only in an explicit `gcr1-sdf-field` profile.

The expected advantage is that recurring SDF property labels can choose different literal contexts for their own values without creating a unique state per absolute record position. The control is GCR1-null at the same table and memory size. The feature must improve the predeclared `nci` development slice after its header cost and then pass a fresh record-aligned validation slice before it is retained.
