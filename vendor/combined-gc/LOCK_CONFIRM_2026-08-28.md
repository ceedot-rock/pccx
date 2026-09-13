# Lock confirm 2026-08-28 — release binary, 114 tests

`cargo test --lib` → **114 passed**.

Release `combined_gc` 1.19.2, default features (no `cm`).

| file | raw | AWARE | program | gzip-9 | vs lock |
|---|---:|---:|---|---:|---|
| dickens 900K | 900,000 | **243,675** | BWT+MTF+ANS | 331,495 | hold |
| obj1 | 21,504 | **10,322** | ZLIB | 10,326 | hold −4 |
| osdb 64K | 65,536 | **27,445** | ZLIB | 27,449 | hold −4 |
| geo default pack | 102,400 | **55,552** | XOR_F4+BWT+DEFLATE | 68,668 | hold |
| geo `--block 64K` | 102,400 | 56,386 | XOR_F4+BWT+DEFLATE | 68,668 | don't force 64K |
| x-ray 900K | 900,000 | **395,630** | DELTA_V+BWT+DEFLATE | 604,752 | hold |

XZ1 (xz -9 + 8B header), gate `len>4MB && !text`:

| file | XZ1 | vs old AWARE |
|---|---:|---:|
| mozilla | 13,503,608 | −4,461,150 |
| samba | 3,763,624 | −864,111 |
| sao | 4,415,080 | −1,523,512 |
| ooffice | 2,426,824 | −455,575 |

Silesia 12-file total **47,881,808** (xz 49,232,072 / bzip2 54,506,769 / gzip 67,631,990).

Rejected this pass: CM vs 243,675 (ANS wins). Delta16 on mr 2MB 507,554 vs DELTA_V 472,656.

Open: mr 2,481,623 vs bzip2 2,441,280 (**+40,343**).
