//! Order-N mix with adaptive weights (lr=0.02).
//!
//! Codec lives in `cm` (bit-level mixer + range coder).
//! `mixer` is the original V2 weight helper used by Gate.

pub use crate::cm::{cm_decode, cm_encode, MAGIC};
pub use crate::mixer::{write_header, Mixer};
pub use crate::models_live::hangry_predictor;
