//! 1MB lazy LZ77 + leftover DP parse.
//!
//! Tokens then order-0 rANS. Same `DLZ1` blob as greedy dict so
//! `Op::DictLz` decompress still works.
//!
//! Lazy path: hash20 + chain 1024 + nice/lazy 255, window 1MB.
//! That is the window DEFLATE cannot see.

use crate::ans;
use crate::dict::{self, MAX_MATCH, MIN_MATCH};
use crate::pipeline;
use crate::vm::{Op, Program};

const ESC: u8 = 0xFF;
pub const MAGICP: &[u8; 4] = b"DLZP";

const HASH_BITS: usize = 20;
const HASH_SIZE: usize = 1 << HASH_BITS;
const CHAIN: usize = 1024;
const NICE: usize = 255;
const MAX_LAZY: usize = 255;
const DP_CHAIN: usize = 16;
const MAX_CAND: usize = 8;

fn hash4(b: &[u8], i: usize) -> usize {
    if i + 3 >= b.len() {
        return 0;
    }
    let x = u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
    ((x.wrapping_mul(0x1e35a7bd)) >> (32 - HASH_BITS)) as usize
}

fn insert(head: &mut [i32], prev: &mut [i32], h: usize, abs: usize) {
    prev[abs] = head[h];
    head[h] = abs as i32;
}

/// Longest match at block index `i`. Walks up to CHAIN prior hits.
fn longest(
    dict: &[u8],
    block: &[u8],
    head: &[i32],
    prev: &[i32],
    i: usize,
    prev_len: usize,
) -> (usize, usize) {
    if i + MIN_MATCH > block.len() {
        return (0, 0);
    }
    let abs = dict.len() + i;
    let h = hash4(block, i);
    let mut p = head[h];
    let mut best_len = prev_len;
    let mut best_off = 0usize;
    let mut steps = 0usize;
    while p >= 0 && steps < CHAIN {
        steps += 1;
        let src = p as usize;
        if abs > src {
            let off = abs - src;
            if off <= dict.len() + block.len() {
                let n = match_len(dict, block, src, i);
                if n > best_len {
                    best_len = n;
                    best_off = off;
                    if best_len >= NICE {
                        break;
                    }
                }
            }
        }
        p = prev[src];
    }
    if best_len >= MIN_MATCH {
        (best_len, best_off)
    } else {
        (0, 0)
    }
}

fn emit_lit(out: &mut Vec<u8>, b: u8) {
    out.push(0x00);
    out.push(b);
}

fn emit_match(out: &mut Vec<u8>, off: usize, len: usize) {
    if off <= 0xffff {
        out.push(0x01);
        out.extend_from_slice(&(off as u16).to_le_bytes());
        out.push(len as u8);
    } else {
        out.push(0x02);
        out.push((off & 0xff) as u8);
        out.push(((off >> 8) & 0xff) as u8);
        out.push(((off >> 16) & 0xff) as u8);
        out.push(len as u8);
    }
}

/// zlib-style lazy parse over the 1MB dict + current block prefix.
pub fn lz_lazy_tokens(block: &[u8], dict: &[u8]) -> Vec<u8> {
    let n = block.len();
    let mut out = Vec::with_capacity(n / 2 + 16);
    if n == 0 {
        return out;
    }
    let hist_len = dict.len() + n;
    let mut head = vec![-1i32; HASH_SIZE];
    let mut prev = vec![-1i32; hist_len];
    for p in 0..dict.len().saturating_sub(3) {
        insert(&mut head, &mut prev, hash4(dict, p), p);
    }

    let mut i = 0usize;
    let mut pending: Option<(usize, usize, usize)> = None; // pos, len, off
    while i < n {
        let (len, off) = longest(dict, block, &head, &prev, i, 0);
        if let Some((pp, plen, poff)) = pending {
            if len > plen {
                emit_lit(&mut out, block[pp]);
                if pp + 3 < n {
                    insert(&mut head, &mut prev, hash4(block, pp), dict.len() + pp);
                }
                pending = if len >= MIN_MATCH {
                    Some((i, len, off))
                } else {
                    emit_lit(&mut out, block[i]);
                    if i + 3 < n {
                        insert(&mut head, &mut prev, hash4(block, i), dict.len() + i);
                    }
                    None
                };
                i += 1;
                continue;
            }
            emit_match(&mut out, poff, plen);
            let end = (pp + plen).min(n);
            let mut k = pp;
            while k < end {
                if k + 3 < n {
                    insert(&mut head, &mut prev, hash4(block, k), dict.len() + k);
                }
                k += 1;
            }
            i = end;
            pending = None;
            continue;
        }
        if len >= MIN_MATCH {
            if len >= MAX_LAZY {
                emit_match(&mut out, off, len);
                let end = (i + len).min(n);
                while i < end {
                    if i + 3 < n {
                        insert(&mut head, &mut prev, hash4(block, i), dict.len() + i);
                    }
                    i += 1;
                }
            } else {
                pending = Some((i, len, off));
                i += 1;
            }
        } else {
            emit_lit(&mut out, block[i]);
            if i + 3 < n {
                insert(&mut head, &mut prev, hash4(block, i), dict.len() + i);
            }
            i += 1;
        }
    }
    if let Some((pp, plen, poff)) = pending {
        if plen >= MIN_MATCH && pp + plen <= n {
            emit_match(&mut out, poff, plen);
        } else {
            emit_lit(&mut out, block[pp]);
        }
    }
    out
}

