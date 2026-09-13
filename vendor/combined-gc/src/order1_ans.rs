//! 4-class order-1 rANS for MTF-RLE0 streams.
//! Contexts: 0 / 1 / 2-3 / 4+. Alphabet 0..65 + escape 66.

use crate::ans::{RansEnc, RANS_L, SCALE, SCALE_BITS};

pub const MAGIC4: &[u8; 4] = b"AN4\0";
const N_CTX: usize = 4;
const N_SYM: usize = 67; // 0..65 + escape
const ESC: usize = 66;

pub fn ctx(prev: u8) -> usize {
    match prev {
        0 => 0,
        1 => 1,
        2 | 3 => 2,
        _ => 3,
    }
}

fn normalize67(counts: &[u32; N_SYM]) -> ([u32; N_SYM], [u32; N_SYM]) {
    let total: u32 = counts.iter().sum();
    let mut freq = [0u32; N_SYM];
    if total == 0 {
        freq[0] = SCALE;
        return (freq, [0u32; N_SYM]);
    }
    let mut assigned = 0u32;
    for (s, &c) in counts.iter().enumerate() {
        if c == 0 {
            continue;
        }
        let f = ((c as u64 * SCALE as u64) / total as u64).max(1) as u32;
        freq[s] = f;
        assigned += f;
    }
    if assigned != SCALE {
        let mut big = 0usize;
        for s in 0..N_SYM {
            if freq[s] > freq[big] {
                big = s;
            }
        }
        if assigned < SCALE {
            freq[big] += SCALE - assigned;
        } else {
            let extra = assigned - SCALE;
            if freq[big] > extra + 1 {
                freq[big] -= extra;
            } else {
                let mut left = extra;
                for s in 0..N_SYM {
                    if left == 0 {
                        break;
                    }
                    if freq[s] > 1 {
                        let take = (freq[s] - 1).min(left);
                        freq[s] -= take;
                        left -= take;
                    }
                }
            }
        }
    }
    let mut start = [0u32; N_SYM];
    let mut run = 0u32;
    for s in 0..N_SYM {
        start[s] = run;
        run += freq[s];
    }
    (freq, start)
}

fn find_sym67(slot: u32, freq: &[u32; N_SYM], start: &[u32; N_SYM]) -> u8 {
    for s in 0..N_SYM {
        if freq[s] == 0 {
            continue;
        }
        if slot >= start[s] && slot < start[s] + freq[s] {
            return s as u8;
        }
    }
    0
}

pub fn encode(data: &[u8]) -> Vec<u8> {
    if data.is_empty() {
        return crate::ans::rans_encode(data);
    }
    let mut counts = [[0u32; N_SYM]; N_CTX];
    let mut prev = 0u8;
    let mut n_esc = 0u32;
    for &b in data {
        let c = ctx(prev);
        if (b as usize) < ESC {
            counts[c][b as usize] += 1;
            prev = b;
        } else {
            counts[c][ESC] += 1;
            n_esc += 1;
            prev = b;
        }
    }
    for row in counts.iter_mut() {
        for c in row.iter_mut() {
            if *c > 0 {
                *c += 1;
            }
        }
        if row.iter().all(|&c| c == 0) {
            row[0] = 1;
        }
    }
    let tables: Vec<_> = counts.iter().map(|row| normalize67(row)).collect();

    let mut prevs = vec![0u8; data.len()];
    prev = 0;
    for (i, &b) in data.iter().enumerate() {
        prevs[i] = prev;
        prev = b;
    }

    let mut enc = RansEnc::new();
    let mut stream = Vec::with_capacity(data.len() / 2 + 16);
    let mut escapes = Vec::with_capacity(n_esc as usize);
    for i in (0..data.len()).rev() {
        let c = ctx(prevs[i]);
        let b = data[i];
        let (freq, start) = &tables[c];
        if (b as usize) < ESC {
            enc.encode(freq[b as usize], start[b as usize], SCALE, &mut stream);
        } else {
            escapes.push(b);
            enc.encode(freq[ESC], start[ESC], SCALE, &mut stream);
        }
    }
    enc.flush(&mut stream);
    escapes.reverse();

    let mut out = Vec::new();
    out.extend_from_slice(MAGIC4);
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.push(SCALE_BITS as u8);
    out.push(N_CTX as u8);
    out.push(N_SYM as u8);
    for (freq, _) in &tables {
        let used: Vec<(u8, u16)> = (0..N_SYM)
            .filter(|&s| freq[s] > 0)
            .map(|s| (s as u8, freq[s] as u16))
            .collect();
        out.push(used.len() as u8);
        for (s, f) in used {
            out.push(s);
            out.extend_from_slice(&f.to_le_bytes());
        }
    }
    out.extend_from_slice(&(escapes.len() as u32).to_le_bytes());
    out.extend_from_slice(&escapes);
    out.extend_from_slice(&(stream.len() as u32).to_le_bytes());
    out.extend_from_slice(&stream);
    out
}

