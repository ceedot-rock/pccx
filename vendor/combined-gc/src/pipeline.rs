//! Invertible op pipeline. Lz = flate2 DEFLATE. Ans = order-0 rANS.

use crate::ans;
use crate::blackjack;
use crate::models_live;
use crate::mtf::{mtf_rle0_decode, mtf_rle0_decode_seeded, mtf_rle0_encode, mtf_rle0_encode_seeded};
use crate::transforms::{bwt_sa_indexed, ibwt_16k_indexed, BWT_BIG};
use crate::vm::{Op, Program};
use crate::zrw;
use flate2::read::{DeflateDecoder, ZlibDecoder};
use flate2::write::{DeflateEncoder, ZlibEncoder};
use flate2::Compression;
use std::io::{Read, Write};

pub fn apply(program: &Program, data: &[u8]) -> Vec<u8> {
    apply_seeded_dict(program, data, None, &[])
}

pub fn apply_seeded(program: &Program, data: &[u8], mtf_seed: Option<&[u8; 256]>) -> Vec<u8> {
    apply_seeded_dict(program, data, mtf_seed, &[])
}

pub fn apply_seeded_dict(
    program: &Program,
    data: &[u8],
    mtf_seed: Option<&[u8; 256]>,
    dict: &[u8],
) -> Vec<u8> {
    if program.ops.iter().any(|o| matches!(o, Op::Tru8Zero)) && zrw::is_zero_run(data) {
        return zrw::compress_zeros_int32_le(data.len());
    }
    let mut cur = data.to_vec();
    let mut deflate_end = false;
    let mut zlib_end = false;
    let mut ans_end = false;
    for op in &program.ops {
        match op {
            Op::Store | Op::Neural => {}
            Op::Delta { order } => cur = delta(&cur, *order),
            Op::XorFloat => cur = xor_float(&cur),
            Op::Rle => cur = rle(&cur),
            Op::Bwt => cur = bwt_sa_indexed(&cur, BWT_BIG),
            Op::Lz => deflate_end = true,
            Op::Zlib => zlib_end = true,
            Op::Ans => ans_end = true,
            Op::Tru8Zero => {}
            Op::BlackjackDelta2 => cur = bj_delta2(&cur),
            Op::Hangry { order } => cur = models_live::hangry_residual(&cur, *order).0,
            Op::Qq { q } => cur = qq_lossy(&cur, *q as i32),
            Op::Mtf => {
                cur = if let Some(seed) = mtf_seed {
                    mtf_rle0_encode_seeded(&cur, seed)
                } else {
                    mtf_rle0_encode(&cur)
                };
            }
            Op::DictLz => cur = crate::lz_opt::prefilter(&cur, dict),
            Op::Dedup => cur = crate::dedup::encode(&cur),
            Op::XorF4 => cur = crate::transforms::xor_f4(&cur),
            Op::DeltaVar => cur = crate::transforms::delta16_varint(&cur),
            Op::Delta16 => cur = crate::transforms::delta_u16(&cur),
            Op::Lzp => cur = crate::lzp::encode(&cur),
            Op::Wfc => cur = crate::wfc::wfc_rle0_encode(&cur),
            Op::Bcj => cur = crate::bcj::encode(&cur),
            Op::Xz => {}
            Op::Bzip => {}
            Op::MixCm => {}
        }
    }
    if deflate_end {
        cur = deflate(&cur);
    }
    if zlib_end {
        cur = zlib_encode(&cur);
    }
    if ans_end {
        cur = crate::clean_o1::encode_auto(&cur);
    }
    cur
}

pub fn invert(program: &Program, residual: &[u8], orig_len: usize) -> Result<Vec<u8>, &'static str> {
    invert_seeded_dict(program, residual, orig_len, None, &[])
}

pub fn invert_seeded(
    program: &Program,
    residual: &[u8],
    orig_len: usize,
    mtf_seed: Option<&[u8; 256]>,
) -> Result<Vec<u8>, &'static str> {
    invert_seeded_dict(program, residual, orig_len, mtf_seed, &[])
}

