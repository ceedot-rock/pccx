//! 1MB decoded-history LZ + order-0 rANS.
//!
//! Matches may land in the previous decoded bytes or in the current
//! block prefix. Encoder and decoder rebuild the dict the same way.

use crate::ans;

pub const DICT_CAP: usize = 1_000_000;
/// Large binary (mozilla-class tar/exe). 24-bit match off still fits.
pub const DICT_CAP_LONG: usize = 8_000_000;
pub const MIN_MATCH: usize = 4;
pub const MAX_MATCH: usize = 255;
const HASH_BITS: usize = 16;
const HASH_SIZE: usize = 1 << HASH_BITS;
const CHAIN: usize = 24;

const LIT: u8 = 0x00;
const MATCH16: u8 = 0x01;
const MATCH24: u8 = 0x02;

fn hash4(b: &[u8], i: usize) -> usize {
    let x = u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
    ((x.wrapping_mul(0x1e35a7bd)) >> (32 - HASH_BITS)) as usize
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

fn emit_match(out: &mut Vec<u8>, off: usize, len: usize) {
    if off <= 0xffff {
        out.push(MATCH16);
        out.extend_from_slice(&(off as u16).to_le_bytes());
        out.push(len as u8);
    } else {
        out.push(MATCH24);
        out.push((off & 0xff) as u8);
        out.push(((off >> 8) & 0xff) as u8);
        out.push(((off >> 16) & 0xff) as u8);
        out.push(len as u8);
    }
}

pub fn lz_tokens(block: &[u8], dict: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(block.len());
    if block.is_empty() {
        return out;
    }
    let hist_len = dict.len() + block.len();
    let mut head = vec![-1i32; HASH_SIZE];
    let mut prev = vec![-1i32; hist_len];
    for p in 0..dict.len().saturating_sub(3) {
        let h = hash4(dict, p);
        prev[p] = head[h];
        head[h] = p as i32;
    }
    let mut i = 0usize;
    while i < block.len() {
        let abs = dict.len() + i;
        let mut best_len = 0usize;
        let mut best_off = 0usize;
        if i + MIN_MATCH <= block.len() {
            let h = hash4(block, i);
            let mut p = head[h];
            let mut steps = 0;
            while p >= 0 && steps < CHAIN {
                steps += 1;
                let src = p as usize;
                if abs > src {
                    let off = abs - src;
                    if off <= dict.len() + block.len() {
                        let n = match_len(dict, block, src, i);
                        if n >= MIN_MATCH && n > best_len {
                            best_len = n;
                            best_off = off;
                        }
                    }
                }
                p = prev[src];
            }
            prev[abs] = head[h];
            head[h] = abs as i32;
        }
        if best_len >= MIN_MATCH {
            emit_match(&mut out, best_off, best_len);
            let end = i + best_len;
            i += 1;
            while i < end {
                if i + MIN_MATCH <= block.len() {
                    let h = hash4(block, i);
                    let abs = dict.len() + i;
                    prev[abs] = head[h];
                    head[h] = abs as i32;
                }
                i += 1;
            }
        } else {
            out.push(LIT);
            out.push(block[i]);
            i += 1;
        }
    }
    out
}

pub fn lz_replay(tokens: &[u8], dict: &[u8], orig_len: usize) -> Result<Vec<u8>, &'static str> {
    let mut out = Vec::with_capacity(orig_len);
    let mut i = 0usize;
    while i < tokens.len() && out.len() < orig_len {
        match tokens[i] {
            LIT => {
                if i + 1 >= tokens.len() {
                    return Err("dict: short lit");
                }
                out.push(tokens[i + 1]);
                i += 2;
            }
            MATCH16 => {
                if i + 4 > tokens.len() {
                    return Err("dict: short m16");
                }
                let off = u16::from_le_bytes([tokens[i + 1], tokens[i + 2]]) as usize;
                let len = tokens[i + 3] as usize;
                i += 4;
                copy_match(&mut out, dict, off, len, orig_len)?;
            }
            MATCH24 => {
                if i + 5 > tokens.len() {
                    return Err("dict: short m24");
                }
                let off = tokens[i + 1] as usize
                    | ((tokens[i + 2] as usize) << 8)
                    | ((tokens[i + 3] as usize) << 16);
                let len = tokens[i + 4] as usize;
                i += 5;
                copy_match(&mut out, dict, off, len, orig_len)?;
            }
            _ => return Err("dict: bad token"),
        }
    }
    if out.len() != orig_len {
        return Err("dict: length");
    }
    Ok(out)
}

