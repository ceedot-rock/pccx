//! 200K–900K BWT via suffix array.
//!
//! `build_sa` / `build_sa_cyclic` are prefix-doubling (O(n log² n)).
//! SA-IS is the next swap behind the same `(L, primary)` API.

pub use crate::transforms::{
    build_sa, build_sa_cyclic, bwt_big_block, bwt_sa_indexed, ibwt_16k_indexed, BWT_BIG,
};

pub const BWT_200K: usize = 200_000;
pub const BWT_900K: usize = 900_000;
pub const BWT_2M: usize = 2_000_000;

pub fn bwt_900k(data: &[u8]) -> Vec<u8> {
    bwt_sa_indexed(data, BWT_900K)
}

pub fn bwt_mtf(data: &[u8], block: usize) -> Vec<u8> {
    crate::mtf::mtf_encode(&bwt_sa_indexed(data, block))
}