pub fn invert_seeded_dict(
    program: &Program,
    residual: &[u8],
    orig_len: usize,
    mtf_seed: Option<&[u8; 256]>,
    dict: &[u8],
) -> Result<Vec<u8>, &'static str> {
    if program.ops.iter().any(|o| matches!(o, Op::Tru8Zero))
        && residual.len() == 8
        && residual.starts_with(zrw::MAGIC)
    {
        return zrw::decompress_zeros(residual);
    }
    let deflate_end = program.ops.iter().any(|o| matches!(o, Op::Lz));
    let zlib_end = program.ops.iter().any(|o| matches!(o, Op::Zlib));
    let ans_end = program.ops.iter().any(|o| matches!(o, Op::Ans));
    let mut cur = if ans_end {
        ans::rans_decode(residual)?
    } else if zlib_end {
        zlib_decode(residual)?
    } else if deflate_end {
        inflate(residual)?
    } else {
        residual.to_vec()
    };
    for op in program.ops.iter().rev() {
        match op {
            Op::Store | Op::Neural | Op::Tru8Zero | Op::Lz | Op::Zlib | Op::Ans | Op::MixCm | Op::Xz | Op::Bzip => {}
            Op::DictLz => cur = crate::lz_opt::unprefilter(&cur, dict, orig_len)?,
            Op::Dedup => cur = crate::dedup::decode(&cur)?,
            Op::XorF4 => cur = crate::transforms::unxor_f4(&cur),
            Op::DeltaVar => cur = crate::transforms::undelta16_varint(&cur),
            Op::Delta16 => cur = crate::transforms::undelta_u16(&cur),
            Op::Lzp => cur = crate::lzp::decode(&cur, orig_len)?,
            Op::Wfc => cur = crate::wfc::wfc_rle0_decode(&cur),
            Op::Bcj => cur = crate::bcj::decode(&cur),
            Op::Xz => {}
            Op::Delta { order } => cur = undelta(&cur, *order),
            Op::XorFloat => cur = unxor_float(&cur),
            Op::Rle => {
                let cap = if program.ops.iter().any(|o| matches!(o, Op::Bwt)) {
                    usize::MAX
                } else {
                    orig_len
                };
                cur = unrle(&cur, cap);
            }
            Op::Bwt => cur = ibwt_16k_indexed(&cur)?,
            Op::BlackjackDelta2 => cur = un_bj_delta2(&cur, orig_len),
            Op::Hangry { order } => cur = models_live::hangry_undelta(&cur, *order),
            Op::Qq { .. } => {}
            Op::Mtf => {
                cur = if let Some(seed) = mtf_seed {
                    mtf_rle0_decode_seeded(&cur, seed)
                } else {
                    mtf_rle0_decode(&cur)
                };
            }
        }
    }
    cur.truncate(orig_len);
    if cur.len() < orig_len && program.ops.iter().all(|o| matches!(o, Op::Store | Op::Neural))
    {
        return Err("pipeline: store residual short");
    }
    Ok(cur)
}

pub fn fitness(program: &Program, data: &[u8]) -> usize {
    program.size() + apply(program, data).len()
}

fn deflate(data: &[u8]) -> Vec<u8> {
    let mut enc = DeflateEncoder::new(Vec::new(), Compression::best());
    if enc.write_all(data).is_err() {
        return data.to_vec();
    }
    enc.finish().unwrap_or_else(|_| data.to_vec())
}

fn inflate(data: &[u8]) -> Result<Vec<u8>, &'static str> {
    let mut dec = DeflateDecoder::new(data);
    let mut out = Vec::new();
    dec.read_to_end(&mut out).map_err(|_| "pipeline: inflate")?;
    Ok(out)
}

pub fn zlib_encode(data: &[u8]) -> Vec<u8> {
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::best());
    if enc.write_all(data).is_err() {
        return data.to_vec();
    }
    enc.finish().unwrap_or_else(|_| data.to_vec())
}

pub fn zlib_decode(data: &[u8]) -> Result<Vec<u8>, &'static str> {
    let mut dec = ZlibDecoder::new(data);
    let mut out = Vec::new();
    dec.read_to_end(&mut out).map_err(|_| "pipeline: zlib")?;
    Ok(out)
}

