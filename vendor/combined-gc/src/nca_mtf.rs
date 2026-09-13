//! NCM2: NCA mix on MTF symbols, not raw bits.
//! BWT + MTF+RUNA/RUNB already clustered the alphabet. This codes that stream.

use crate::ans::{RansEnc, RANS_L, SCALE};
use crate::clean_o8;

pub const MAGIC: &[u8; 4] = b"NCM2";
const N_CTX: usize = 8;
const N_SYM: usize = 256;

fn ctx(prev: u8) -> usize {
    clean_o8::ctx(prev).min(N_CTX - 1)
}

fn normalize(cnt: &[u32; N_SYM]) -> ([u32; N_SYM], [u32; N_SYM]) {
    let tot: u32 = cnt.iter().sum();
    let mut freq = [0u32; N_SYM];
    let mut start = [0u32; N_SYM];
    if tot == 0 {
        freq[0] = SCALE;
        return (freq, start);
    }
    let mut assigned = 0u32;
    for (i, &c) in cnt.iter().enumerate() {
        if c == 0 {
            continue;
        }
        let f = ((c as u64 * SCALE as u64) / tot as u64).max(1) as u32;
        freq[i] = f;
        assigned += f;
    }
    if assigned != SCALE {
        let mut big = 0usize;
        for i in 0..N_SYM {
            if freq[i] > freq[big] {
                big = i;
            }
        }
        if assigned < SCALE {
            freq[big] += SCALE - assigned;
        } else if freq[big] > assigned - SCALE + 1 {
            freq[big] -= assigned - SCALE;
        }
    }
    let mut acc = 0u32;
    for i in 0..N_SYM {
        start[i] = acc;
        acc += freq[i];
    }
    (freq, start)
}

pub fn encode(mtf: &[u8]) -> Vec<u8> {
    if mtf.is_empty() {
        return MAGIC.to_vec();
    }
    let mut cnt = [[1u32; N_SYM]; N_CTX];
    let mut rec: Vec<(u8, u32, u32)> = Vec::with_capacity(mtf.len());
    let mut prev = 0u8;
    for &s in mtf {
        let c = ctx(prev);
        let (freq, start) = normalize(&cnt[c]);
        rec.push((s, freq[s as usize], start[s as usize]));
        cnt[c][s as usize] += 1;
        prev = s;
    }

    let mut stream = Vec::with_capacity(mtf.len() / 2);
    let mut enc = RansEnc::new();
    for &(sym, f, cum) in rec.iter().rev() {
        let _ = sym;
        enc.encode(f.max(1), cum, SCALE, &mut stream);
    }
    enc.flush(&mut stream);

    let mut out = Vec::with_capacity(8 + stream.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(mtf.len() as u32).to_le_bytes());
    out.extend_from_slice(&(stream.len() as u32).to_le_bytes());
    out.extend_from_slice(&stream);
    out
}

pub fn decode(buf: &[u8], orig_mtf: &[u8]) -> Result<Vec<u8>, &'static str> {
    // decode needs orig only for escape restore in this first cut — full invert below
    let _ = orig_mtf;
    decode_full(buf)
}

pub fn decode_full(buf: &[u8]) -> Result<Vec<u8>, &'static str> {
    if buf.len() < 12 || &buf[..4] != MAGIC {
        return Err("ncm2: magic");
    }
    let n = u32::from_le_bytes(buf[4..8].try_into().unwrap()) as usize;
    let slen = u32::from_le_bytes(buf[8..12].try_into().unwrap()) as usize;
    if buf.len() < 12 + slen || slen < 8 {
        return Err("ncm2: stream");
    }
    let stream = &buf[12..12 + slen];
    let mut cnt = [[1u32; N_SYM]; N_CTX];
    let mut cursor = slen;
    let mut state = u64::from_le_bytes(stream[slen - 8..slen].try_into().unwrap());
    cursor -= 8;
    while state < RANS_L && cursor > 0 {
        cursor -= 1;
        state = (state << 8) | stream[cursor] as u64;
    }
    let mut out = Vec::with_capacity(n);
    let mut prev = 0u8;
    for _ in 0..n {
        let c = ctx(prev);
        let (freq, start) = normalize(&cnt[c]);
        let slot = (state & (SCALE as u64 - 1)) as u32;
        let mut sym = 0u8;
        for s in 0..N_SYM {
            if freq[s] > 0 && slot >= start[s] && slot < start[s] + freq[s] {
                sym = s as u8;
                break;
            }
        }
        let f = freq[sym as usize].max(1);
        let cum = start[sym as usize];
        state = f as u64 * (state >> 12) + (state & (SCALE as u64 - 1)) - cum as u64;
        while state < RANS_L && cursor > 0 {
            cursor -= 1;
            state = (state << 8) | stream[cursor] as u64;
        }
        out.push(sym);
        cnt[c][sym as usize] += 1;
        prev = sym;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ncm2_roundtrip_mtf_like() {
        let mut src = Vec::new();
        for _ in 0..800 {
            src.extend_from_slice(&[0, 0, 0, 1, 0, 0, 2, 0, 3, 0, 5, 0, 0]);
        }
        let enc = encode(&src);
        let back = decode_full(&enc).expect("dec");
        assert_eq!(back, src);
    }

    #[test]
    fn ncm2_vs_cln_on_dickens_mtf() {
        let path = "/tmp/dickens900.bin";
        let raw = match std::fs::read(path) {
            Ok(s) => s,
            Err(_) => return,
        };
        let bwt = crate::transforms::bwt_sa_indexed(&raw, raw.len());
        let mtf = crate::mtf::mtf_rle0_encode(&bwt);
        let cln = crate::clean_o1::encode_auto(&mtf);
        let ncm2 = encode(&mtf);
        let bits = crate::nca_mix::encode(&mtf);
        eprintln!(
            "dickens900 MTF len={} CLN={} NCM2={} NCM1-on-MTF={}",
            mtf.len(),
            cln.len(),
            ncm2.len(),
            bits.len()
        );
        let back = decode_full(&ncm2).expect("rt");
        assert_eq!(back, mtf);
    }
}
