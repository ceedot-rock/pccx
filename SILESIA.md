# Silesia standing (PCCX 0.3.0)

Measured 2026-09-14. DECODE_OK where a PCCX size is given. Own path only (no host gzip/xz/bz2 wrap).

## First 256 KiB of all 12 files

| file | PCCX | gzip-9 | xz-9 | bzip2-9 |
|---|---:|---:|---:|---:|
| xml | 11974 | 16115 | 13060 | 11559 |
| nci | 14986 | 22000 | 15944 | 14563 |
| reymont | 54080 | 72353 | 62048 | 53158 |
| mr | 56829 | 79021 | 61252 | 55898 |
| webster | 62918 | 79166 | 69508 | 62139 |
| dickens | 78269 | 98593 | 85788 | 77252 |
| osdb | 85284 | 98799 | 84560 | 82475 |
| mozilla | 119813 | 120596 | 113000 | 122711 |
| x-ray | 120589 | 182634 | 137292 | 129632 |
| ooffice | 136719 | 142495 | 123528 | 132783 |
| samba | 169235 | 163070 | 156704 | 161546 |
| sao | 189097 | 195943 | 167536 | 182638 |

Sum: PCCX 1099793 · gzip 1349785 · xz 1190220 · bzip2 1165354.
Wins: gzip 11/12 · xz 7/12 · bzip2 2/12 (mozilla, x-ray).

## Full files (256 KiB tiles)

| file | PCCX | gzip-9 | bzip2-9 | xz-9 |
|---|---:|---:|---:|---:|
| xml | 503070 | 658917 | 441186 | 453260 |
| reymont | 1393750 | 1823208 | 1246230 | 1317152 |
| ooffice | 3162179 | 3092938 | 2862526 | 2426816 |
| mr | 2799688 | 3660806 | 2441280 | 2750272 |
| osdb | 3412448 | 3673189 | 2802792 | 2849908 |
| dickens | — | 3854747 | 2799520 | 2830604 |

dickens / mozilla / webster / nci / samba full-file encode did not complete in this lab (process kill ~8 MiB+).
