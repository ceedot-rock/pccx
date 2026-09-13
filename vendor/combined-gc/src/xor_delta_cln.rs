//! XOR_F4 / DELTA_V then BWT+MTF+CLN (encode_auto).

use crate::pipeline;
use crate::vm::{Op, Program};

pub fn xorf4_bwt_cln(data: &[u8]) -> Vec<u8> {
    pipeline::apply(
        &Program {
            ops: vec![Op::XorF4, Op::Bwt, Op::Mtf, Op::Ans],
        },
        data,
    )
}

pub fn deltav_bwt_cln(data: &[u8]) -> Vec<u8> {
    pipeline::apply(
        &Program {
            ops: vec![Op::DeltaVar, Op::Bwt, Op::Mtf, Op::Ans],
        },
        data,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xorf4_bwt_cln_roundtrip() {
        let src: Vec<u8> = (0u8..200).cycle().take(800).collect();
        let p = Program {
            ops: vec![Op::XorF4, Op::Bwt, Op::Mtf, Op::Ans],
        };
        let r = pipeline::apply(&p, &src);
        let back = pipeline::invert(&p, &r, src.len()).unwrap();
        assert_eq!(back, src);
        let _ = xorf4_bwt_cln(&src);
    }

    #[test]
    fn deltav_bwt_cln_roundtrip() {
        let mut src = Vec::new();
        let mut v = 40i16;
        for _ in 0..400 {
            src.extend_from_slice(&v.to_le_bytes());
            v = v.wrapping_add(1);
        }
        let p = Program {
            ops: vec![Op::DeltaVar, Op::Bwt, Op::Mtf, Op::Ans],
        };
        let r = pipeline::apply(&p, &src);
        let back = pipeline::invert(&p, &r, src.len()).unwrap();
        assert_eq!(back, src);
        let _ = deltav_bwt_cln(&src);
    }
}