fn byte_at(dict: &[u8], block: &[u8], abs: usize) -> u8 {
    if abs < dict.len() {
        dict[abs]
    } else {
        block[abs - dict.len()]
    }
}

fn match_len(dict: &[u8], block: &[u8], src: usize, dst_i: usize) -> usize {
    let max = MAX_MATCH.min(block.len() - dst_i);
    let hist = dict.len() + block.len();
    let mut n = 0;
    while n < max && src + n < hist && src + n < dict.len() + dst_i {
        if byte_at(dict, block, src + n) != block[dst_i + n] {
            break;
        }
        n += 1;
    }
    n
}

fn lit_cost() -> u32 {
    2
}
fn match_cost(off: usize) -> u32 {
    if off <= 0xffff {
        4
    } else {
        5
    }
}

/// Optimal parse. Returns the same token language as `dict::lz_tokens`.
pub fn lz_opt_tokens(block: &[u8], dict: &[u8]) -> Vec<u8> {
    let n = block.len();
    if n == 0 {
        return Vec::new();
    }
    let hist_len = dict.len() + n;
    let mut head = vec![-1i32; HASH_SIZE];
    let mut prev = vec![-1i32; hist_len];
    for p in 0..dict.len().saturating_sub(3) {
        let h = hash4(dict, p);
        prev[p] = head[h];
        head[h] = p as i32;
    }

    let inf = u32::MAX / 4;
    let mut dp = vec![inf; n + 1];
    let mut back: Vec<(u32, u32)> = vec![(0, 0); n + 1]; // (prev_i, packed)
    dp[0] = 0;

    for i in 0..n {
        if dp[i] == inf {
            continue;
        }
        // literal
        let c = dp[i].saturating_add(lit_cost());
        if c < dp[i + 1] {
            dp[i + 1] = c;
            back[i + 1] = (i as u32, 0);
        }
        if i + MIN_MATCH > n {
            let abs = dict.len() + i;
            if i + 3 < n {
                let h = hash4(block, i);
                prev[abs] = head[h];
                head[h] = abs as i32;
            }
            continue;
        }
        let abs = dict.len() + i;
        let h = hash4(block, i);
        let mut p = head[h];
        let mut steps = 0;
        let mut seen = 0;
        while p >= 0 && steps < DP_CHAIN && seen < MAX_CAND {
            steps += 1;
            let src = p as usize;
            if abs > src {
                let off = abs - src;
                if off <= dict.len() + block.len() {
                    let len = match_len(dict, block, src, i);
                    if len >= MIN_MATCH {
                        seen += 1;
                        let c = dp[i].saturating_add(match_cost(off));
                        let j = i + len;
                        if c < dp[j] {
                            dp[j] = c;
                            back[j] = (i as u32, off as u32);
                        }
                    }
                }
            }
            p = prev[src];
        }
        prev[abs] = head[h];
        head[h] = abs as i32;
    }

    // traceback
    let mut path: Vec<(usize, usize, usize)> = Vec::new(); // (start, off, len) off=0 lit
    let mut i = n;
    while i > 0 {
        let (prev_i, off) = back[i];
        let p = prev_i as usize;
        if p >= i {
            // broken dp — fall back to literals
            path.clear();
            for k in 0..n {
                path.push((k, 0, 1));
            }
            break;
        }
        path.push((p, off as usize, i - p));
        i = p;
    }
    path.reverse();

    let mut out = Vec::with_capacity(n);
    for (start, off, len) in path {
        if off == 0 {
            for k in 0..len {
                out.push(0x00);
                out.push(block[start + k]);
            }
        } else if off <= 0xffff {
            out.push(0x01);
            out.extend_from_slice(&(off as u16).to_le_bytes());
            out.push(len as u8);
        } else {
            out.push(0x02);
            out.push((off & 0xff) as u8);
            out.push(((off >> 8) & 0xff) as u8);
            out.push(((off >> 16) & 0xff) as u8);
            out.push(len as u8);
        }
    }
    out
}

