//! AWAREv5.1 container.
//!
//! V12 layout (this crate, 1.1+):
//!   b"AWAREv5.1" | version:u8=1 | orig_total:u64 le | nblocks:u32 le
//!   repeated: prog_len:u32 | prog | orig_len:u32 | residual_len:u32 | residual
//!   xxh3_64 over the prefix.
//!
//! `prog` is a serialized `Program` (`PRG1...`), not raw BWT bytes.

use xxhash_rust::xxh3::xxh3_64;

pub const MAGIC: &[u8] = b"AWAREv5.1";
pub const VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AwareBlock {
    pub prog: Vec<u8>,
    pub residual: Vec<u8>,
    pub orig_len: u32,
}

#[derive(Debug, Clone)]
pub struct ParsedAware {
    pub orig_total: u64,
    pub blocks: Vec<AwareBlock>,
}

pub fn encode_aware(blocks: Vec<AwareBlock>, orig_total: u64) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.push(VERSION);
    out.extend_from_slice(&orig_total.to_le_bytes());
    out.extend_from_slice(&(blocks.len() as u32).to_le_bytes());
    for b in blocks {
        out.extend_from_slice(&(b.prog.len() as u32).to_le_bytes());
        out.extend_from_slice(&b.prog);
        out.extend_from_slice(&b.orig_len.to_le_bytes());
        out.extend_from_slice(&(b.residual.len() as u32).to_le_bytes());
        out.extend_from_slice(&b.residual);
    }
    let h = xxh3_64(&out);
    out.extend_from_slice(&h.to_le_bytes());
    out
}

pub fn verify_aware(buf: &[u8]) -> Result<(), &'static str> {
    if buf.len() < MAGIC.len() + 1 + 8 + 4 + 8 {
        return Err("aware: truncated");
    }
    if !buf.starts_with(MAGIC) {
        return Err("aware: bad magic");
    }
    let (body, tail) = buf.split_at(buf.len() - 8);
    let expect = xxh3_64(body);
    let got = u64::from_le_bytes(tail.try_into().unwrap());
    if expect != got {
        return Err("aware: xxh3 mismatch");
    }
    Ok(())
}

pub fn parse_aware(buf: &[u8]) -> Result<ParsedAware, &'static str> {
    verify_aware(buf)?;
    let mut pos = MAGIC.len();
    if buf[pos] != VERSION {
        return Err("aware: bad version");
    }
    pos += 1;
    let orig_total = u64::from_le_bytes(buf[pos..pos + 8].try_into().unwrap());
    pos += 8;
    let nblocks = u32::from_le_bytes(buf[pos..pos + 4].try_into().unwrap()) as usize;
    pos += 4;
    let body_end = buf.len() - 8;
    let mut blocks = Vec::with_capacity(nblocks);
    for _ in 0..nblocks {
        if pos + 4 > body_end {
            return Err("aware: truncated prog_len");
        }
        let prog_len = u32::from_le_bytes(buf[pos..pos + 4].try_into().unwrap()) as usize;
        pos += 4;
        if pos + prog_len + 8 > body_end {
            return Err("aware: truncated prog");
        }
        let prog = buf[pos..pos + prog_len].to_vec();
        pos += prog_len;
        let orig_len = u32::from_le_bytes(buf[pos..pos + 4].try_into().unwrap());
        pos += 4;
        let res_len = u32::from_le_bytes(buf[pos..pos + 4].try_into().unwrap()) as usize;
        pos += 4;
        if pos + res_len > body_end {
            return Err("aware: truncated residual");
        }
        let residual = buf[pos..pos + res_len].to_vec();
        pos += res_len;
        blocks.push(AwareBlock {
            prog,
            residual,
            orig_len,
        });
    }
    Ok(ParsedAware { orig_total, blocks })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_parse() {
        let blocks = vec![AwareBlock {
            prog: b"PRG1\x01\x00\x00".to_vec(),
            residual: b"xyz".to_vec(),
            orig_len: 3,
        }];
        let encoded = encode_aware(blocks, 3);
        assert!(verify_aware(&encoded).is_ok());
        let p = parse_aware(&encoded).unwrap();
        assert_eq!(p.orig_total, 3);
        assert_eq!(p.blocks[0].residual, b"xyz");
    }
}
