//! Weighted Frequency Count after BWT. Invertible list-update, then same RUNA/RUNB as MTF.

use crate::mtf::{rle0_from_ranks, rle0_to_ranks};

const INC: u32 = 8;

fn better(c: usize, s: usize, w: &[u32; 256], last: &[u32; 256]) -> bool {
    if w[c] != w[s] {
        return w[c] > w[s];
    }
    if last[c] != last[s] {
        return last[c] > last[s];
    }
    c < s
}

pub fn wfc_encode(data: &[u8]) -> Vec<u8> {
    let mut w = [0u32; 256];
    let mut last = [0u32; 256];
    let mut list: Vec<u8> = (0..=255).collect();
    let mut out = Vec::with_capacity(data.len());
    for (i, &s) in data.iter().enumerate() {
        let pos = list.iter().position(|&x| x == s).unwrap_or(0);
        out.push(pos as u8);
        w[s as usize] = w[s as usize].saturating_add(INC);
        last[s as usize] = i as u32 + 1;
        list.remove(pos);
        let mut j = 0usize;
        while j < list.len() && better(list[j] as usize, s as usize, &w, &last) {
            j += 1;
        }
        list.insert(j, s);
    }
    out
}

pub fn wfc_decode(ranks: &[u8]) -> Vec<u8> {
    let mut w = [0u32; 256];
    let mut last = [0u32; 256];
    let mut list: Vec<u8> = (0..=255).collect();
    let mut out = Vec::with_capacity(ranks.len());
    for (i, &rank) in ranks.iter().enumerate() {
        let pos = (rank as usize).min(list.len().saturating_sub(1));
        let s = list[pos];
        out.push(s);
        w[s as usize] = w[s as usize].saturating_add(INC);
        last[s as usize] = i as u32 + 1;
        list.remove(pos);
        let mut j = 0usize;
        while j < list.len() && better(list[j] as usize, s as usize, &w, &last) {
            j += 1;
        }
        list.insert(j, s);
    }
    out
}

/// Attached 1.18.3 variant: freq only, bubble, decay at 10000. No recency key.
pub fn wfc_freq_encode(data: &[u8]) -> Vec<u8> {
    let mut freq = [0usize; 256];
    let mut list: Vec<u8> = (0..=255).collect();
    let mut out = Vec::with_capacity(data.len());
    for &s in data {
        let pos = list.iter().position(|&c| c == s).unwrap_or(0);
        out.push(pos as u8);
        freq[s as usize] += 8;
        let mut i = pos;
        while i > 0 && freq[list[i] as usize] > freq[list[i - 1] as usize] {
            list.swap(i, i - 1);
            i -= 1;
        }
        if freq[s as usize] > 10000 {
            for f in freq.iter_mut() {
                *f >>= 1;
            }
        }
    }
    out
}

pub fn wfc_freq_decode(ranks: &[u8]) -> Vec<u8> {
    let mut freq = [0usize; 256];
    let mut list: Vec<u8> = (0..=255).collect();
    let mut out = Vec::with_capacity(ranks.len());
    for &rank in ranks {
        let r = (rank as usize).min(list.len().saturating_sub(1));
        let s = list[r];
        out.push(s);
        freq[s as usize] += 8;
        let mut i = r;
        while i > 0 && freq[list[i] as usize] > freq[list[i - 1] as usize] {
            list.swap(i, i - 1);
            i -= 1;
        }
        if freq[s as usize] > 10000 {
            for f in freq.iter_mut() {
                *f >>= 1;
            }
        }
    }
    out
}

pub fn wfc_rle0_encode(data: &[u8]) -> Vec<u8> {
    rle0_from_ranks(&wfc_encode(data))
}

pub fn wfc_rle0_decode(buf: &[u8]) -> Vec<u8> {
    wfc_decode(&rle0_to_ranks(buf))
}

pub fn rank_stats(ranks: &[u8]) -> (f64, [f64; 8], f64) {
    if ranks.is_empty() {
        return (0.0, [0.0; 8], 0.0);
    }
    let n = ranks.len() as f64;
    let mut hist = [0u32; 256];
    let mut sum = 0u64;
    for &r in ranks {
        hist[r as usize] += 1;
        sum += r as u64;
    }
    let mut p8 = [0.0; 8];
    for i in 0..8 {
        p8[i] = hist[i] as f64 / n;
    }
    let mut h = 0.0;
    for &c in &hist {
        if c == 0 {
            continue;
        }
        let p = c as f64 / n;
        h -= p * p.log2();
    }
    (h, p8, sum as f64 / n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wfc_roundtrip() {
        let src = b"banana-bandana-mississippi";
        assert_eq!(wfc_decode(&wfc_encode(src)), src);
    }

    #[test]
    fn wfc_freq_roundtrip() {
        let src = b"abracadabraabracadabra".repeat(20);
        assert_eq!(wfc_freq_decode(&wfc_freq_encode(&src)), src.as_slice());
    }

    #[test]
    fn wfc_rle0_roundtrip() {
        let src = b"aaaaabbbbbcccccxxxxx".repeat(40);
        let c = wfc_rle0_encode(&src);
        assert_eq!(wfc_rle0_decode(&c), src.as_slice());
    }
}
