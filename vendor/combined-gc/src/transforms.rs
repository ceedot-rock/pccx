//! Byte transforms from the V2 dump.
//!
//! Honest limits:
//! - `bwt_16k_chunk` as dumped stores no primary index, so it is not invertible.
//!   `bwt_16k_indexed` / `ibwt_16k_indexed` add a u32 index per 16K chunk.
//! - RLE marker 0xFF is not escaped; a literal 0xFF byte is ambiguous.

pub const BWT_CHUNK: usize = 16_384;
/// Superblock for SA-BWT. 16K rotation-sort was the old path.
/// 256K is the first honest step toward bzip's ~900K blocks.
pub const BWT_BIG: usize = 2_000_000;

pub use crate::mtf::{mtf_decode, mtf_encode};

pub fn delta_transform(x: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(x.len());
    if x.is_empty() {
        return out;
    }
    out.push(x[0]);
    for i in 1..x.len() {
        out.push(x[i].wrapping_sub(x[i - 1]));
    }
    out
}

pub fn undelta_transform(x: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(x.len());
    if x.is_empty() {
        return out;
    }
    out.push(x[0]);
    for i in 1..x.len() {
        let prev = *out.last().unwrap();
        out.push(prev.wrapping_add(x[i]));
    }
    out
}

pub fn rle_encode(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < input.len() {
        let mut run = 1usize;
        while i + run < input.len() && input[i + run] == input[i] && run < 255 {
            run += 1;
        }
        if run >= 4 {
            out.extend_from_slice(&[0xFF, input[i], run as u8]);
            i += run;
        } else {
            out.push(input[i]);
            i += 1;
        }
    }
    out
}

/// Dumped BWT: last column only, no index. Forward-only.
pub fn bwt_16k_chunk(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for chunk in data.chunks(BWT_CHUNK) {
        if chunk.is_empty() {
            continue;
        }
        let mut sa: Vec<usize> = (0..chunk.len()).collect();
        sa.sort_by_key(|&j| &chunk[j..]);
        for &j in &sa {
            out.push(chunk[(j + chunk.len() - 1) % chunk.len()]);
        }
    }
    out
}

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Last SA time in ms for packs >= 256K. 0 = unknown (treat as under budget).
static LAST_SA_MS: AtomicU64 = AtomicU64::new(0);
pub const SA_BUDGET_MS: u64 = 200;

pub fn last_sa_ms() -> u64 {
    LAST_SA_MS.load(Ordering::Relaxed)
}

pub fn sa_under_budget() -> bool {
    let ms = last_sa_ms();
    ms == 0 || ms < SA_BUDGET_MS
}

#[cfg(test)]
pub fn set_last_sa_ms(ms: u64) {
    LAST_SA_MS.store(ms, Ordering::Relaxed);
}

/// Suffix array via libdivsufsort (induced sort). O(n).
/// libsais not vendored; same induced-sort API, timed for the 4MB guard.
pub fn build_sa(data: &[u8]) -> Vec<u32> {
    if data.is_empty() {
        return Vec::new();
    }
    let t0 = Instant::now();
    let mut sa = vec![0i32; data.len()];
    divsufsort::sort_in_place(data, &mut sa);
    if data.len() >= 256_000 {
        LAST_SA_MS.store(t0.elapsed().as_millis() as u64, Ordering::Relaxed);
    }
    sa.into_iter().map(|x| x as u32).collect()
}

/// Rotation SA via divsufsort on S||S. LF invert needs cyclic order.
pub fn build_sa_cyclic(data: &[u8]) -> Vec<u32> {
    let n = data.len();
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![0];
    }
    let mut doubled = Vec::with_capacity(n * 2);
    doubled.extend_from_slice(data);
    doubled.extend_from_slice(data);
    let sa = build_sa(&doubled);
    let mut rot = Vec::with_capacity(n);
    for &i in &sa {
        if (i as usize) < n {
            rot.push(i);
            if rot.len() == n {
                break;
            }
        }
    }
    rot
}

/// Cyclic BWT via SA(S||S). Suffix-SA of S alone is not invertible on repeats
/// (mississippi packs fail). `(L, primary)` + `ibwt`.
pub fn bwt_big_block(data: &[u8]) -> (Vec<u8>, usize) {
    let n = data.len();
    if n == 0 {
        return (Vec::new(), 0);
    }
    if n == 1 {
        return (vec![data[0]], 0);
    }
    let sa = build_sa_cyclic(data);
    let mut bwt = Vec::with_capacity(n);
    let mut primary = 0usize;
    for (i, &suf) in sa.iter().enumerate() {
        if suf == 0 {
            primary = i;
        }
        bwt.push(data[(suf as usize + n - 1) % n]);
    }
    (bwt, primary)
}

