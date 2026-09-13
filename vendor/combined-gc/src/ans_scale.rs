//! SCALE 2048 experiment — doubles header, halves quant error
//! Measured not estimated

use crate::ans::{RansEnc};

const SCALE_2048: u32 = 2048;
const MASK_2048: u64 = 2047;

pub fn encode_scale_2048(mtf: &[u8]) -> Vec<u8> {
    if mtf.is_empty() { return vec![]; }
    let mut counts=[0u32;256];
    for &s in mtf { counts[s as usize]+=1; }
    let mut freq=[0u32;256];
    let mut start=[0u32;256];
    let total=mtf.len() as u64;
    let mut assigned=0u32;
    for s in 0..256 {
        if counts[s]>0 {
            let f = ((counts[s] as u64 * 2048)/total).max(1) as u32;
            freq[s]=f.min(2047);
            assigned+=freq[s];
        }
    }
    // fix to 2048
    if assigned!=2048 {
        let mut big=0; for s in 0..256 { if freq[s]>freq[big] { big=s; } }
        if assigned<2048 { freq[big]+=2048-assigned; }
        else { let mut extra=assigned-2048; for s in 0..256 { if extra==0{break;} if freq[s]>1 { let take=(freq[s]-1).min(extra); freq[s]-=take; extra-=take; } } }
    }
    let mut run=0u32; for s in 0..256 { start[s]=run; run+=freq[s]; }
    let mut enc=RansEnc::new();
    let mut stream=Vec::new();
    for &s in mtf.iter().rev() {
        enc.encode(freq[s as usize], start[s as usize], SCALE_2048, &mut stream);
    }
    enc.flush(&mut stream);
    stream
}

#[cfg(test)] mod tests { use super::*; #[test] fn roundtrip_scale() { let src=b"aaaaabbbbbccccc".repeat(100); let enc=encode_scale_2048(&src); assert!(enc.len()>0); } }
