Copy `pcc.rs` over `lbr1/splb/src/pcc.rs`.
Enable `aware = ["dep:combined-gc"]` and `default = ["aware"]` in `splb/Cargo.toml`.
Point `combined-gc` at the lab Combined GC crate.
Replace `combined_gc::codec::*` with `combined_gc::frame::compress` / `decompress`.
