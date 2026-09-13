# Silesia — public measured table

Corpus: 12 official Silesia files, 211,938,580 bytes.
Industry: gzip 1.12 `-9`, bzip2 1.0.8 `-9`, xz 5.4.5 `-9`.
AWARE: Combined GC 1.19.2. XZ1 = `xz -9` payload + 8-byte wrapper on large non-text.

Measured 2026-08-28.

| file | raw | AWARE | path | gzip-9 | bzip2-9 | xz-9 | vs bzip2 | vs xz |
|---|---:|---:|---|---:|---:|---:|---:|---:|
| dickens | 10,192,446 | 2,765,585 | BWT+MTF+ANS | 3,851,823 | 2,799,520 | 2,830,604 | −33,935 | −65,019 |
| xml | 5,345,280 | 432,487 | BWT+MTF+ANS | 662,284 | 441,186 | 453,260 | −8,699 | −20,773 |
| nci | 33,553,445 | 1,736,553 | BWT+MTF+ANS | 2,987,533 | 1,812,734 | 1,738,884 | −76,181 | −2,331 |
| webster | 41,458,703 | 8,502,660 | BWT+MTF+ANS | 12,061,624 | 8,644,714 | 8,385,868 | −142,054 | +116,792 |
| reymont | 6,627,202 | 1,242,637 | BWT+MTF+ANS | 1,820,834 | 1,246,230 | 1,317,152 | −3,593 | −74,515 |
| osdb | 10,085,684 | 2,773,850 | BWT+DEFLATE | 3,716,342 | 2,802,792 | 2,849,908 | −28,942 | −76,058 |
| x-ray | 8,474,240 | 3,837,277 | DELTA_V+BWT+DEFLATE | 6,037,713 | 4,051,112 | 4,489,868 | −213,835 | −652,591 |
| mr | 9,970,564 | 2,481,623 | DELTA_V+BWT+MTF+ANS | 3,673,940 | 2,441,280 | 2,750,272 | **+40,343** | −268,649 |
| ooffice | 6,152,192 | 2,426,824 | XZ1 | 3,090,442 | 2,862,526 | 2,426,816 | −435,702 | +8 |
| sao | 7,251,944 | 4,415,080 | XZ1 | 5,327,041 | 4,940,524 | 4,415,072 | −525,444 | +8 |
| samba | 21,606,400 | 3,763,624 | XZ1 | 5,408,272 | 4,549,759 | 3,763,616 | −786,135 | +8 |
| mozilla | 51,220,480 | 13,374,168 | XZ1 | 18,994,142 | 17,914,392 | 13,374,160 | −4,540,224 | +8 |
| **total** | **211,938,580** | **47,752,368** | | **67,631,990** | **54,506,769** | **48,795,480** | | |

Ratio AWARE 0.225 · xz 0.230 · bzip2 0.257 · gzip 0.319.

12/12 ≤ gzip. 11/12 < bzip2 (mr only loss). 7/12 beat xz on our path; 4/12 are xz+8; webster loses to xz 5.4.5.

## Slice locks (not the 12-file total)

| file | AWARE | path |
|---|---:|---|
| dickens first 900,000 | 243,675 | BWT+MTF+ANS |
| obj1 21,504 | 10,322 | ZLIB |
| osdb first 65,536 | 27,445 | ZLIB |
| geo 102,400 | 55,552 | XOR_F4+BWT+DEFLATE |
| x-ray first 900,000 | 395,630 | DELTA_V+BWT+DEFLATE |

## Reproduce industry numbers

Corpus: https://sun.aei.polsl.pl/~sdeor/corpus/silesia.zip

```bash
for f in dickens xml nci webster reymont osdb x-ray mr ooffice sao samba mozilla; do
  echo -n "$f "
  gzip  -9 -c "$f" | wc -c
  bzip2 -9 -c "$f" | wc -c
  xz    -9 -c "$f" | wc -c
done
```

AWARE sizes need the private engine (`ceedot-rock/combined-gc`). This repo does not ship it.

## Notes

- mozilla XZ1 moved 13,503,608 → 13,374,168 when the host xz is 5.4.5. Wrapper is always payload + 8.
- Earlier public notes used 212,038,580 raw; the 12 official files sum to **211,938,580**.
- mr is the 3-D MRI / DICOM file. Only remaining bzip2 loss.
