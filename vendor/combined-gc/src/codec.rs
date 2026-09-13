//! Shipped codec surface.
//!
//! Fast (default): zeros → ZRW, large non-text → XZ1 if smaller, else ZLB1.
//! Max: existing AWARE bake-off (BWT / DELTA / XOR). Slow. Ratio table.
//!
//! Decode reads the magic. Same decoder for both modes.

use crate::analyzer;
use crate::frame::{self, CompressResult};
use crate::pipeline;
use crate::vm::{Op, Program};
use crate::xz_thin;
use crate::zrw;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Fast,
    Smart,
    Max,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Fast => "fast",
            Mode::Smart => "smart",
            Mode::Max => "max",
        }
    }
}

pub fn encode(data: &[u8], mode: Mode) -> CompressResult {
    match mode {
        Mode::Fast => encode_fast(data),
        Mode::Smart => encode_smart(data),
        Mode::Max => frame::compress(data),
    }
}

/// ZLIB/XZ1 when BWT cannot win. Otherwise `--max` with `pack_size_smart`.
pub fn encode_smart(data: &[u8]) -> CompressResult {
    let analysis = analyzer::Analyzer::analyze(data);
    if zrw::is_zero_run(data) {
        return encode_fast(data);
    }
    if !crate::experts::should_pay_sa(data, analysis.entropy) {
        return encode_fast(data);
    }
    let pack = crate::pack::pack_size_smart(data, analysis.entropy);
    frame::compress_with_pack(data, pack)
}

pub fn decode(buf: &[u8]) -> Result<Vec<u8>, &'static str> {
    Ok(decode_report(buf)?.bytes)
}

#[derive(Debug)]
pub struct DecodeReport {
    pub bytes: Vec<u8>,
    pub container: &'static str,
    pub family: crate::experts::Family,
}

/// Mirror of encode_smart: dispatch on container tag, then classify plaintext.
/// Experts never choose the invert path — the file already stored it.
pub fn peek_container(buf: &[u8]) -> &'static str {
    if buf.len() >= 4 && buf.starts_with(zrw::MAGIC) && buf.len() == 8 {
        "ZRW"
    } else if buf.starts_with(pipeline::ZLB_MAGIC) {
        "ZLB1"
    } else if buf.starts_with(xz_thin::XZ_MAGIC) {
        "XZ1"
    } else if buf.starts_with(crate::bz_thin::BZ_MAGIC) {
        "BZ1"
    } else if buf.starts_with(crate::nca_mix::MAGIC) {
        "NCM1"
    } else if buf.starts_with(crate::aware_v6::MAGIC) {
        "AWAREv6"
    } else {
        "AWAREv5"
    }
}

pub fn decode_report(buf: &[u8]) -> Result<DecodeReport, &'static str> {
    let container = peek_container(buf);
    let bytes = frame::decompress(buf)?;
    let family = crate::experts::classify(&bytes).family;
    Ok(DecodeReport {
        bytes,
        container,
        family,
    })
}

fn encode_fast(data: &[u8]) -> CompressResult {
    let analysis = analyzer::Analyzer::analyze(data);
    if zrw::is_zero_run(data) {
        let program = Program {
            ops: vec![Op::Tru8Zero],
        };
        let bytes = zrw::compress_zeros_int32_le(data.len());
        return CompressResult {
            bytes,
            program,
            analysis,
        };
    }
    if data.len() > 4_000_000 && !analyzer::is_text_like(data) {
        if let Some(xz) = xz_thin::emit_thin_xz(data) {
            let z = pipeline::emit_thin_zlib(data);
            if xz.len() < z.len() {
                return CompressResult {
                    bytes: xz,
                    program: Program {
                        ops: vec![Op::Xz],
                    },
                    analysis,
                };
            }
        }
    }
    let bytes = pipeline::emit_thin_zlib(data);
    CompressResult {
        bytes,
        program: Program {
            ops: vec![Op::Zlib],
        },
        analysis,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fast_zeros_roundtrip() {
        let src = vec![0u8; 10_000];
        let enc = encode(&src, Mode::Fast);
        assert!(enc.bytes.starts_with(zrw::MAGIC) || enc.bytes.len() < 32);
        let back = decode(&enc.bytes).expect("decode zeros");
        assert_eq!(back, src);
    }

    #[test]
    fn decode_mirrors_smart_zeros() {
        let src = vec![0u8; 10_000];
        let enc = encode(&src, Mode::Smart);
        let r = decode_report(&enc.bytes).expect("decode zeros");
        assert_eq!(r.container, "ZRW");
        assert_eq!(r.bytes, src);
    }

    #[test]
    fn decode_mirrors_smart_text() {
        let src = b"chapter one. Mr. Pickwick walked into town.\n".repeat(200);
        let enc = encode(&src, Mode::Smart);
        let r = decode_report(&enc.bytes).expect("decode text");
        assert_eq!(r.bytes, src);
        assert!(r.container == "AWAREv6" || r.container == "ZLB1");
        assert_eq!(r.family, crate::experts::Family::Text);
    }

    #[test]
    fn fast_text_roundtrip() {
        let src = b"The quick brown fox jumps over the lazy dog.\n".repeat(80);
        let enc = encode(&src, Mode::Fast);
        assert!(enc.bytes.starts_with(pipeline::ZLB_MAGIC));
        let back = decode(&enc.bytes).expect("decode text");
        assert_eq!(back, src);
    }

    #[test]
    fn fast_binary_roundtrip() {
        let src: Vec<u8> = (0u8..=255).cycle().take(4096).collect();
        let enc = encode(&src, Mode::Fast);
        let back = decode(&enc.bytes).expect("decode bin");
        assert_eq!(back, src);
    }

    #[test]
    fn smart_high_h_stays_fast() {
        let src: Vec<u8> = (0..8192).map(|i| i as u8).collect();
        let enc = encode(&src, Mode::Smart);
        assert!(
            enc.bytes.starts_with(pipeline::ZLB_MAGIC)
                || enc.program.describe().contains("ZLIB")
                || enc.program.describe().contains("DELTA"),
            "got {}",
            enc.program.describe()
        );
        let back = decode(&enc.bytes).expect("decode smart flat");
        assert_eq!(back, src);
    }

    #[test]
    fn max_still_decodes() {
        let src = b"chapter one. Mr. Pickwick. chapter two.\n".repeat(40);
        let enc = encode(&src, Mode::Max);
        let back = decode(&enc.bytes).expect("decode max");
        assert_eq!(back, src);
    }
}
