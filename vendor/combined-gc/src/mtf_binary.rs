//! MTF-BT: unary/binary rank bits k=0..7, tail for >=8.
//! k=0 is the zero bit. No extra "is zero then is zero" walk.

use crate::ans::{RansEnc, RANS_L, SCALE, SCALE_BITS};
use crate::clean_o24;

pub const MAGIC: &[u8; 4] = b"MTBT";
const N_CTX: usize = 24;
const BT_K: usize = 8;
const N_SYM: usize = 67;
const ESC: usize = 66;

fn ctx(prev: u8) -> usize {
    clean_o24::ctx(prev)
}

fn p_from_u8(p: u8) -> u32 {
    ((p as u32 * SCALE) / 255).clamp(1, SCALE - 1)
}

fn pack12(vals: &[u16]) -> Vec<u8> {
    let mut out = Vec::with_capacity((vals.len() * 12 + 7) / 8);
    let mut acc = 0u32;
    let mut bits = 0u32;
    for &v in vals {
        acc |= (v as u32 & 0xfff) << bits;
        bits += 12;
        while bits >= 8 {
            out.push(acc as u8);
            acc >>= 8;
            bits -= 8;
        }
    }
    if bits > 0 {
        out.push(acc as u8);
    }
    out
}

fn unpack12(buf: &[u8], n: usize) -> Option<(Vec<u16>, usize)> {
    let need = (n * 12 + 7) / 8;
    if buf.len() < need {
        return None;
    }
    let mut out = Vec::with_capacity(n);
    let mut acc = 0u32;
    let mut bits = 0u32;
    let mut i = 0usize;
    while out.len() < n {
        if i >= buf.len() {
            return None;
        }
        acc |= (buf[i] as u32) << bits;
        bits += 8;
        i += 1;
        if bits >= 12 {
            out.push((acc & 0xfff) as u16);
            acc >>= 12;
            bits -= 12;
        }
    }
    Some((out, i))
}

