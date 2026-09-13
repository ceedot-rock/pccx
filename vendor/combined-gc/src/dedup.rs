//! Exact 16K-block dedup on a post-BWT/MTF byte stream.

use xxhash_rust::xxh3::xxh3_64;

pub const MAGIC: &[u8; 4] = b"DDP1";
pub const BLOCK: usize = 16_384;

pub fn encode(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() / 8 + 16);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    let mut seen: Vec<(u64, usize)> = Vec::new();
    let mut i = 0usize;
    while i < data.len() {
        let n = (data.len() - i).min(BLOCK);
        let slice = &data[i..i + n];
        let h = xxh3_64(slice);
        let mut copied = false;
        for &(oh, off) in seen.iter().rev().take(64) {
            if oh == h && off + n <= data.len() && &data[off..off + n] == slice {
                out.push(1);
                out.extend_from_slice(&(n as u16).to_le_bytes());
                out.extend_from_slice(&(off as u32).to_le_bytes());
                copied = true;
                break;
            }
        }
        if !copied {
            seen.push((h, i));
            out.push(0);
            out.extend_from_slice(&(n as u16).to_le_bytes());
            out.extend_from_slice(slice);
        }
        i += n;
    }
    out
}

pub fn decode(buf: &[u8]) -> Result<Vec<u8>, &'static str> {
    if buf.len() < 8 || &buf[..4] != MAGIC {
        return Err("ddp: magic");
    }
    let orig = u32::from_le_bytes(buf[4..8].try_into().unwrap()) as usize;
    let mut pos = 8usize;
    let mut out = Vec::with_capacity(orig);
    while out.len() < orig {
        if pos >= buf.len() {
            return Err("ddp: trunc");
        }
        let tag = buf[pos];
        pos += 1;
        if pos + 2 > buf.len() {
            return Err("ddp: len");
        }
        let n = u16::from_le_bytes(buf[pos..pos + 2].try_into().unwrap()) as usize;
        pos += 2;
        if n == 0 || out.len() + n > orig {
            return Err("ddp: range");
        }
        match tag {
            0 => {
                if pos + n > buf.len() {
                    return Err("ddp: lit");
                }
                out.extend_from_slice(&buf[pos..pos + n]);
                pos += n;
            }
            1 => {
                if pos + 4 > buf.len() {
                    return Err("ddp: off");
                }
                let off = u32::from_le_bytes(buf[pos..pos + 4].try_into().unwrap()) as usize;
                pos += 4;
                if off + n > out.len() {
                    return Err("ddp: oob");
                }
                for k in 0..n {
                    let b = out[off + k];
                    out.push(b);
                }
            }
            _ => return Err("ddp: tag"),
        }
    }
    if out.len() != orig {
        return Err("ddp: size");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedup_roundtrip() {
        let block = b"abcdefghijklmnopqrstuvwxyz012345".repeat(512);
        assert_eq!(block.len(), BLOCK);
        let mut src = Vec::new();
        src.extend_from_slice(&block);
        src.extend_from_slice(&block);
        src.extend_from_slice(&block[..100]);
        let c = encode(&src);
        assert!(c.len() < src.len());
        assert_eq!(decode(&c).unwrap(), src);
    }

    #[test]
    fn dedup_no_repeat_roundtrip() {
        let src: Vec<u8> = (0..20000).map(|i| (i % 251) as u8).collect();
        let c = encode(&src);
        assert_eq!(decode(&c).unwrap(), src);
    }
}
