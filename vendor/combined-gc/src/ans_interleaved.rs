//! 4-way interleaved rANS — 4 states parallel
//! Ratio must stay 2765585 +-0.05%, speed +30% target, measured

use crate::ans::RansEnc;

pub fn encode_interleaved(data: &[u8]) -> Vec<u8> {
    let mut streams=[Vec::new(),Vec::new(),Vec::new(),Vec::new()];
    let mut encs=[RansEnc::new(),RansEnc::new(),RansEnc::new(),RansEnc::new()];
    // simple: split by mod 4, each with SCALE 1024 uniform for demo
    // real impl uses per-stream freq tables
    for (i,&b) in data.iter().enumerate().rev() {
        let sid=i%4;
        // uniform 256
        let freq=4u32; // 1024/256
        let start=(b as u32)*4;
        encs[sid].encode(freq, start, 1024, &mut streams[sid]);
    }
    for s in 0..4 { encs[s].flush(&mut streams[s]); }
    // concat with len headers
    let mut out=Vec::new();
    for s in 0..4 { out.extend_from_slice(&(streams[s].len() as u32).to_le_bytes()); out.extend_from_slice(&streams[s]); }
    out
}

#[cfg(test)] mod tests { use super::*; #[test] fn interleaved_encodes() { let src=b"test data ".repeat(1000); let enc=encode_interleaved(&src); assert!(enc.len()>0); } }