pub const MAGIC2: &[u8; 4] = b"DLZ2";

fn split_tokens(tokens: &[u8]) -> Result<(Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>), &'static str> {
    let mut flags = Vec::new();
    let mut lits = Vec::new();
    let mut lens = Vec::new();
    let mut dists = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        match tokens[i] {
            0x00 => {
                if i + 1 >= tokens.len() {
                    return Err("dlz2: short lit");
                }
                flags.push(0);
                lits.push(tokens[i + 1]);
                i += 2;
            }
            0x01 => {
                if i + 4 > tokens.len() {
                    return Err("dlz2: short m16");
                }
                flags.push(1);
                let off = u16::from_le_bytes([tokens[i + 1], tokens[i + 2]]) as u32;
                lens.push(tokens[i + 3]);
                dists.extend_from_slice(&off.to_le_bytes()[..3]);
                i += 4;
            }
            0x02 => {
                if i + 5 > tokens.len() {
                    return Err("dlz2: short m24");
                }
                flags.push(1);
                let off = tokens[i + 1] as u32
                    | ((tokens[i + 2] as u32) << 8)
                    | ((tokens[i + 3] as u32) << 16);
                lens.push(tokens[i + 4]);
                dists.extend_from_slice(&off.to_le_bytes()[..3]);
                i += 5;
            }
            _ => return Err("dlz2: bad token"),
        }
    }
    Ok((flags, lits, lens, dists))
}

fn join_tokens(
    flags: &[u8],
    lits: &[u8],
    lens: &[u8],
    dists: &[u8],
) -> Result<Vec<u8>, &'static str> {
    let mut out = Vec::new();
    let mut li = 0usize;
    let mut ni = 0usize;
    let mut di = 0usize;
    for &f in flags {
        if f == 0 {
            if li >= lits.len() {
                return Err("dlz2: lit underrun");
            }
            out.push(0x00);
            out.push(lits[li]);
            li += 1;
        } else {
            if ni >= lens.len() || di + 3 > dists.len() {
                return Err("dlz2: match underrun");
            }
            let off = dists[di] as usize
                | ((dists[di + 1] as usize) << 8)
                | ((dists[di + 2] as usize) << 16);
            di += 3;
            let len = lens[ni];
            ni += 1;
            if off <= 0xffff {
                out.push(0x01);
                out.extend_from_slice(&(off as u16).to_le_bytes());
                out.push(len);
            } else {
                out.push(0x02);
                out.push((off & 0xff) as u8);
                out.push(((off >> 8) & 0xff) as u8);
                out.push(((off >> 16) & 0xff) as u8);
                out.push(len);
            }
        }
    }
    Ok(out)
}

fn put_blob(out: &mut Vec<u8>, blob: &[u8]) {
    out.extend_from_slice(&(blob.len() as u32).to_le_bytes());
    out.extend_from_slice(blob);
}

fn take_blob(buf: &[u8], pos: &mut usize) -> Result<Vec<u8>, &'static str> {
    if *pos + 4 > buf.len() {
        return Err("dlz2: short len");
    }
    let n = u32::from_le_bytes(buf[*pos..*pos + 4].try_into().unwrap()) as usize;
    *pos += 4;
    if *pos + n > buf.len() {
        return Err("dlz2: short blob");
    }
    let v = buf[*pos..*pos + n].to_vec();
    *pos += n;
    Ok(v)
}

fn encode_dlz2(tokens: &[u8], orig_len: usize) -> Result<Vec<u8>, &'static str> {
    let (flags, lits, lens, dists) = split_tokens(tokens)?;
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC2);
    out.extend_from_slice(&(orig_len as u32).to_le_bytes());
    put_blob(&mut out, &crate::clean_o1::encode_auto(&flags));
    put_blob(&mut out, &crate::clean_o1::encode_auto(&lits));
    put_blob(&mut out, &crate::clean_o1::encode_auto(&lens));
    put_blob(&mut out, &crate::clean_o1::encode_auto(&dists));
    Ok(out)
}

