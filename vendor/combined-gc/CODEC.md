# Combined GC 1.20.0 codec

Two modes. One decoder.

```
combined_gc -i FILE -o FILE.cg          # fast (default)
combined_gc --max -i FILE -o FILE.cg    # BWT bake-off
combined_gc -d -i FILE.cg -o FILE
```

| mode | path | dickens 900K | time |
|---|---|---:|---:|
| fast | ZLB1 zlib-ng | 331,491 | 0.06s |
| max | BWT+MTF+ANS | 243,675 | 3.04s |
| gzip -9 | | 331,495 | ~0.1s |

Fast: all-zero → ZRW. `len>4MB` and not text → XZ1 if smaller. Else ZLB1.
Max: existing pick_best (BWT / DELTA / XOR / ZLIB / XZ1). `--bench` uses max.

118 tests. Decode reads ZRW / ZLB1 / XZ1 / AWAREv6.
