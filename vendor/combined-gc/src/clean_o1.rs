//! Binary O1 on zero vs nonzero + order-0 tail.
//! Adaptive 2-byte P(0|prev==0), P(0|prev!=0). No per-block zero tables.

use crate::ans::{rans_encode, RansEnc, RANS_L, SCALE, SCALE_BITS};

pub const MAGIC: &[u8; 4] = b"CLN1";
const _P0_GIVEN_0_STATIC: u32 = 3560;
const _P0_GIVEN_1_STATIC: u32 = 1310;

pub fn compute_p0_per_block(mtf: &[u8]) -> (u8, u8) {
    let mut cnt0_given0 = 0usize;
    let mut cnt_given0 = 0usize;
    let mut cnt0_given1 = 0usize;
    let mut cnt_given1 = 0usize;
    let mut prev_zero = true;
    for &s in mtf {
        let is_zero = s == 0;
        if prev_zero {
            cnt_given0 += 1;
            if is_zero {
                cnt0_given0 += 1;
            }
        } else {
            cnt_given1 += 1;
            if is_zero {
                cnt0_given1 += 1;
            }
        }
        prev_zero = is_zero;
    }
    let p0_g0 = if cnt_given0 > 0 {
        (cnt0_given0 * 255 / cnt_given0) as u8
    } else {
        221
    };
    let p0_g1 = if cnt_given1 > 0 {
        (cnt0_given1 * 255 / cnt_given1) as u8
    } else {
        81
    };
    (p0_g0, p0_g1)
}

fn p_from_u8(p: u8) -> u32 {
    let v = (p as u32 * SCALE) / 255;
    v.clamp(1, SCALE - 1)
}

pub fn encode(mtf: &[u8]) -> Vec<u8> {
    if mtf.is_empty() {
        return rans_encode(mtf);
    }
    let (p0g0, p0g1) = compute_p0_per_block(mtf);
    let p0_0 = p_from_u8(p0g0);
    let p0_1 = p_from_u8(p0g1);

    let mut bits = Vec::with_capacity(mtf.len());
    let mut tail = Vec::new();
    for &s in mtf {
        bits.push(if s == 0 { 1u8 } else { 0 });
        if s != 0 {
            tail.push(s);
        }
    }
    let mut prevs = vec![0u8; bits.len()];
    let mut prev = 1u8;
    for (i, &z) in bits.iter().enumerate() {
        prevs[i] = prev;
        prev = z;
    }

    let mut enc = RansEnc::new();
    let mut zstream = Vec::with_capacity(mtf.len() / 8 + 16);
    for i in (0..bits.len()).rev() {
        let p0 = if prevs[i] == 1 { p0_0 } else { p0_1 };
        // bits[i]==1 → symbol was 0 → encode bit 0 with freq p0
        if bits[i] == 1 {
            enc.encode(p0, 0, SCALE, &mut zstream);
        } else {
            enc.encode(SCALE - p0, p0, SCALE, &mut zstream);
        }
    }
    enc.flush(&mut zstream);
    let tail_bytes = rans_encode(&tail);

    let mut out = Vec::with_capacity(16 + zstream.len() + tail_bytes.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(mtf.len() as u32).to_le_bytes());
    out.push(SCALE_BITS as u8);
    out.push(p0g0);
    out.push(p0g1);
    out.extend_from_slice(&(zstream.len() as u32).to_le_bytes());
    out.extend_from_slice(&zstream);
    out.extend_from_slice(&(tail_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&tail_bytes);
    out
}

pub fn decode(buf: &[u8]) -> Result<Vec<u8>, &'static str> {
    if buf.len() < 4 + 4 + 1 + 2 + 4 + 8 + 4 {
        return Err("cln1: truncated");
    }
    if &buf[..4] != MAGIC {
        return Err("cln1: magic");
    }
    let orig_len = u32::from_le_bytes(buf[4..8].try_into().unwrap()) as usize;
    if buf[8] as u32 != SCALE_BITS {
        return Err("cln1: scale");
    }
    let p0_0 = p_from_u8(buf[9]);
    let p0_1 = p_from_u8(buf[10]);
    let mut pos = 11usize;
    let zlen = u32::from_le_bytes(buf[pos..pos + 4].try_into().unwrap()) as usize;
    pos += 4;
    if pos + zlen + 4 > buf.len() {
        return Err("cln1: zstream");
    }
    let zstream = &buf[pos..pos + zlen];
    pos += zlen;
    let tlen = u32::from_le_bytes(buf[pos..pos + 4].try_into().unwrap()) as usize;
    pos += 4;
    if pos + tlen > buf.len() {
        return Err("cln1: tail");
    }
    let tail = crate::ans::rans_decode(&buf[pos..pos + tlen])?;

    if zlen < 8 {
        return Err("cln1: state");
    }
    let mut state = u64::from_le_bytes(zstream[zlen - 8..].try_into().unwrap());
    let mut cursor = zlen - 8;
    let mask = (SCALE - 1) as u64;
    let mut bits = vec![0u8; orig_len];
    let mut prev = 1u8;
    for i in 0..orig_len {
        let p0 = if prev == 1 { p0_0 } else { p0_1 };
        let slot = (state & mask) as u32;
        let is_zero = if slot < p0 { 1u8 } else { 0 };
        let (f, cum) = if is_zero == 1 {
            (p0 as u64, 0u64)
        } else {
            ((SCALE - p0) as u64, p0 as u64)
        };
        state = f * (state >> SCALE_BITS) + (state & mask) - cum;
        while state < RANS_L {
            if cursor == 0 {
                return Err("cln1: underrun");
            }
            cursor -= 1;
            state = (state << 8) | zstream[cursor] as u64;
        }
        bits[i] = is_zero;
        prev = is_zero;
    }
    let mut out = Vec::with_capacity(orig_len);
    let mut ti = 0usize;
    for &z in &bits {
        if z == 1 {
            out.push(0);
        } else {
            if ti >= tail.len() {
                return Err("cln1: tail underrun");
            }
            out.push(tail[ti]);
            ti += 1;
        }
    }
    Ok(out)
}

pub fn encode_auto(data: &[u8]) -> Vec<u8> {
    let a0 = rans_encode(data);
    if data.len() < 128 {
        return a0;
    }
    let c4 = crate::order1_ans::encode(data);
    let cl = encode(data);
    let c8 = crate::clean_o8::encode(data);
    let c16 = crate::clean_o16::encode(data);
    let c24 = crate::clean_o24::encode(data);
    let c32 = crate::clean_o32::encode(data);
    let bt = crate::mtf_binary::encode(data);
    [a0, c4, cl, c8, c16, c24, c32, bt]
        .into_iter()
        .min_by_key(|v| v.len())
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_roundtrip() {
        let mut src = Vec::new();
        for _ in 0..600 {
            src.extend_from_slice(&[0, 0, 0, 0, 1, 0, 0, 2, 0, 3, 0, 0]);
        }
        let c = encode(&src);
        assert_eq!(decode(&c).unwrap(), src);
        assert_eq!(&c[..4], MAGIC);
    }
}