/// Pack SA-BWT blocks: `[u32 n][u32 primary][n bytes L]` per block.
pub fn bwt_sa_indexed(data: &[u8], block: usize) -> Vec<u8> {
    let block = block.max(1);
    let mut out = Vec::new();
    for chunk in data.chunks(block) {
        if chunk.is_empty() {
            continue;
        }
        let (l, primary) = bwt_big_block(chunk);
        let n = l.len() as u32;
        out.extend_from_slice(&n.to_le_bytes());
        out.extend_from_slice(&(primary as u32).to_le_bytes());
        out.extend_from_slice(&l);
    }
    out
}

/// Invertible BWT. Per chunk: `[u32 n][u32 primary][n bytes L]`.
/// Rotations, not suffix-only (the dumped `bwt_16k_chunk` is not invertible).
pub fn bwt_16k_indexed(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for chunk in data.chunks(BWT_CHUNK) {
        if chunk.is_empty() {
            continue;
        }
        let n = chunk.len();
        let mut sa: Vec<usize> = (0..n).collect();
        sa.sort_by(|&i, &j| {
            for k in 0..n {
                let a = chunk[(i + k) % n];
                let b = chunk[(j + k) % n];
                if a != b {
                    return a.cmp(&b);
                }
            }
            std::cmp::Ordering::Equal
        });
        let primary = sa.iter().position(|&j| j == 0).unwrap_or(0) as u32;
        out.extend_from_slice(&(n as u32).to_le_bytes());
        out.extend_from_slice(&primary.to_le_bytes());
        for &j in &sa {
            out.push(chunk[(j + n - 1) % n]);
        }
    }
    out
}

pub fn ibwt_16k_indexed(data: &[u8]) -> Result<Vec<u8>, &'static str> {
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < data.len() {
        if pos + 8 > data.len() {
            return Err("bwt: truncated header");
        }
        let n = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
        let primary = u32::from_le_bytes(data[pos + 4..pos + 8].try_into().unwrap()) as usize;
        pos += 8;
        if n == 0 {
            continue;
        }
        if pos + n > data.len() {
            return Err("bwt: truncated L");
        }
        if primary >= n {
            return Err("bwt: primary oob");
        }
        let l = &data[pos..pos + n];
        pos += n;
        out.extend_from_slice(&ibwt(l, primary));
    }
    Ok(out)
}

fn ibwt(l: &[u8], primary: usize) -> Vec<u8> {
    let n = l.len();
    let mut count = [0usize; 256];
    for &b in l {
        count[b as usize] += 1;
    }
    let mut c = [0usize; 256];
    let mut sum = 0;
    for b in 0..256 {
        c[b] = sum;
        sum += count[b];
    }
    let mut occ = [0usize; 256];
    let mut lf = vec![0usize; n];
    for i in 0..n {
        let b = l[i] as usize;
        lf[i] = c[b] + occ[b];
        occ[b] += 1;
    }
    let mut i = primary;
    let mut out = vec![0u8; n];
    for k in (0..n).rev() {
        out[k] = l[i];
        i = lf[i];
    }
    out
}

pub fn xor_float(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut prev: u32 = 0;
    for chunk in data.chunks(4) {
        if chunk.len() < 4 {
            out.extend_from_slice(chunk);
            break;
        }
        let cur = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        let x = cur ^ prev;
        out.extend_from_slice(&x.to_be_bytes());
        prev = cur;
    }
    out
}

pub fn xor_f4(data: &[u8]) -> Vec<u8> {
    let x = xor_float(data);
    let n4 = x.len() / 4;
    let mut out = Vec::with_capacity(4 + x.len());
    out.extend_from_slice(&(n4 as u32).to_le_bytes());
    for lane in 0..4 {
        for i in 0..n4 {
            out.push(x[i * 4 + lane]);
        }
    }
    out.extend_from_slice(&x[n4 * 4..]);
    out
}