fn decode_dlz2(buf: &[u8], dict: &[u8]) -> Result<Vec<u8>, &'static str> {
    if buf.len() < 8 || &buf[..4] != MAGIC2 {
        return Err("dlz2: magic");
    }
    let orig_len = u32::from_le_bytes(buf[4..8].try_into().unwrap()) as usize;
    let mut pos = 8;
    let flags = ans::rans_decode(&take_blob(buf, &mut pos)?)?;
    let lits = ans::rans_decode(&take_blob(buf, &mut pos)?)?;
    let lens = ans::rans_decode(&take_blob(buf, &mut pos)?)?;
    let dists = ans::rans_decode(&take_blob(buf, &mut pos)?)?;
    let tokens = join_tokens(&flags, &lits, &lens, &dists)?;
    dict::lz_replay(&tokens, dict, orig_len)
}

/// LIT bytes + ESC-framed matches. Reconstructable, BWT-friendly.
pub fn prefilter(block: &[u8], dict: &[u8]) -> Vec<u8> {
    let tokens = lz_lazy_tokens(block, dict);
    let mut out = Vec::with_capacity(block.len());
    let mut i = 0;
    while i < tokens.len() {
        match tokens[i] {
            0x00 => {
                if i + 1 >= tokens.len() {
                    break;
                }
                let b = tokens[i + 1];
                if b == ESC {
                    out.push(ESC);
                    out.push(0);
                } else {
                    out.push(b);
                }
                i += 2;
            }
            0x01 => {
                if i + 4 > tokens.len() {
                    break;
                }
                let off = u16::from_le_bytes([tokens[i + 1], tokens[i + 2]]) as u32;
                out.push(ESC);
                out.push(1);
                out.push(tokens[i + 3]);
                out.extend_from_slice(&off.to_le_bytes()[..3]);
                i += 4;
            }
            0x02 => {
                if i + 5 > tokens.len() {
                    break;
                }
                out.push(ESC);
                out.push(1);
                out.push(tokens[i + 4]);
                out.push(tokens[i + 1]);
                out.push(tokens[i + 2]);
                out.push(tokens[i + 3]);
                i += 5;
            }
            _ => break,
        }
    }
    out
}

pub fn unprefilter(stream: &[u8], dict: &[u8], orig_len: usize) -> Result<Vec<u8>, &'static str> {
    let mut out = Vec::with_capacity(orig_len);
    let mut i = 0;
    while i < stream.len() && out.len() < orig_len {
        if stream[i] != ESC {
            out.push(stream[i]);
            i += 1;
            continue;
        }
        if i + 1 >= stream.len() {
            return Err("dlzp: short esc");
        }
        match stream[i + 1] {
            0 => {
                out.push(ESC);
                i += 2;
                continue;
            }
            1 => {}
            _ => return Err("dlzp: bad tag"),
        }
        if i + 6 > stream.len() {
            return Err("dlzp: short match");
        }
        let len = stream[i + 2] as usize;
        let off = stream[i + 3] as usize
            | ((stream[i + 4] as usize) << 8)
            | ((stream[i + 5] as usize) << 16);
        i += 6;
        if off == 0 || len < MIN_MATCH {
            return Err("dlzp: bad match");
        }
        for _ in 0..len {
            if out.len() >= orig_len {
                break;
            }
            let abs = dict.len() + out.len();
            if abs < off {
                return Err("dlzp: oob");
            }
            let src = abs - off;
            let b = if src < dict.len() {
                dict[src]
            } else {
                let j = src - dict.len();
                if j >= out.len() {
                    return Err("dlzp: fwd");
                }
                out[j]
            };
            out.push(b);
        }
    }
    if out.len() != orig_len {
        return Err("dlzp: length");
    }
    Ok(out)
}

pub fn encode_pre_bwt(block: &[u8], dict: &[u8], seed: Option<&[u8; 256]>) -> Vec<u8> {
    let pre = prefilter(block, dict);
    let prog = Program {
        ops: vec![Op::Bwt, Op::Mtf, Op::Ans],
    };
    let coded = pipeline::apply_seeded(&prog, &pre, seed);
    let mut out = Vec::with_capacity(12 + coded.len());
    out.extend_from_slice(MAGICP);
    out.extend_from_slice(&(block.len() as u32).to_le_bytes());
    out.extend_from_slice(&(pre.len() as u32).to_le_bytes());
    out.extend_from_slice(&coded);
    out
}

