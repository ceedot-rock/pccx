//! 24-state split: (prev==0?0:12)+(prev==1?0:6)+class, u12, skip empty ctx.

use crate::ans::{RansEnc, RANS_L, SCALE, SCALE_BITS};

pub const MAGIC: &[u8; 4] = b"CL24";
const N_CTX: usize = 24;
const N_SYM: usize = 67;
const ESC: usize = 66;

fn class(prev: u8) -> usize {
    match prev {
        0 | 1 => 0,
        2 => 1,
        3 => 2,
        4..=7 => 3,
        8..=15 => 4,
        _ => 5,
    }
}

pub fn ctx(prev: u8) -> usize {
    let a = if prev == 0 { 0 } else { 12 };
    let b = if prev == 1 { 0 } else { 6 };
    (a + b + class(prev)).min(23)
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

fn p_from_u8(p: u8) -> u32 {
    ((p as u32 * SCALE) / 255).clamp(1, SCALE - 1)
}

fn normalize67(counts: &[u32; N_SYM]) -> ([u32; N_SYM], [u32; N_SYM]) {
    let total: u32 = counts.iter().sum();
    let mut freq = [0u32; N_SYM];
    if total == 0 {
        freq[1] = SCALE;
        let mut start = [0u32; N_SYM];
        start[1] = 0;
        return (freq, start);
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
        let mut big = 1usize;
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

fn find_sym67(slot: u32, freq: &[u32; N_SYM], start: &[u32; N_SYM]) -> u8 {
    for s in 0..N_SYM {
        if freq[s] == 0 {
            continue;
        }
        if slot >= start[s] && slot < start[s] + freq[s] {
            return s as u8;
        }
    }
    1
}

pub fn encode(mtf: &[u8]) -> Vec<u8> {
    if mtf.is_empty() {
        return crate::ans::rans_encode(mtf);
    }
    let mut z_given = [[0u32; 2]; N_CTX]; // [ctx][is_zero]
    let mut tail_c = [[0u32; N_SYM]; N_CTX];
    let mut seen = [false; N_CTX];
    let mut prev = 0u8;
    for &s in mtf {
        let c = ctx(prev);
        seen[c] = true;
        if s == 0 {
            z_given[c][1] += 1;
        } else {
            z_given[c][0] += 1;
            let idx = (s as usize).min(ESC);
            tail_c[c][idx] += 1;
        }
        prev = s;
    }
    let mut p0u8 = [0u8; N_CTX];
    for c in 0..N_CTX {
        let n = z_given[c][0] + z_given[c][1];
        p0u8[c] = if n > 0 {
            ((z_given[c][1] as u64 * 255) / n as u64) as u8
        } else {
            128
        };
    }
    for row in tail_c.iter_mut() {
        for x in row.iter_mut() {
            if *x > 0 {
                *x += 1;
            }
        }
        if row.iter().all(|&v| v == 0) {
            row[1] = 1;
        }
    }
    let tables: Vec<_> = tail_c.iter().map(|r| normalize67(r)).collect();

    let mut prevs = vec![0u8; mtf.len()];
    prev = 0;
    for (i, &s) in mtf.iter().enumerate() {
        prevs[i] = prev;
        prev = s;
    }

    let mut zenc = RansEnc::new();
    let mut zstream = Vec::with_capacity(mtf.len() / 8 + 16);
    for i in (0..mtf.len()).rev() {
        let c = ctx(prevs[i]);
        let p0 = p_from_u8(p0u8[c]);
        if mtf[i] == 0 {
            zenc.encode(p0, 0, SCALE, &mut zstream);
        } else {
            zenc.encode(SCALE - p0, p0, SCALE, &mut zstream);
        }
    }
    zenc.flush(&mut zstream);

    let mut tenc = RansEnc::new();
    let mut tstream = Vec::new();
    let mut escapes = Vec::new();
    for i in (0..mtf.len()).rev() {
        if mtf[i] == 0 {
            continue;
        }
        let c = ctx(prevs[i]);
        let (freq, start) = &tables[c];
        if (mtf[i] as usize) < ESC {
            tenc.encode(freq[mtf[i] as usize], start[mtf[i] as usize], SCALE, &mut tstream);
        } else {
            escapes.push(mtf[i]);
            tenc.encode(freq[ESC], start[ESC], SCALE, &mut tstream);
        }
    }
    tenc.flush(&mut tstream);
    escapes.reverse();

    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(mtf.len() as u32).to_le_bytes());
    out.push((SCALE_BITS as u8) | 0x80);
    out.extend_from_slice(&p0u8);
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
    out.extend_from_slice(&(zstream.len() as u32).to_le_bytes());
    out.extend_from_slice(&zstream);
    out.extend_from_slice(&(tstream.len() as u32).to_le_bytes());
    out.extend_from_slice(&tstream);
    out
}

pub fn decode(buf: &[u8]) -> Result<Vec<u8>, &'static str> {
    if buf.len() < 4 + 4 + 1 + N_CTX + 8 {
        return Err("cln16: truncated");
    }
    if &buf[..4] != MAGIC {
        return Err("cln16: magic");
    }
    let orig_len = u32::from_le_bytes(buf[4..8].try_into().unwrap()) as usize;
    let packed = buf[8] & 0x80 != 0;
    if (buf[8] & 0x7f) as u32 != SCALE_BITS {
        return Err("cln16: scale");
    }
    let mut p0u8 = [0u8; N_CTX];
    p0u8.copy_from_slice(&buf[9..9 + N_CTX]);
    let mut pos = 9 + N_CTX;
    if pos + 4 > buf.len() {
        return Err("cln24: mask");
    }
    let mask = u32::from_le_bytes(buf[pos..pos + 4].try_into().unwrap());
    pos += 4;
    let mut freq = [[0u32; N_SYM]; N_CTX];
    let mut start = [[0u32; N_SYM]; N_CTX];
    for c in 0..N_CTX {
        if mask & (1 << c) == 0 {
            freq[c][1] = SCALE;
            continue;
        }
        if pos >= buf.len() {
            return Err("cln24: n_used");
        }
        let n_used = buf[pos] as usize;
        pos += 1;
        if packed {
            if pos + n_used > buf.len() {
                return Err("cln16: syms");
            }
            let syms = buf[pos..pos + n_used].to_vec();
            pos += n_used;
            let (fs, used) = unpack12(&buf[pos..], n_used).ok_or("cln16: u12")?;
            pos += used;
            for (i, s) in syms.into_iter().enumerate() {
                if (s as usize) < N_SYM {
                    freq[c][s as usize] = fs[i] as u32;
                }
            }
        } else {
            for _ in 0..n_used {
                if pos + 3 > buf.len() {
                    return Err("cln16: freq");
                }
                let s = buf[pos] as usize;
                let f = u16::from_le_bytes(buf[pos + 1..pos + 3].try_into().unwrap()) as u32;
                if s < N_SYM {
                    freq[c][s] = f;
                }
                pos += 3;
            }
        }
        let mut run = 0u32;
        for s in 0..N_SYM {
            start[c][s] = run;
            run += freq[c][s];
        }
        if run != SCALE {
            if packed && run > 0 {
                let mut big = 0usize;
                for s in 0..N_SYM {
                    if freq[c][s] > freq[c][big] {
                        big = s;
                    }
                }
                if run < SCALE {
                    freq[c][big] += SCALE - run;
                } else if freq[c][big] > run - SCALE {
                    freq[c][big] -= run - SCALE;
                } else {
                    return Err("cln16: sum");
                }
                run = 0;
                for s in 0..N_SYM {
                    start[c][s] = run;
                    run += freq[c][s];
                }
            }
            if run != SCALE {
                return Err("cln16: sum");
            }
        }
    }
    if pos + 4 > buf.len() {
        return Err("cln16: esc");
    }
    let n_esc = u32::from_le_bytes(buf[pos..pos + 4].try_into().unwrap()) as usize;
    pos += 4;
    if pos + n_esc + 8 > buf.len() {
        return Err("cln16: esc body");
    }
    let escapes = buf[pos..pos + n_esc].to_vec();
    pos += n_esc;
    let zlen = u32::from_le_bytes(buf[pos..pos + 4].try_into().unwrap()) as usize;
    pos += 4;
    if pos + zlen + 4 > buf.len() {
        return Err("cln16: zstream");
    }
    let zstream = &buf[pos..pos + zlen];
    pos += zlen;
    let tlen = u32::from_le_bytes(buf[pos..pos + 4].try_into().unwrap()) as usize;
    pos += 4;
    if pos + tlen > buf.len() {
        return Err("cln16: tstream");
    }
    let tstream = &buf[pos..pos + tlen];
    if zlen < 8 || tlen < 8 {
        return Err("cln16: state");
    }

    let mask = (SCALE - 1) as u64;
    let mut zstate = u64::from_le_bytes(zstream[zlen - 8..].try_into().unwrap());
    let mut zcur = zlen - 8;
    let mut tstate = u64::from_le_bytes(tstream[tlen - 8..].try_into().unwrap());
    let mut tcur = tlen - 8;
    let mut ei = 0usize;
    let mut out = vec![0u8; orig_len];
    let mut prev = 0u8;
    for i in 0..orig_len {
        let c = ctx(prev);
        let p0 = p_from_u8(p0u8[c]);
        let slot = (zstate & mask) as u32;
        let is_zero = slot < p0;
        let (zf, zcum) = if is_zero {
            (p0 as u64, 0u64)
        } else {
            ((SCALE - p0) as u64, p0 as u64)
        };
        zstate = zf * (zstate >> SCALE_BITS) + (zstate & mask) - zcum;
        while zstate < RANS_L {
            if zcur == 0 {
                return Err("cln16: z underrun");
            }
            zcur -= 1;
            zstate = (zstate << 8) | zstream[zcur] as u64;
        }
        if is_zero {
            out[i] = 0;
            prev = 0;
            continue;
        }
        let tslot = (tstate & mask) as u32;
        let s = find_sym67(tslot, &freq[c], &start[c]);
        let tf = freq[c][s as usize] as u64;
        let tcum = start[c][s as usize] as u64;
        tstate = tf * (tstate >> SCALE_BITS) + (tstate & mask) - tcum;
        while tstate < RANS_L {
            if tcur == 0 {
                return Err("cln16: t underrun");
            }
            tcur -= 1;
            tstate = (tstate << 8) | tstream[tcur] as u64;
        }
        let b = if s as usize == ESC {
            if ei >= escapes.len() {
                return Err("cln16: esc underrun");
            }
            let v = escapes[ei];
            ei += 1;
            v
        } else {
            s
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
    fn cln24_roundtrip() {
        let mut src = Vec::new();
        for _ in 0..400 {
            src.extend_from_slice(&[0, 0, 0, 1, 0, 2, 0, 0, 5, 0, 20, 0]);
        }
        src.push(200);
        let c = encode(&src);
        assert_eq!(decode(&c).unwrap(), src);
        assert_eq!(&c[..4], MAGIC);
    }
}
