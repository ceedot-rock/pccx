//! Zero-Run Wrapper. 8-byte flagship for *all-zero* buffers only.
//!
//! Layout: magic `ZRW\0` (4) + original length as little-endian u32 (4).
//! This is not a general codec. Do not claim it beats gzip on mixed data.

pub const MAGIC: &[u8; 4] = b"ZRW\0";

pub fn is_zero_run(chunk: &[u8]) -> bool {
    !chunk.is_empty() && chunk.iter().all(|&b| b == 0)
}

pub fn compress_zeros_int32_le(count: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(8);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(count as u32).to_le_bytes());
    out
}

pub fn decompress_zeros(buf: &[u8]) -> Result<Vec<u8>, &'static str> {
    if buf.len() != 8 {
        return Err("zrw: expected 8-byte header");
    }
    if &buf[0..4] != MAGIC {
        return Err("zrw: bad magic");
    }
    let count = u32::from_le_bytes(buf[4..8].try_into().unwrap()) as usize;
    Ok(vec![0u8; count])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flagship_8b_roundtrip() {
        let raw = vec![0u8; 10_000];
        assert!(is_zero_run(&raw));
        let c = compress_zeros_int32_le(raw.len());
        assert_eq!(c.len(), 8);
        assert_eq!(decompress_zeros(&c).unwrap(), raw);
    }

    #[test]
    fn rejects_nonzero() {
        assert!(!is_zero_run(&[0, 0, 1]));
        assert!(!is_zero_run(&[]));
    }
}