pub fn unxor_f4(data: &[u8]) -> Vec<u8> {
    if data.len() < 4 {
        return data.to_vec();
    }
    let n4 = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;
    let need = 4 + n4 * 4;
    if data.len() < need {
        return data.to_vec();
    }
    let mut x = vec![0u8; n4 * 4];
    for lane in 0..4 {
        for i in 0..n4 {
            x[i * 4 + lane] = data[4 + lane * n4 + i];
        }
    }
    x.extend_from_slice(&data[need..]);
    unxor_float(&x)
}

fn put_uleb(out: &mut Vec<u8>, mut v: u32) {
    loop {
        let mut b = (v & 0x7f) as u8;
        v >>= 7;
        if v != 0 {
            b |= 0x80;
        }
        out.push(b);
        if v == 0 {
            break;
        }
    }
}

fn get_uleb(buf: &[u8], i: &mut usize) -> Option<u32> {
    let mut v = 0u32;
    let mut sh = 0;
    while *i < buf.len() {
        let b = buf[*i];
        *i += 1;
        v |= ((b & 0x7f) as u32) << sh;
        if b & 0x80 == 0 {
            return Some(v);
        }
        sh += 7;
        if sh > 28 {
            return None;
        }
    }
    None
}

pub fn delta_u16(data: &[u8]) -> Vec<u8> {
    if data.len() < 2 {
        return data.to_vec();
    }
    let mut out = Vec::with_capacity(data.len());
    out.extend_from_slice(&data[..2]);
    let mut i = 2;
    while i + 1 < data.len() {
        let prev = u16::from_le_bytes([data[i - 2], data[i - 1]]);
        let cur = u16::from_le_bytes([data[i], data[i + 1]]);
        out.extend_from_slice(&cur.wrapping_sub(prev).to_le_bytes());
        i += 2;
    }
    if i < data.len() {
        out.push(data[i]);
    }
    out
}

pub fn undelta_u16(data: &[u8]) -> Vec<u8> {
    if data.len() < 2 {
        return data.to_vec();
    }
    let mut out = Vec::with_capacity(data.len());
    out.extend_from_slice(&data[..2]);
    let mut i = 2;
    while i + 1 < data.len() {
        let prev = u16::from_le_bytes([out[i - 2], out[i - 1]]);
        let d = u16::from_le_bytes([data[i], data[i + 1]]);
        out.extend_from_slice(&prev.wrapping_add(d).to_le_bytes());
        i += 2;
    }
    if i < data.len() {
        out.push(data[i]);
    }
    out
}

pub fn delta16_varint(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + 4);
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    let rem = data.len() % 2;
    let mut prev = 0i32;
    let mut i = 0;
    while i + 2 <= data.len() {
        let v = i16::from_le_bytes([data[i], data[i + 1]]) as i32;
        let d = v.wrapping_sub(prev);
        prev = v;
        let zz = ((d << 1) ^ (d >> 31)) as u32;
        put_uleb(&mut out, zz);
        i += 2;
    }
    if rem == 1 {
        out.push(data[data.len() - 1]);
    }
    out
}

pub fn undelta16_varint(data: &[u8]) -> Vec<u8> {
    if data.len() < 4 {
        return data.to_vec();
    }
    let orig = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;
    let mut i = 4usize;
    let mut out = Vec::with_capacity(orig);
    let mut prev = 0i32;
    while out.len() + 2 <= orig {
        let zz = match get_uleb(data, &mut i) {
            Some(v) => v as i32,
            None => break,
        };
        let d = (zz >> 1) ^ -(zz & 1);
        let v = prev.wrapping_add(d);
        prev = v;
        out.extend_from_slice(&(v as i16).to_le_bytes());
    }
    if out.len() < orig && i < data.len() {
        out.push(data[i]);
    }
    out.truncate(orig);
    out
}

