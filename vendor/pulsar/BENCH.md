# pulsar 2.5.0 — real Silesia + Calgary

Public name: **pulsar** (repo still `ceedot-rock/pulsar-best`).

Measured 2026-09-01 on this machine. Official files. Lossless (encode/decode byte match on every file). Engine: BW23 = RLE-1 + BWT + MTF + Wheeler + 4-ctx rANS. OZL2 skipped on files ≥256 KiB. Adaptive 2 MiB blocks on files ≥8 MiB.

Target was xz-6 ~49.4M on Silesia. **Not reached.**

## Silesia totals (2.5.0)

| codec | bytes | ratio |
|---|---:|---:|
| raw | 211,938,580 | 1.0000 |
| gzip -9 | 67,631,918 | 0.3191 |
| **pulsar 2.5.0** | **55,745,438** | **0.2630** |
| bzip2 -9 | 54,506,769 | 0.2572 |
| zstd -19 | 52,895,306 | 0.2496 |
| brotli -q 11 | 49,564,563 | 0.2339 |
| xz -6 | 49,408,952 | 0.2331 |

vs 2.3.0 (56,654,942): **−909,504**. vs gzip: **0.8242**. vs bzip2: **1.0227** (+1.24M). vs xz-6: **1.1282** (+6.34M). vs brotli-11: **1.1247**.

Beats gzip-9 on **12/12**. Beats bzip2-9 on **nci** only. Beats xz-6 on **mr, nci, osdb, x-ray**. The ~6.3M xz gap is mostly **mozilla** (18,110,513 vs xz 13,577,880 = +4.53M).

## Silesia per file (2.5.0)

| file | raw | pulsar | gzip-9 | bzip2-9 | xz-6 | br-11 | zstd-19 | s | ok |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---|
| xml | 5,345,280 | 478,360 | 662,280 | 441,186 | 453,268 | 430,566 | 453,206 | 21.7 | OK |
| ooffice | 6,152,192 | 2,950,568 | 3,090,434 | 2,862,526 | 2,426,824 | 2,478,855 | 2,595,374 | 20.8 | OK |
| reymont | 6,627,202 | 1,331,009 | 1,820,826 | 1,246,230 | 1,317,160 | 1,332,158 | 1,348,459 | 33.5 | OK |
| sao | 7,251,944 | 5,150,474 | 5,327,037 | 4,940,524 | 4,415,080 | 4,586,092 | 5,000,573 | 35.2 | OK |
| x-ray | 8,474,240 | 4,307,052 | 6,037,707 | 4,051,112 | 4,489,920 | 4,682,753 | 5,129,823 | 22.3 | OK |
| mr | 9,970,564 | 2,596,170 | 3,673,937 | 2,441,280 | 2,750,236 | 2,823,136 | 3,106,733 | 23.1 | OK |
| osdb | 10,085,684 | 2,844,929 | 3,716,337 | 2,802,792 | 2,850,112 | 2,816,278 | 3,100,173 | 72.8 | OK |
| dickens | 10,192,446 | 2,834,360 | 3,851,815 | 2,799,520 | 2,831,684 | 2,827,777 | 2,849,941 | 71.0 | OK |
| samba | 21,606,400 | 4,699,263 | 5,408,266 | 4,549,759 | 3,787,408 | 3,766,340 | 3,894,671 | 169.2 | OK |
| nci | 33,553,445 | 1,764,304 | 2,987,529 | 1,812,734 | 1,797,072 | 1,519,768 | 1,664,984 | 367.4 | OK |
| webster | 41,458,703 | 8,678,436 | 12,061,616 | 8,644,714 | 8,712,308 | 8,428,575 | 8,679,701 | 443.6 | OK |
| mozilla | 51,220,480 | 18,110,513 | 18,994,134 | 17,914,392 | 13,577,880 | 13,872,265 | 15,071,668 | 430.9 | OK |
| **total** | **211,938,580** | **55,745,438** | **67,631,918** | **54,506,769** | **49,408,952** | **49,564,563** | **52,895,306** | | |

## Calgary large 18 (2.5.0)

| codec | bytes | ratio |
|---|---:|---:|
| raw | 3,251,493 | 1.0000 |
| gzip -9 | 1,059,335 | 0.3258 |
| **pulsar 2.5.0** | **924,157** | **0.2842** |
| zstd -19 | 920,966 | 0.2832 |
| xz -6 | 885,860 | 0.2724 |
| bzip2 -9 | 866,501 | 0.2665 |
| brotli -q 11 | 856,672 | 0.2635 |