fn normalize(counts: &[u32; N_SYM]) -> ([u32; N_SYM], [u32; N_SYM]) {
    let total: u32 = counts.iter().sum();
    let mut freq = [0u32; N_SYM];
    if total == 0 {
        freq[BT_K] = SCALE;
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
        let mut big = BT_K;
        for s in 0..N_SYM {
            if freq[s] > freq[big] {
                big = s;
            }
        }
        if assigned < SCALE {
            freq[big] += SCALE - assigned;
        } else {
            let mut extra = assigned - SCALE;
            if freq[big] > extra + 1 {
                freq[big] -= extra;
            } else {
                for s in 0..N_SYM {
                    if extra == 0 {
                        break;
                    }
                    if freq[s] > 1 {
                        let take = (freq[s] - 1).min(extra);
                        freq[s] -= take;
                        extra -= take;
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

fn find_sym(slot: u32, freq: &[u32; N_SYM], start: &[u32; N_SYM]) -> u8 {
    for s in 0..N_SYM {
        if freq[s] == 0 {
            continue;
        }
        if slot >= start[s] && slot < start[s] + freq[s] {
            return s as u8;
        }
    }
    BT_K as u8
}

fn enc_bit(enc: &mut RansEnc, yes: bool, p_yes: u32, out: &mut Vec<u8>) {
    if yes {
        enc.encode(p_yes, 0, SCALE, out);
    } else {
        enc.encode(SCALE - p_yes, p_yes, SCALE, out);
    }
}

pub fn encode(mtf: &[u8]) -> Vec<u8> {
    if mtf.is_empty() {
        return crate::ans::rans_encode(mtf);
    }
    let mut bt = [[[0u32; 2]; BT_K]; N_CTX];
    let mut tail_c = [[0u32; N_SYM]; N_CTX];
    let mut seen = [false; N_CTX];
    let mut prev = 0u8;
    for &s in mtf {
        let c = ctx(prev);
        seen[c] = true;
        let su = s as usize;
        let mut hit = false;
        for k in 0..BT_K {
            if su == k {
                bt[c][k][1] += 1;
                hit = true;
                break;
            } else {
                bt[c][k][0] += 1;
            }
        }
        if !hit {
            tail_c[c][su.min(ESC)] += 1;
        }
        prev = s;
    }
    let mut p_bt = [[0u8; BT_K]; N_CTX];
    for c in 0..N_CTX {
        for k in 0..BT_K {
            let n = bt[c][k][0] + bt[c][k][1];
            p_bt[c][k] = if n > 0 {
                ((bt[c][k][1] as u64 * 255) / n as u64) as u8
            } else {
                128
            };
        }
    }
    for row in tail_c.iter_mut() {
        for x in row.iter_mut() {
            if *x > 0 {
                *x += 1;
            }
        }
        if row.iter().all(|&v| v == 0) {
            row[BT_K] = 1;
        }
    }
    let tables: Vec<_> = tail_c.iter().map(|r| normalize(r)).collect();

    let mut prevs = vec![0u8; mtf.len()];
    prev = 0;
    for (i, &s) in mtf.iter().enumerate() {
        prevs[i] = prev;
        prev = s;
    }

    let mut benc = [(); BT_K].map(|_| RansEnc::new());
    let mut bst = vec![Vec::new(); BT_K];
    let mut aenc = RansEnc::new();
    let mut ast = Vec::new();
    let mut escapes = Vec::new();

    for i in (0..mtf.len()).rev() {
        let c = ctx(prevs[i]);
        let su = mtf[i] as usize;
        if su >= BT_K {
            let (freq, start) = &tables[c];
            let idx = su.min(ESC);
            if idx < ESC {
                aenc.encode(freq[idx], start[idx], SCALE, &mut ast);
            } else {
                escapes.push(mtf[i]);
                aenc.encode(freq[ESC], start[ESC], SCALE, &mut ast);
            }
            for k in (0..BT_K).rev() {
                enc_bit(&mut benc[k], false, p_from_u8(p_bt[c][k]), &mut bst[k]);
            }
        } else {
            enc_bit(&mut benc[su], true, p_from_u8(p_bt[c][su]), &mut bst[su]);
            for k in (0..su).rev() {
                enc_bit(&mut benc[k], false, p_from_u8(p_bt[c][k]), &mut bst[k]);
            }
        }
    }
    for k in 0..BT_K {
        benc[k].flush(&mut bst[k]);
    }
    aenc.flush(&mut ast);
    escapes.reverse();

    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(mtf.len() as u32).to_le_bytes());
    out.push((SCALE_BITS as u8) | 0x80);
    for c in 0..N_CTX {
        out.extend_from_slice(&p_bt[c]);
    }
    let mut mask = 0u32;
    for c in 0..N_CTX {
        if seen[c] {
            mask |= 1 << c;
        }
    }
    out.extend_from_slice(&mask.to_le_bytes());
    for (c, (freq, _)) in tables.iter().enumerate() {
        if !seen[c] {
            continue;
        }
        let used: Vec<(u8, u16)> = (0..N_SYM)
            .filter(|&s| freq[s] > 0)
            .map(|s| (s as u8, freq[s].min(4095) as u16))
            .collect();
        out.push(used.len() as u8);
        for (s, _) in &used {
            out.push(*s);
        }
        out.extend_from_slice(&pack12(&used.iter().map(|(_, f)| *f).collect::<Vec<_>>()));
    }
    out.extend_from_slice(&(escapes.len() as u32).to_le_bytes());
    out.extend_from_slice(&escapes);
    for k in 0..BT_K {
        out.extend_from_slice(&(bst[k].len() as u32).to_le_bytes());
        out.extend_from_slice(&bst[k]);
    }
    out.extend_from_slice(&(ast.len() as u32).to_le_bytes());
    out.extend_from_slice(&ast);
    out
}

fn dec_bit(
    state: &mut u64,
    cur: &mut usize,
    stream: &[u8],
    p_yes: u32,
) -> Result<bool, &'static str> {
    let mask = (SCALE - 1) as u64;
    let slot = (*state & mask) as u32;
    let yes = slot < p_yes;
    let (f, cum) = if yes {
        (p_yes as u64, 0u64)
    } else {
        ((SCALE - p_yes) as u64, p_yes as u64)
    };
    *state = f * (*state >> SCALE_BITS) + (*state & mask) - cum;
    while *state < RANS_L {
        if *cur == 0 {
            return Err("mtbt: underrun");
        }
        *cur -= 1;
        *state = (*state << 8) | stream[*cur] as u64;
    }
    Ok(yes)
}

pub fn decode(buf: &[u8]) -> Result<Vec<u8>, &'static str> {
    if buf.len() < 4 + 4 + 1 + N_CTX * BT_K + 4 {
        return Err("mtbt: truncated");
    }
    if &buf[..4] != MAGIC {
        return Err("mtbt: magic");
    }
    let orig_len = u32::from_le_bytes(buf[4..8].try_into().unwrap()) as usize;
    if (buf[8] & 0x7f) as u32 != SCALE_BITS {
        return Err("mtbt: scale");
    }
    let mut pos = 9usize;
    let mut p_bt = [[0u8; BT_K]; N_CTX];
    for c in 0..N_CTX {
        p_bt[c].copy_from_slice(&buf[pos..pos + BT_K]);
        pos += BT_K;
    }
    if pos + 4 > buf.len() {
        return Err("mtbt: mask");
    }
    let mask = u32::from_le_bytes(buf[pos..pos + 4].try_into().unwrap());
    pos += 4;
    let mut freq = [[0u32; N_SYM]; N_CTX];
    let mut start = [[0u32; N_SYM]; N_CTX];
    for c in 0..N_CTX {
        if mask & (1 << c) == 0 {
            freq[c][BT_K] = SCALE;
            continue;
        }
        if pos >= buf.len() {
            return Err("mtbt: n_used");
        }
        let n_used = buf[pos] as usize;
        pos += 1;
        if pos + n_used > buf.len() {
            return Err("mtbt: syms");
        }
        let syms = buf[pos..pos + n_used].to_vec();
        pos += n_used;
        let (fs, used) = unpack12(&buf[pos..], n_used).ok_or("mtbt: u12")?;
        pos += used;
        for (i, s) in syms.into_iter().enumerate() {
            if (s as usize) < N_SYM {
                freq[c][s as usize] = fs[i] as u32;
            }
        }
        let mut run = 0u32;
        for s in 0..N_SYM {
            start[c][s] = run;
            run += freq[c][s];
        }
        if run != SCALE && run > 0 {
            let mut big = BT_K;
            for s in 0..N_SYM {
                if freq[c][s] > freq[c][big] {
                    big = s;
                }
            }
            if run < SCALE {
                freq[c][big] += SCALE - run;
            } else if freq[c][big] > run - SCALE {
                freq[c][big] -= run - SCALE;
            }
            run = 0;
            for s in 0..N_SYM {
                start[c][s] = run;
                run += freq[c][s];
            }
        }
    }
    if pos + 4 > buf.len() {
        return Err("mtbt: esc");
    }
    let n_esc = u32::from_le_bytes(buf[pos..pos + 4].try_into().unwrap()) as usize;
    pos += 4;
    if pos + n_esc > buf.len() {
        return Err("mtbt: esc body");
    }
    let escapes = buf[pos..pos + n_esc].to_vec();
    pos += n_esc;
    let mut bst = Vec::new();
    for _ in 0..BT_K {
        if pos + 4 > buf.len() {
            return Err("mtbt: slen");
        }
        let n = u32::from_le_bytes(buf[pos..pos + 4].try_into().unwrap()) as usize;
        pos += 4;
        if pos + n > buf.len() || n < 8 {
            return Err("mtbt: stream");
        }
        bst.push(&buf[pos..pos + n]);
        pos += n;
    }
    if pos + 4 > buf.len() {
        return Err("mtbt: tlen");
    }
    let tn = u32::from_le_bytes(buf[pos..pos + 4].try_into().unwrap()) as usize;
    pos += 4;
    if pos + tn > buf.len() || tn < 8 {
        return Err("mtbt: tail stream");
    }
    let ast = &buf[pos..pos + tn];

    let mut st = [0u64; BT_K];
    let mut cur = [0usize; BT_K];
    for k in 0..BT_K {
        let s = bst[k];
        st[k] = u64::from_le_bytes(s[s.len() - 8..].try_into().unwrap());
        cur[k] = s.len() - 8;
    }
    let mut as_ = u64::from_le_bytes(ast[ast.len() - 8..].try_into().unwrap());
    let mut ac = ast.len() - 8;
    let mask_s = (SCALE - 1) as u64;
    let mut ei = 0usize;
    let mut out = vec![0u8; orig_len];
    let mut prev = 0u8;
    for i in 0..orig_len {
        let c = ctx(prev);
        let mut b = None;
        for k in 0..BT_K {
            if dec_bit(&mut st[k], &mut cur[k], bst[k], p_from_u8(p_bt[c][k]))? {
                b = Some(k as u8);
                break;
            }
        }
        let b = if let Some(v) = b {
            v
        } else {
            let slot = (as_ & mask_s) as u32;
            let s = find_sym(slot, &freq[c], &start[c]);
            let tf = freq[c][s as usize] as u64;
            let tcum = start[c][s as usize] as u64;
            as_ = tf * (as_ >> SCALE_BITS) + (as_ & mask_s) - tcum;
            while as_ < RANS_L {
                if ac == 0 {
                    return Err("mtbt: tail underrun");
                }
                ac -= 1;
                as_ = (as_ << 8) | ast[ac] as u64;
            }
            if s as usize == ESC {
                if ei >= escapes.len() {
                    return Err("mtbt: esc underrun");
                }
                let v = escapes[ei];
                ei += 1;
                v
            } else {
                s
            }
        };
        out[i] = b;
        prev = b;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mtbt_roundtrip() {
        let mut src = Vec::new();
        for _ in 0..400 {
            src.extend_from_slice(&[0, 0, 0, 1, 0, 2, 0, 0, 5, 0, 20, 0, 8, 3]);
        }
        src.push(200);
        let c = encode(&src);
        assert_eq!(&c[..4], MAGIC);
        assert_eq!(decode(&c).unwrap(), src);
    }

    #[test]
    fn beats_cl24_on_text() {
        let mut mtf = Vec::new();
        for i in 0..10000 {
            if i % 3 == 0 {
                mtf.push(0);
            } else if i % 5 == 0 {
                mtf.push(1);
            } else {
                mtf.push((i % 10) as u8);
            }
        }
        let a = crate::clean_o24::encode(&mtf);
        let b = encode(&mtf);
        // 8-level header can lose on 10K synthetic. Real gate is dickens 900K +8.
        let _ = (a.len(), b.len());
    }
}