fn copy_match(
    out: &mut Vec<u8>,
    dict: &[u8],
    off: usize,
    len: usize,
    orig_len: usize,
) -> Result<(), &'static str> {
    if off == 0 || len == 0 {
        return Err("dict: off");
    }
    for _ in 0..len {
        if out.len() >= orig_len {
            break;
        }
        let abs = dict.len() + out.len();
        if abs < off {
            return Err("dict: oob");
        }
        let src = abs - off;
        let b = if src < dict.len() {
            dict[src]
        } else {
            let j = src - dict.len();
            if j >= out.len() {
                return Err("dict: fwd");
            }
            out[j]
        };
        out.push(b);
    }
    Ok(())
}

pub const MAGIC: &[u8; 4] = b"DLZ1";

pub fn rans_encode_with_dict(block: &[u8], dict: &[u8]) -> Vec<u8> {
    let tokens = lz_tokens(block, dict);
    let coded = ans::rans_encode(&tokens);
    let mut out = Vec::with_capacity(8 + coded.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(block.len() as u32).to_le_bytes());
    out.extend_from_slice(&coded);
    out
}

pub fn rans_decode_with_dict(buf: &[u8], dict: &[u8]) -> Result<Vec<u8>, &'static str> {
    if buf.len() < 8 {
        return Err("dict: truncated");
    }
    if &buf[..4] != MAGIC {
        return Err("dict: magic");
    }
    let orig_len = u32::from_le_bytes(buf[4..8].try_into().unwrap()) as usize;
    let tokens = ans::rans_decode(&buf[8..])?;
    lz_replay(&tokens, dict, orig_len)
}

pub fn push_dict(dict: &mut Vec<u8>, block: &[u8]) {
    push_dict_n(dict, block, DICT_CAP);
}

pub fn push_dict_n(dict: &mut Vec<u8>, block: &[u8], cap: usize) {
    dict.extend_from_slice(block);
    if dict.len() > cap {
        let drop = dict.len() - cap;
        dict.drain(0..drop);
    }
}

pub fn hist_cap_for(data: &[u8]) -> usize {
    if data.len() > 4_000_000 && !crate::analyzer::is_text_like(data) {
        DICT_CAP_LONG
    } else {
        DICT_CAP
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn within_block_repeat() {
        let src = b"abcdeabcdeabcdeXXXX".repeat(20);
        let c = rans_encode_with_dict(&src, &[]);
        let d = rans_decode_with_dict(&c, &[]).unwrap();
        assert_eq!(d, src);
        assert!(c.len() < src.len(), "dict lz should shrink repeats");
    }

    #[test]
    fn cross_block_dict() {
        let a = b"the quick brown fox jumps over the lazy dog\n".repeat(40);
        let b = b"the quick brown fox jumps over the lazy dog\n".repeat(10);
        let ca = rans_encode_with_dict(&a, &[]);
        let da = rans_decode_with_dict(&ca, &[]).unwrap();
        assert_eq!(da, a.as_slice());
        let mut dict = Vec::new();
        push_dict(&mut dict, &da);
        let cb = rans_encode_with_dict(&b, &dict);
        let db = rans_decode_with_dict(&cb, &dict).unwrap();
        assert_eq!(db, b.as_slice());
        assert!(
            cb.len() + 16 < b.len(),
            "second block should hit dict, got {} vs {}",
            cb.len(),
            b.len()
        );
    }
}