vs 2.3.0 Calgary (~916.8k): **worse** (+~7k). Beats gzip-9 on the set total. Loses to bzip2/xz/brotli. Tiny files (obj1, paper4–6, progc/l/p, trans) still lose gzip.

---

# PULSAR-BEST 2.3.0 — real Silesia + Calgary

Measured 2026-08-30 on this machine. Official files. Lossless (`DECODE_OK`).

## 2.3.0 levers (added this session)

Wired into BW22, per-block pick of the smallest:

1. **MTF-1** (one-step move) vs full MTF
2. **Piecewise-static 4-ctx rANS** on Wheeler tokens (32 Ki token windows)
3. **Unmixed streams** — selector + zero-run lengths + nonzero ranks, each with its own ctx rANS

On official Calgary the picker still selects whole-block MTF + Wheeler + 4-ctx on every file tried (`LEVER_DEBUG`). MTF-1 and 32k piecewise lose. Unmix is close on book1 (243,016 vs 241,194) and does not win. First-try LZP-2 / bubble-WFC / bitplane-split were measured and dropped from the pick grid because they expanded.

Silesia 2.2.2 totals below still stand: same winning path. Calgary 2.3.0 re-measured.

- Silesia: GitHub `MiloszKrajewski/SilesiaCorpus` — 12 files, **211,938,580** bytes.
- Calgary: `corpus.canterbury.ac.nz/resources/calgary.zip` — 18-file large set **3,251,493**; standard 14-file **3,141,622**.
- Engine: emit-gated min(OZL2, PZ22 on <64KiB, BW22).
- BW22 2.3.0: RLE-1 + BWT (900 KiB, virtual sentinel SA) + MTF/MTF-1 + (Wheeler 4-ctx | piecewise | unmixed). Winner on official text is still MTF + Wheeler + 4-ctx.
- References: `gzip -9`, `bzip2 -9`, `xz -6` (reference sizes reused from the 2.2.0 official table; gzip/bzip2 totals match the published industry table).
- Not a Silesia rank claim. Combined GC remains the product face.

## Silesia totals

| codec | bytes | ratio |
|---|---:|---:|
| raw | 211,938,580 | 1.0000 |
| gzip -9 | 67,631,990 | 0.3191 |
| **pulsar-best 2.2.2/2.3.0 (BW22)** | **56,654,942** | **0.2673** |
| bzip2 -9 | 54,506,769 | 0.2572 |
| xz -6 | 49,233,340 | 0.2323 |

vs 2.2.0 OZL2 (68,510,216): **−11,855,274** bytes.

Beats gzip-9 on **12/12** files. Beats bzip2-9 on **0/12**. Beats xz-6 on **0/12**.
Gap to bzip2-9: **+2,148,173** bytes (**+3.94%**).

## Silesia per file

| file | raw | pulsar | gzip-9 | bzip2-9 | xz-6 | vs bz | ok |
|---|---:|---:|---:|---:|---:|---:|---|
| xml | 5,345,280 | 473,150 | 662,284 | 441,186 | 453,260 | +7.2% | OK |
| ooffice | 6,152,192 | 2,936,768 | 3,090,442 | 2,862,526 | 2,426,816 | +2.6% | OK |
| reymont | 6,627,202 | 1,307,469 | 1,820,834 | 1,246,230 | 1,317,152 | +4.9% | OK |
| sao | 7,251,944 | 5,121,635 | 5,327,041 | 4,940,524 | 4,415,072 | +3.7% | OK |
| x-ray | 8,474,240 | 4,276,720 | 6,037,713 | 4,051,112 | 4,489,912 | +5.6% | OK |
| mr | 9,970,564 | 2,647,404 | 3,673,940 | 2,441,280 | 2,750,228 | +8.4% | OK |
| osdb | 10,085,684 | 2,947,692 | 3,716,342 | 2,802,792 | 2,850,104 | +5.2% | OK |
| dickens | 10,192,446 | 2,907,619 | 3,851,823 | 2,799,520 | 2,831,676 | +3.9% | OK |
| samba | 21,606,400 | 4,719,479 | 5,408,272 | 4,549,759 | 3,787,400 | +3.7% | OK |
| nci | 33,553,445 | 1,886,804 | 2,987,533 | 1,812,734 | 1,779,272 | +4.1% | OK |
| webster | 41,458,703 | 8,948,058 | 12,061,624 | 8,644,714 | 8,628,848 | +3.5% | OK |
| mozilla | 51,220,480 | 18,482,144 | 18,994,142 | 17,914,392 | 13,503,600 | +3.2% | OK |
| **total** | **211,938,580** | **56,654,942** | **67,631,990** | **54,506,769** | **49,233,340** | **+3.94%** | |

