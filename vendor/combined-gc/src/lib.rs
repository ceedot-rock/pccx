//! Combined GC — public surface is the real coders.

pub mod gate; // keeps routing, but routes to real coders
pub mod bwt_big; // 200K-900K SA (doubling today, SA-IS-shaped API)
pub mod mtf;
pub mod wfc;
pub mod ans; // rANS enc+dec
pub mod order1_ans; // 4-class O1 on MTF-RLE0
pub mod clean_o1; // binary O1 zero-stream + O0 tail
pub mod clean_o8; // 8-state split O1
pub mod clean_o16; // 16-state split O1
pub mod clean_o24; // 24-state split O1
pub mod clean_o32;
pub mod lzp;
pub mod mtf_binary;
pub mod xor_delta_cln;
pub mod dedup; // 16K BWT/MTF block copy
pub mod lz_opt; // DP parse + match finder across dict
pub mod mix; // order-N mix with adaptive weights
pub mod zrw; // keep flagship

pub mod analyzer;
pub mod aware_container;
pub mod aware_v6;
pub mod blackjack;
pub mod cm;
pub mod cont1088_table;
pub mod dict;
pub mod frame;
pub mod mixer;
pub mod models_live;
pub mod nca;
pub mod experts;
pub mod nca_mix;
pub mod nca_mtf;
pub mod pipeline;
pub mod transforms;
pub mod vm;

pub mod pack;
pub mod codec;
pub mod bcj;
pub mod xz_thin;
pub mod bz_thin;
pub mod text_detect;
pub mod wrt_dict;
pub mod ifm;
pub mod rle_gamma;
pub mod ans_scale;
pub mod ans_interleaved;
pub mod delta_xor;
pub mod analyzer_full;
#[cfg(feature = "experimental")]
pub mod match_maker;
#[cfg(feature = "experimental")]
pub mod match_maker_codec;
#[cfg(feature = "experimental")]
pub mod match_maker_zlib;

pub const VERSION: &str = "2.0.2-skip-sa";

pub fn version() -> &'static str {
    VERSION
}
