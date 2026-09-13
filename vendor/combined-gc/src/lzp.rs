//! 256K LZP prefilter. Escape 0xFF + (len-4) + dist24.

pub const LZP_MIN: usize = 4;
pub const LZP_WIN: usize = 262_144;
pub const LZP_HASH_BITS: usize = 18;
pub const LZP_MAX_LEN: usize = 258;

pub fn encode(src: &[u8]) -> Vec<u8> {
    let mask = (1 << LZP_HASH_BITS) - 1;
    let mut hash = vec![-1isize; 1 << LZP_HASH_BITS];
    let mut out = Vec::with_capacity(src.len());
    let mut i = 0usize;
    while i < src.len() {
        if i + 3 < src.len() {
            let h = ((src[i] as usize) << 16
                | (src[i + 1] as usize) << 8
                | src[i + 2] as usize
                | (src[i + 3] as usize) << 4)
                & mask;
            let prev = hash[h];
            hash[h] = i as isize;
            if prev >= 0 && (i as isize - prev) <= LZP_WIN as isize {
                let p = prev as usize;
                let mut len = 0usize;
                while i + len < src.len()
                    && p + len < i
                    && src[p + len] == src[i + len]
                    && len < LZP_MAX_LEN
                {
                    len += 1;
                }
                if len >= LZP_MIN {
                    out.push(0xFF);
                    // 0x00 reserved for literal 0xFF. len=4 → 0x01.
                    out.push((len - LZP_MIN + 1) as u8);
                    let dist = (i - p) as u32;
                    out.extend_from_slice(&dist.to_le_bytes()[..3]);
                    i += len;
                    continue;
                }
            }
        }
        if src[i] == 0xFF {
            out.push(0xFF);
            out.push(0x00);
        } else {
            out.push(src[i]);
        }
        i += 1;
    }
    out
}

pub fn decode(buf: &[u8], orig_len: usize) -> Result<Vec<u8>, &'static str> {
    let mut out = Vec::with_capacity(orig_len);
    let mut i = 0usize;
    while i < buf.len() && out.len() < orig_len {
        if buf[i] == 0xFF {
            if i + 1 >= buf.len() {
                return Err("lzp: truncated");
            }
            if buf[i + 1] == 0x00 {
                out.push(0xFF);
                i += 2;
                continue;
            }
            if i + 4 >= buf.len() {
                return Err("lzp: match hdr");
            }
            let len = buf[i + 1] as usize + LZP_MIN - 1;
            let dist = u32::from_le_bytes([buf[i + 2], buf[i + 3], buf[i + 4], 0]) as usize;
            if dist == 0 || dist > out.len() {
                return Err("lzp: dist");
            }
            let start = out.len() - dist;
            for k in 0..len {
                if out.len() >= orig_len {
                    break;
                }
                out.push(out[start + k]);
            }
            i += 5;
        } else {
            out.push(buf[i]);
            i += 1;
        }
    }
    out.truncate(orig_len);
    if out.len() != orig_len {
        return Err("lzp: length");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lzp_roundtrip_repeat() {
        let src = b"the quick brown fox jumps over the lazy dog\n".repeat(80);
        let c = encode(&src);
        let d = decode(&c, src.len()).unwrap();
        assert_eq!(d, src.as_slice());
        assert!(c.len() < src.len());
    }

    #[test]
    fn lzp_ff_literal() {
        let src = vec![0xFFu8, 1, 2, 0xFF, 3];
        let c = encode(&src);
        assert_eq!(decode(&c, src.len()).unwrap(), src);
    }
}