## Calgary large 18

| codec | bytes | ratio |
|---|---:|---:|
| raw | 3,251,493 | 1.0000 |
| gzip -9 | 1,059,440 | 0.3258 |
| **pulsar-best 2.3.0** | **916,780** | **0.2820** |
| xz -6 | 885,716 | 0.2724 |
| bzip2 -9 | 866,501 | 0.2665 |

Beats gzip-9 on **9/18**. Beats bzip2-9 on **0/18**. Gap to bzip2-9: **+50,279** (**+5.80%**). Picker did not switch off the 2.2.2 path; tiny header delta only.

## Calgary per file (large 18)

| file | raw | pulsar | gzip-9 | bzip2-9 | xz-6 | vs gz | ok |
|---|---:|---:|---:|---:|---:|---|---|
| bib | 111,261 | 29,157 | 34,900 | 27,467 | 30,588 | beat gz | OK |
| book1 | 768,771 | 241,206 | 312,281 | 232,598 | 261,116 | beat gz | OK |
| book2 | 610,856 | 163,607 | 206,158 | 157,443 | 169,824 | beat gz | OK |
| geo | 102,400 | 62,427 | 68,414 | 56,921 | 53,364 | beat gz | OK |
| news | 377,109 | 122,985 | 144,400 | 118,600 | 118,900 | beat gz | OK |
| obj1 | 21,504 | 13,611 | 10,320 | 10,787 | 9,428 | lose | OK |
| obj2 | 246,814 | 80,537 | 81,087 | 76,441 | 61,504 | beat gz | OK |
| paper1 | 53,161 | 18,122 | 18,543 | 16,558 | 17,280 | beat gz | OK |
| paper2 | 82,199 | 26,860 | 29,667 | 25,041 | 27,228 | beat gz | OK |
| paper3 | 46,526 | 17,280 | 18,074 | 15,837 | 17,064 | beat gz | OK |
| paper4 | 13,286 | 6,263 | 5,534 | 5,188 | 5,408 | lose | OK |
| paper5 | 11,954 | 6,104 | 4,995 | 4,837 | 4,900 | lose | OK |
| paper6 | 38,105 | 13,787 | 13,213 | 12,292 | 12,504 | lose | OK |
| pic | 513,216 | 52,579 | 52,381 | 49,759 | 41,992 | lose | OK |
| progc | 39,611 | 14,013 | 13,261 | 12,544 | 12,560 | lose | OK |
| progl | 71,646 | 17,023 | 16,164 | 15,579 | 14,984 | lose | OK |
| progp | 49,379 | 12,036 | 11,186 | 10,710 | 10,352 | lose | OK |
| trans | 93,695 | 19,175 | 18,862 | 17,899 | 16,720 | lose | OK |
| **large 18** | **3,251,493** | **916,299** | **1,059,440** | **866,501** | **885,716** | | |

## Calgary standard 14

paper3–6 excluded. pulsar = 916,299 − (17,278+6,261+6,102+13,785) = **872,873**.

| codec | bytes | ratio |
|---|---:|---:|
| raw | 3,141,622 | 1.0000 |
| pulsar-best 2.2.2 | 872,873 | 0.2778 |
| gzip -9 | 1,017,624 | 0.3239 |
| xz -6 | 845,840 | 0.2692 |
| bzip2 -9 | 828,347 | 0.2637 |

## Read

Lossless on every official file. BW22 is now the winner on every Silesia file and on the large Calgary text files.

It **beats gzip-9 on the full official Silesia set** (56,654,942 vs 67,631,990).

It does **not** beat bzip2-9 on either official set (Silesia +3.94%, Calgary-18 +5.75%). Drive lock sizes (obj1 10,322 / osdb 27,445 / dickens 243,675) are **not** reproduced by this tree.

Next real ratio lever is adaptive / QLFC-class entropy on the Wheeler stream (static 4-ctx is the current ceiling), not another BWT invert fix. SA matches naive rotation sort on checked samples.
