# Silesia — public measured table

Corpus: 12 official Silesia files, **211,938,580** bytes  
Source zip: https://sun.aei.polsl.pl/~sdeor/corpus/silesia.zip

Industry measured on Penguin 2026-08-29. AWARE freeze is Combined GC 1.19.2 + XZ1 (2026-08-28). **Do not invent cells.**

| codec | bytes | ratio | vs AWARE |
|---|---:|---:|---:|
| **zpaq 7.15 `-m5`** | **39,112,912** | **0.1845** | **−8,639,456** |
| **AWARE 1.19.2 + XZ1** | **47,752,368** | **0.2253** | — |
| xz -9 | 48,795,480 | 0.2302 | +1,043,112 |
| brotli 1.1.0 `-q 11` | 49,564,563 | 0.2339 | +1,812,195 |
| zstd 1.5.5 `-19` | 53,024,573 | 0.2502 | +5,272,205 |
| bzip2 1.0.8 `-9` | 54,506,769 | 0.2572 | +6,754,401 |
| zstd 1.5.5 `-3` | 66,547,664 | 0.3140 | +18,795,296 |
| gzip 1.12 `-9` | 67,631,990 | 0.3191 | +19,879,622 |

Honest read: **zpaq-m5 is smaller than AWARE on this corpus** (8,639,456 bytes). zpaq is the slow max-reference compressor (x-ray 13,321 s, samba 15,076 s). **AWARE still smaller than xz-9, brotli-11, zstd-19, and gzip-9.** Gate is not claimed here. Not a universal #1.

XZ1 = host xz-9 + 8-byte wrap on mozilla, samba, sao, ooffice.

Machine JSON: `benches/silesia-brotli-zpaq-2026-08-29.json`

## brotli 1.1.0 `-q 11` per file (complete)

| file | raw | brotli-11 | seconds |
|---|---:|---:|---:|
| dickens | 10,192,446 | 2,827,777 | 76.2 |
| mozilla | 51,220,480 | 13,872,265 | 784.1 |
| mr | 9,970,564 | 2,823,136 | 66.5 |
| nci | 33,553,445 | 1,519,768 | 151.5 |
| ooffice | 6,152,192 | 2,478,855 | 40.9 |
| osdb | 10,085,684 | 2,816,278 | 54.2 |
| reymont | 6,627,202 | 1,332,158 | 35.1 |
| samba | 21,606,400 | 3,766,340 | 98.7 |
| sao | 7,251,944 | 4,586,092 | 59.1 |
| webster | 41,458,703 | 8,428,575 | 222.7 |
| xml | 5,345,280 | 430,566 | 23.7 |
| x-ray | 8,474,240 | 4,682,753 | 66.3 |
| **total** | **211,938,580** | **49,564,563** | |

## zpaq 7.15 `-m5` per file (complete — separate archives)

| file | raw | zpaq-m5 | seconds |
|---|---:|---:|---:|
| dickens | 10,192,446 | 2,094,774 | 95.7 |
| mozilla | 51,220,480 | 12,041,086 | 1,025.5 |
| mr | 9,970,564 | 2,181,336 | 76.1 |
| nci | 33,553,445 | 1,251,135 | 198.6 |
| ooffice | 6,152,192 | 1,766,581 | 65.7 |
| osdb | 10,085,684 | 2,204,769 | 91.4 |
| reymont | 6,627,202 | 956,530 | 77.6 |
| samba | 21,606,400 | 3,053,849 | 15,076.2 |
| sao | 7,251,944 | 3,899,285 | 102.8 |
| webster | 41,458,703 | 5,666,863 | 3,112.9 |
| xml | 5,345,280 | 326,974 | 50.6 |
| x-ray | 8,474,240 | 3,669,730 | 13,320.8 |
| **total** | **211,938,580** | **39,112,912** | |