/// Standalone thin file: ZLB1 | u32 orig_len | zlib payload.
pub const ZLB_MAGIC: &[u8; 4] = b"ZLB1";

pub fn emit_thin_zlib(data: &[u8]) -> Vec<u8> {
    let payload = zlib_encode(data);
    let mut out = Vec::with_capacity(8 + payload.len());
    out.extend_from_slice(ZLB_MAGIC);
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&payload);
    out
}

pub fn parse_thin_zlib(buf: &[u8]) -> Result<Vec<u8>, &'static str> {
    if buf.len() < 8 || &buf[..4] != ZLB_MAGIC {
        return Err("zlb1: magic");
    }
    let orig = u32::from_le_bytes(buf[4..8].try_into().unwrap()) as usize;
    let out = zlib_decode(&buf[8..])?;
    if orig != 0 && out.len() != orig {
        return Err("zlb1: length");
    }
    Ok(out)
}

fn delta(data: &[u8], order: u8) -> Vec<u8> {
    if data.is_empty() || order == 0 {
        return data.to_vec();
    }
    let order = order as usize;
    let mut out = Vec::with_capacity(data.len());
    out.extend_from_slice(&data[..order.min(data.len())]);
    for i in order..data.len() {
        out.push(data[i].wrapping_sub(data[i - order]));
    }
    out
}

fn undelta(data: &[u8], order: u8) -> Vec<u8> {
    if data.is_empty() || order == 0 {
        return data.to_vec();
    }
    let order = order as usize;
    let mut out = Vec::with_capacity(data.len());
    out.extend_from_slice(&data[..order.min(data.len())]);
    for i in order..data.len() {
        let prev = out[i - order];
        out.push(data[i].wrapping_add(prev));
    }
    out
}

fn xor_float(data: &[u8]) -> Vec<u8> {
    if data.len() < 4 {
        return data.to_vec();
    }
    let mut out = Vec::with_capacity(data.len());
    out.extend_from_slice(&data[0..4]);
    let mut prev = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let mut i = 4;
    while i + 3 < data.len() {
        let cur = u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]);
        out.extend_from_slice(&(cur ^ prev).to_le_bytes());
        prev = cur;
        i += 4;
    }
    out.extend_from_slice(&data[i..]);
    out
}

fn unxor_float(data: &[u8]) -> Vec<u8> {
    if data.len() < 4 {
        return data.to_vec();
    }
    let mut out = Vec::with_capacity(data.len());
    out.extend_from_slice(&data[0..4]);
    let mut prev = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let mut i = 4;
    while i + 3 < data.len() {
        let x = u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]);
        let cur = x ^ prev;
        out.extend_from_slice(&cur.to_le_bytes());
        prev = cur;
        i += 4;
    }
    out.extend_from_slice(&data[i..]);
    out
}

/// WAVE 4 RLE: marker 0xFE, escaped as FE 00.
fn rle(data: &[u8]) -> Vec<u8> {
    if data.is_empty() {
        return vec![];
    }
    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        let mut run = 1usize;
        while i + run < data.len() && data[i + run] == b && run < 255 {
            run += 1;
        }
        if run >= 4 {
            // FE <run> <byte> — run never 0, so FE 00 stays the literal-FE escape
            out.extend_from_slice(&[0xFE, run as u8, b]);
            i += run;
        } else if b == 0xFE {
            out.extend_from_slice(&[0xFE, 0x00]);
            i += 1;
        } else {
            out.push(b);
            i += 1;
        }
    }
    out
}

fn unrle(data: &[u8], original_len: usize) -> Vec<u8> {
    let cap = if original_len == usize::MAX {
        data.len().saturating_mul(2)
    } else {
        original_len
    };
    let mut out = Vec::with_capacity(cap);
    let mut i = 0;
    while i < data.len() && out.len() < original_len {
        if data[i] == 0xFE && i + 1 < data.len() {
            if data[i + 1] == 0x00 {
                out.push(0xFE);
                i += 2;
            } else if i + 2 < data.len() {
                let run = data[i + 1] as usize;
                let b = data[i + 2];
                for _ in 0..run {
                    if out.len() >= original_len {
                        break;
                    }
                    out.push(b);
                }
                i += 3;
            } else {
                out.push(data[i]);
                i += 1;
            }
        } else {
            out.push(data[i]);
            i += 1;
        }
    }
    out.truncate(original_len);
    out
}

