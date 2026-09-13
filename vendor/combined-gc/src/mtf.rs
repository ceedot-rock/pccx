//! Move-to-front + bzip2 RLE0 (RUNA/RUNB) merged into one stage.

pub fn identity_list() -> [u8; 256] {
    let mut list = [0u8; 256];
    for i in 0..256 {
        list[i] = i as u8;
    }
    list
}

pub fn mtf_list_after(data: &[u8], start: &[u8; 256]) -> [u8; 256] {
    let mut dict = start.to_vec();
    for &b in data {
        if let Some(i) = dict.iter().position(|&x| x == b) {
            dict.remove(i);
            dict.insert(0, b);
        }
    }
    let mut out = [0u8; 256];
    out.copy_from_slice(&dict);
    out
}

pub fn seed_list_from_dict(dict: &[u8]) -> [u8; 256] {
    let mut list = [0u8; 256];
    let mut used = [false; 256];
    let mut n = 0usize;
    for &b in dict.iter().rev() {
        if !used[b as usize] {
            used[b as usize] = true;
            list[n] = b;
            n += 1;
            if n == 256 {
                return list;
            }
        }
    }
    for b in 0..=255u8 {
        if !used[b as usize] {
            list[n] = b;
            n += 1;
        }
    }
    list
}

pub fn mtf_encode_with_list(data: &[u8], start: &[u8; 256]) -> Vec<u8> {
    let mut dict: Vec<u8> = start.to_vec();
    let mut out = Vec::with_capacity(data.len());
    for &b in data {
        let i = dict.iter().position(|&x| x == b).unwrap_or(0);
        out.push(i as u8);
        dict.remove(i);
        dict.insert(0, b);
    }
    out
}

pub fn mtf_decode_with_list(data: &[u8], start: &[u8; 256]) -> Vec<u8> {
    let mut dict: Vec<u8> = start.to_vec();
    let mut out = Vec::with_capacity(data.len());
    for &idx in data {
        let i = idx as usize;
        if i >= dict.len() {
            break;
        }
        let b = dict[i];
        out.push(b);
        dict.remove(i);
        dict.insert(0, b);
    }
    out
}

pub fn mtf_encode(data: &[u8]) -> Vec<u8> {
    let mut dict: Vec<u8> = (0..=255).collect();
    let mut out = Vec::with_capacity(data.len());
    for &b in data {
        let i = dict.iter().position(|&x| x == b).unwrap_or(0);
        out.push(i as u8);
        dict.remove(i);
        dict.insert(0, b);
    }
    out
}

pub fn mtf_decode(data: &[u8]) -> Vec<u8> {
    let mut dict: Vec<u8> = (0..=255).collect();
    let mut out = Vec::with_capacity(data.len());
    for &idx in data {
        let i = idx as usize;
        if i >= dict.len() {
            break;
        }
        let b = dict[i];
        out.push(b);
        dict.remove(i);
        dict.insert(0, b);
    }
    out
}

const RUNA: u16 = 0;
const RUNB: u16 = 1;

fn put_sym(s: u16, out: &mut Vec<u8>) {
    if s < 255 {
        out.push(s as u8);
    } else {
        out.push(255);
        out.push((s - 255) as u8);
    }
}

fn flush_zeros(mut n: u32, out: &mut Vec<u8>) {
    while n > 0 {
        if n & 1 == 1 {
            put_sym(RUNA, out);
        } else {
            put_sym(RUNB, out);
        }
        n = (n - 1) >> 1;
    }
}

pub fn rle0_from_ranks(ranks: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(ranks.len());
    let mut zeros = 0u32;
    for &r in ranks {
        if r == 0 {
            zeros += 1;
        } else {
            if zeros > 0 {
                flush_zeros(zeros, &mut out);
                zeros = 0;
            }
            put_sym(r as u16 + 1, &mut out);
        }
    }
    if zeros > 0 {
        flush_zeros(zeros, &mut out);
    }
    out
}

pub fn rle0_to_ranks(buf: &[u8]) -> Vec<u8> {
    let mut ranks = Vec::with_capacity(buf.len());
    let mut i = 0usize;
    let mut run_pow = 0u32;
    let mut pending = 0u32;
    while i < buf.len() {
        let s = if buf[i] < 255 {
            let v = buf[i] as u16;
            i += 1;
            v
        } else {
            if i + 1 >= buf.len() {
                break;
            }
            let v = 255u16 + buf[i + 1] as u16;
            i += 2;
            v
        };
        if s == RUNA || s == RUNB {
            let add = if s == RUNA { 1u32 } else { 2 };
            pending += add << run_pow;
            run_pow += 1;
        } else {
            for _ in 0..pending {
                ranks.push(0);
            }
            pending = 0;
            run_pow = 0;
            ranks.push(s.saturating_sub(1).min(255) as u8);
        }
    }
    for _ in 0..pending {
        ranks.push(0);
    }
    ranks
}

pub fn mtf_rle0_encode(data: &[u8]) -> Vec<u8> {
    rle0_from_ranks(&mtf_encode(data))
}

pub fn mtf_rle0_decode(buf: &[u8]) -> Vec<u8> {
    mtf_decode(&rle0_to_ranks(buf))
}

pub fn mtf_rle0_encode_seeded(data: &[u8], start: &[u8; 256]) -> Vec<u8> {
    rle0_from_ranks(&mtf_encode_with_list(data, start))
}

pub fn mtf_rle0_decode_seeded(buf: &[u8], start: &[u8; 256]) -> Vec<u8> {
    mtf_decode_with_list(&rle0_to_ranks(buf), start)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip() {
        let src = b"banana-bandana";
        assert_eq!(mtf_decode(&mtf_encode(src)), src);
    }

    #[test]
    fn rle0_roundtrip_repeats() {
        let src = b"aaaaabbbbbcccccxxxxx".repeat(20);
        let c = mtf_rle0_encode(&src);
        let d = mtf_rle0_decode(&c);
        assert_eq!(d, src.as_slice());
        assert!(c.len() < src.len());
    }

    #[test]
    fn rle0_roundtrip_text() {
        let src = b"the quick brown fox jumps over the lazy dog\n".repeat(8);
        assert_eq!(mtf_rle0_decode(&mtf_rle0_encode(src.as_slice())), src.as_slice());
    }

    #[test]
    fn seeded_roundtrip() {
        let dict = b"the quick brown fox";
        let seed = seed_list_from_dict(dict);
        let src = b"the fox and the brown dog";
        let c = mtf_rle0_encode_seeded(src, &seed);
        assert_eq!(mtf_rle0_decode_seeded(&c, &seed), src);
    }

    #[test]
    fn seeded_mtf_roundtrip() {
        let start = identity_list();
        let src = b"aaaaabbbbb the the the fox fox\n".repeat(40);
        let c = mtf_rle0_encode_seeded(&src, &start);
        assert_eq!(mtf_rle0_decode_seeded(&c, &start), src.as_slice());
        let after = mtf_list_after(&src, &start);
        let c2 = mtf_rle0_encode_seeded(&src, &after);
        assert_eq!(mtf_rle0_decode_seeded(&c2, &after), src.as_slice());
    }
}