pub fn decode(buf: &[u8]) -> Result<Vec<u8>, &'static str> {
    if buf.len() < 4 + 4 + 3 + 8 {
        return Err("an4: truncated");
    }
    if &buf[..4] != MAGIC4 {
        return Err("an4: magic");
    }
    let orig_len = u32::from_le_bytes(buf[4..8].try_into().unwrap()) as usize;
    if buf[8] as u32 != SCALE_BITS || buf[9] as usize != N_CTX || buf[10] as usize != N_SYM {
        return Err("an4: header");
    }
    let mut pos = 11usize;
    let mut freq = [[0u32; N_SYM]; N_CTX];
    let mut start = [[0u32; N_SYM]; N_CTX];
    for c in 0..N_CTX {
        if pos >= buf.len() {
            return Err("an4: n_used");
        }
        let n_used = buf[pos] as usize;
        pos += 1;
        for _ in 0..n_used {
            if pos + 3 > buf.len() {
                return Err("an4: freq");
            }
            let s = buf[pos] as usize;
            let f = u16::from_le_bytes(buf[pos + 1..pos + 3].try_into().unwrap()) as u32;
            if s < N_SYM {
                freq[c][s] = f;
            }
            pos += 3;
        }
        let mut run = 0u32;
        for s in 0..N_SYM {
            start[c][s] = run;
            run += freq[c][s];
        }
        if run != SCALE {
            return Err("an4: sum");
        }
    }
    if pos + 4 > buf.len() {
        return Err("an4: esc len");
    }
    let n_esc = u32::from_le_bytes(buf[pos..pos + 4].try_into().unwrap()) as usize;
    pos += 4;
    if pos + n_esc + 4 > buf.len() {
        return Err("an4: esc");
    }
    let escapes = buf[pos..pos + n_esc].to_vec();
    pos += n_esc;
    let slen = u32::from_le_bytes(buf[pos..pos + 4].try_into().unwrap()) as usize;
    pos += 4;
    if pos + slen > buf.len() {
        return Err("an4: stream");
    }
    let stream = &buf[pos..pos + slen];
    if slen < 8 {
        return Err("an4: state");
    }
    let mut state = u64::from_le_bytes(stream[slen - 8..].try_into().unwrap());
    let mut cursor = slen - 8;
    let mask = (SCALE - 1) as u64;
    let mut out = vec![0u8; orig_len];
    let mut prev = 0u8;
    let mut ei = 0usize;
    for i in 0..orig_len {
        let c = ctx(prev);
        let slot = (state & mask) as u32;
        let s = find_sym67(slot, &freq[c], &start[c]);
        let f = freq[c][s as usize] as u64;
        let cum = start[c][s as usize] as u64;
        state = f * (state >> SCALE_BITS) + (state & mask) - cum;
        while state < RANS_L {
            if cursor == 0 {
                return Err("an4: underrun");
            }
            cursor -= 1;
            state = (state << 8) | stream[cursor] as u64;
        }
        let byte = if s as usize == ESC {
            if ei >= escapes.len() {
                return Err("an4: esc underrun");
            }
            let b = escapes[ei];
            ei += 1;
            b
        } else {
            s
        };
        out[i] = byte;
        prev = byte;
    }
    Ok(out)
}

pub fn encode_auto(data: &[u8]) -> Vec<u8> {
    let a0 = crate::ans::rans_encode(data);
    if data.len() < 128 {
        return a0;
    }
    let a4 = encode(data);
    if a4.len() < a0.len() {
        a4
    } else {
        a0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_ctx_roundtrip() {
        let mut src = Vec::new();
        for _ in 0..800 {
            src.extend_from_slice(&[0, 0, 0, 1, 0, 0, 2, 0, 3, 0, 5, 0, 0]);
        }
        src.push(200);
        src.push(0);
        let c = encode(&src);
        assert_eq!(decode(&c).unwrap(), src);
        let _ = crate::ans::rans_encode(&src);
    }
}