pub fn unxor_float(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut prev: u32 = 0;
    for chunk in data.chunks(4) {
        if chunk.len() < 4 {
            out.extend_from_slice(chunk);
            break;
        }
        let x = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        let cur = x ^ prev;
        out.extend_from_slice(&cur.to_be_bytes());
        prev = cur;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delta_roundtrip() {
        let src = b"abracadabra\x00\xff\x01";
        assert_eq!(undelta_transform(&delta_transform(src)), src);
    }

    #[test]
    fn xor_float_roundtrip() {
        let src: Vec<u8> = (0u8..40).collect();
        assert_eq!(unxor_float(&xor_float(&src)), src);
    }

    #[test]
    fn xor_f4_roundtrip() {
        let src: Vec<u8> = (0u8..80).collect();
        assert_eq!(unxor_f4(&xor_f4(&src)), src);
    }

    #[test]
    fn delta_u16_roundtrip() {
        let mut src = Vec::new();
        let mut v = 1000u16;
        for _ in 0..200 {
            src.extend_from_slice(&v.to_le_bytes());
            v = v.wrapping_add(3);
        }
        src.push(7);
        assert_eq!(undelta_u16(&delta_u16(&src)), src);
    }

    #[test]
    fn delta16_varint_roundtrip() {
        let mut src = Vec::new();
        let mut v = 1000i16;
        for _ in 0..200 {
            src.extend_from_slice(&v.to_le_bytes());
            v = v.wrapping_add(3);
        }
        src.push(7);
        assert_eq!(undelta16_varint(&delta16_varint(&src)), src);
    }

    #[test]
    fn mr_delta16_slice_if_present() {
        let p = "/home/workdir/artifacts/corpora/mr";
        let data = match std::fs::read(p) {
            Ok(d) => d,
            Err(_) => return,
        };
        let slice = &data[..data.len().min(2 * 1024 * 1024)];
        let d16 = crate::pipeline::apply(
            &crate::vm::Program {
                ops: vec![
                    crate::vm::Op::Delta16,
                    crate::vm::Op::Bwt,
                    crate::vm::Op::Mtf,
                    crate::vm::Op::Ans,
                ],
            },
            slice,
        );
        let dv = crate::pipeline::apply(
            &crate::vm::Program {
                ops: vec![
                    crate::vm::Op::DeltaVar,
                    crate::vm::Op::Bwt,
                    crate::vm::Op::Mtf,
                    crate::vm::Op::Ans,
                ],
            },
            slice,
        );
        eprintln!(
            "mr {}B  DELTA16+BWT+MTF+ANS={}  DELTA_V+BWT+MTF+ANS={}",
            slice.len(),
            d16.len(),
            dv.len()
        );
        assert!(d16.len() < slice.len());
        let back = crate::pipeline::invert(
            &crate::vm::Program {
                ops: vec![
                    crate::vm::Op::Delta16,
                    crate::vm::Op::Bwt,
                    crate::vm::Op::Mtf,
                    crate::vm::Op::Ans,
                ],
            },
            &d16,
            slice.len(),
        )
        .expect("invert");
        assert_eq!(back, slice);
    }

    #[test]
    fn mtf_roundtrip() {
        let src = b"banana-bandana-abracadabra";
        assert_eq!(mtf_decode(&mtf_encode(src)), src);
    }

    #[test]
    fn indexed_bwt_roundtrip() {
        let src = b"banana-bandana-abracadabra";
        let c = bwt_16k_indexed(src);
        let d = ibwt_16k_indexed(&c).unwrap();
        assert_eq!(d, src);
    }

    #[test]
    fn sa_bwt_banana() {
        let src = b"banana";
        let (l, primary) = bwt_big_block(src);
        assert_eq!(l.len(), 6);
        let back = ibwt(&l, primary);
        assert_eq!(back, src);
    }

    #[test]
    fn sa_matches_naive_on_repeat() {
        let src = b"mississippi river mississippi river mississippi\n".repeat(200);
        let chunk = &src[4096..8192];
        let got = build_sa(chunk);
        let mut naive: Vec<u32> = (0..chunk.len() as u32).collect();
        naive.sort_by(|&i, &j| chunk[i as usize..].cmp(&chunk[j as usize..]));
        assert_eq!(got, naive, "SA mismatch");
    }

    #[test]
    fn sa_bwt_packed_roundtrip() {
        let src = b"mississippi river mississippi river mississippi\n".repeat(200);
        for (i, chunk) in src.chunks(4096).enumerate() {
            let (l, primary) = bwt_big_block(chunk);
            let back = ibwt(&l, primary);
            assert_eq!(back, chunk, "chunk {i} n={}", chunk.len());
        }
        let c = bwt_sa_indexed(&src, 4096);
        let d = ibwt_16k_indexed(&c).expect("parse");
        assert_eq!(d.len(), src.len());
        assert_eq!(d, src);
    }

    #[test]
    fn sa_bwt_random_4k() {
        let src: Vec<u8> = (0..4096).map(|i| ((i * 131) ^ (i >> 3)) as u8).collect();
        let (l, primary) = bwt_big_block(&src);
        assert_eq!(ibwt(&l, primary), src);
    }
}
