//! DELTA_V and XOR improvements
//! Current locks: x-ray 395,630 DELTA_V+BWT+DEFLATE, geo 55,552 XOR_F4+BWT+DEFLATE
//! Try ZigZag + BWT 2M and XOR_F8

pub fn delta_v_zigzag_encode(data: &[u8]) -> Vec<u8> {
    let mut out=Vec::with_capacity(data.len());
    let mut prev=0u8;
    for &b in data {
        let d=b.wrapping_sub(prev);
        // zigzag for signed delta
        let zz = ((d as i8 as i16) << 1) ^ ((d as i8 as i16) >> 15);
        out.push(zz as u8);
        prev=b;
    }
    out
}

pub fn delta_v_zigzag_decode(data: &[u8]) -> Vec<u8> {
    let mut out=Vec::with_capacity(data.len());
    let mut prev=0u8;
    for &zz in data {
        let d = ((zz as i16 >> 1) ^ (-((zz & 1) as i16))) as u8;
        let b=prev.wrapping_add(d);
        out.push(b);
        prev=b;
    }
    out
}

pub fn xor_f8_encode(data: &[u8]) -> Vec<u8> {
    let mut out=data.to_vec();
    // XOR with 8-byte sliding window of previous 8 bytes interpreted as f64 bits
    if data.len()>=8 {
        for i in 8..data.len() {
            out[i] ^= out[i-8];
        }
    }
    out
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn delta_zz_roundtrip() { let src=b"\x00\x01\x02\xFF\xFE".repeat(100); let enc=delta_v_zigzag_encode(&src); let dec=delta_v_zigzag_decode(&enc); assert_eq!(dec, src.to_vec()); }
    #[test] fn xor_f8_roundtrip() { let src=(0..1000).map(|i| (i%256) as u8).collect::<Vec<_>>(); let enc=xor_f8_encode(&src); let mut dec=enc.clone(); for i in (8..dec.len()).rev() { dec[i] ^= dec[i-8]; } assert_eq!(dec, src); }
}