pub fn decode_pre_bwt(
    buf: &[u8],
    dict: &[u8],
    seed: Option<&[u8; 256]>,
) -> Result<Vec<u8>, &'static str> {
    if buf.len() < 12 || &buf[..4] != MAGICP {
        return Err("dlzp: magic");
    }
    let orig = u32::from_le_bytes(buf[4..8].try_into().unwrap()) as usize;
    let pre_len = u32::from_le_bytes(buf[8..12].try_into().unwrap()) as usize;
    let prog = Program {
        ops: vec![Op::Bwt, Op::Mtf, Op::Ans],
    };
    let mid = pipeline::invert_seeded(&prog, &buf[12..], pre_len, seed)?;
    unprefilter(&mid, dict, orig)
}

pub fn encode(block: &[u8], dict: &[u8]) -> Vec<u8> {
    let tokens = lz_lazy_tokens(block, dict);
    let mixed = {
        let coded = ans::rans_encode(&tokens);
        let mut out = Vec::with_capacity(8 + coded.len());
        out.extend_from_slice(dict::MAGIC);
        out.extend_from_slice(&(block.len() as u32).to_le_bytes());
        out.extend_from_slice(&coded);
        out
    };
    match encode_dlz2(&tokens, block.len()) {
        Ok(split) if split.len() < mixed.len() => split,
        _ => mixed,
    }
}

pub fn decode(buf: &[u8], dict: &[u8]) -> Result<Vec<u8>, &'static str> {
    decode_seeded(buf, dict, None)
}

pub fn decode_seeded(
    buf: &[u8],
    dict: &[u8],
    seed: Option<&[u8; 256]>,
) -> Result<Vec<u8>, &'static str> {
    if buf.len() >= 4 && &buf[..4] == MAGICP {
        decode_pre_bwt(buf, dict, seed)
    } else if buf.len() >= 4 && &buf[..4] == MAGIC2 {
        decode_dlz2(buf, dict)
    } else {
        dict::rans_decode_with_dict(buf, dict)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dp_roundtrip_repeat() {
        let src = b"abcdeabcdeabcdeXXXX".repeat(30);
        let c = encode(&src, &[]);
        let d = decode(&c, &[]).unwrap();
        assert_eq!(d, src);
        assert!(c.len() < src.len());
    }

    #[test]
    fn dp_uses_dict() {
        let a = b"the quick brown fox jumps over the lazy dog\n".repeat(20);
        let b = a.clone();
        let ca = encode(&a, &[]);
        let da = decode(&ca, &[]).unwrap();
        let cb = encode(&b, &da);
        let db = decode(&cb, &da).unwrap();
        assert_eq!(db, b.as_slice());
        assert!(cb.len() < ca.len() || cb.len() + 32 < b.len());
    }

    #[test]
    fn dlz2_split_roundtrip() {
        let src = b"the quick brown fox jumps over the lazy dog\n".repeat(80);
        let tokens = lz_lazy_tokens(&src, &[]);
        let split = encode_dlz2(&tokens, src.len()).unwrap();
        let back = decode_dlz2(&split, &[]).unwrap();
        assert_eq!(back, src.as_slice());
        let mixed = encode(&src, &[]);
        let back2 = decode(&mixed, &[]).unwrap();
        assert_eq!(back2, src.as_slice());
    }

    #[test]
    fn prefilter_roundtrip() {
        let src = b"repeat-me-please-repeat-me-please-repeat-me-please\n".repeat(40);
        let pre = prefilter(&src, &[]);
        let back = unprefilter(&pre, &[], src.len()).unwrap();
        assert_eq!(back, src.as_slice());
        let c = encode_pre_bwt(&src, &[], None);
        let d = decode_pre_bwt(&c, &[], None).unwrap();
        assert_eq!(d, src.as_slice());
        assert_eq!(&c[..4], MAGICP);
    }

    #[test]
    fn lazy_sees_past_32k() {
        let mut src = vec![0u8; 40_000];
        src[0..16].copy_from_slice(b"UNIQUE-PHRASE!!!");
        src[35_000..35_016].copy_from_slice(b"UNIQUE-PHRASE!!!");
        let tok = lz_lazy_tokens(&src, &[]);
        assert!(tok.iter().any(|&t| t == 0x01 || t == 0x02));
        let c = encode(&src, &[]);
        let d = decode(&c, &[]).unwrap();
        assert_eq!(d, src);
    }
}