fn qq_lossy(data: &[u8], q: i32) -> Vec<u8> {
    data.iter()
        .map(|&b| {
            let s = b as i8 as i32;
            let y = models_live::qq_soft_delta_q(s, q);
            y as u8
        })
        .collect()
}

fn bj_delta2(data: &[u8]) -> Vec<u8> {
    if data.len() < 12 || data.len() % 4 != 0 {
        return data.to_vec();
    }
    let ints: Vec<i32> = data
        .chunks_exact(4)
        .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    let body = blackjack::delta2_encode(&ints);
    let mut out = Vec::with_capacity(4 + body.len());
    out.extend_from_slice(&(ints.len() as u32).to_le_bytes());
    out.extend_from_slice(&ints[0].to_le_bytes());
    if ints.len() > 1 {
        out.extend_from_slice(&ints[1].to_le_bytes());
    }
    out.extend_from_slice(&body);
    out
}

fn un_bj_delta2(data: &[u8], orig_len: usize) -> Vec<u8> {
    if data.len() < 8 {
        return data.to_vec();
    }
    let n = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;
    if n == 0 {
        return vec![];
    }
    let a0 = i32::from_le_bytes(data[4..8].try_into().unwrap());
    if n == 1 {
        return a0.to_le_bytes().to_vec();
    }
    if data.len() < 12 {
        return data.to_vec();
    }
    let a1 = i32::from_le_bytes(data[8..12].try_into().unwrap());
    let d2: Vec<i32> = data[12..]
        .chunks_exact(4)
        .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    let mut d1 = vec![a1.wrapping_sub(a0)];
    for &v in &d2 {
        let prev = *d1.last().unwrap();
        d1.push(prev.wrapping_add(v));
    }
    let mut x = vec![a0, a1];
    for i in 1..d1.len() {
        let next = x[i].wrapping_add(d1[i]);
        x.push(next);
    }
    x.truncate(n);
    let mut out = Vec::with_capacity(n * 4);
    for v in x {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.truncate(orig_len);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delta_deflate_roundtrip() {
        let src: Vec<u8> = (0..200).map(|i| (i / 3) as u8).collect();
        let p = Program {
            ops: vec![Op::Delta { order: 1 }, Op::Lz],
        };
        let r = apply(&p, &src);
        let back = invert(&p, &r, src.len()).unwrap();
        assert_eq!(back, src);
    }

    #[test]
    fn bwt_mtf_rle_ans_roundtrip() {
        let src = b"mississippi river mississippi river mississippi\n".repeat(20);
        let p = Program {
            ops: vec![Op::Bwt, Op::Mtf, Op::Rle, Op::Ans],
        };
        let r = apply(&p, &src);
        let back = invert(&p, &r, src.len()).unwrap();
        assert_eq!(back, src);
    }

    #[test]
    fn bwt_mtf_rle_deflate_roundtrip() {
        let src = b"mississippi river mississippi river mississippi\n".repeat(20);
        let p = Program {
            ops: vec![Op::Bwt, Op::Mtf, Op::Rle, Op::Lz],
        };
        let r = apply(&p, &src);
        let back = invert(&p, &r, src.len()).unwrap();
        assert_eq!(back, src);
    }

    #[test]
    fn hangry5_deflate_roundtrip() {
        let src = b"Hangry Taylor order five residual must invert exactly.\n".repeat(3);
        let p = Program {
            ops: vec![Op::Hangry { order: 5 }, Op::Lz],
        };
        let r = apply(&p, &src);
        let back = invert(&p, &r, src.len()).unwrap();
        assert_eq!(back, src);
    }

    #[test]
    fn rle_roundtrip() {
        let src = [0u8, 0, 0, 0, 0, 1, 2, 2, 2, 2, 2, 3];
        let p = Program {
            ops: vec![Op::Rle],
        };
        let r = apply(&p, &src);
        let back = invert(&p, &r, src.len()).unwrap();
        assert_eq!(back, src);
    }
}
