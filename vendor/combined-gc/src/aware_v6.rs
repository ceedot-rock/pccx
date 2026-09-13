//! AWAREv6: same block payload as v5.1 plus a 1MB decoded sliding dict.
//!
//! Encoder and decoder rebuild the dict from decoded blocks in order.
//! MTF list is seeded from dict recency (most recent unique byte first).
//! No new coder. Block 0 has empty dict.

use crate::mtf;
use xxhash_rust::xxh3::xxh3_64;

pub const MAGIC: &[u8] = b"AWAREv6\0";
pub const DICT_SIZE: usize = 1_000_000;

#[derive(Debug, Clone)]
pub struct AwareV6 {
    pub dict: Vec<u8>,
}

impl Default for AwareV6 {
    fn default() -> Self {
        Self {
            dict: Vec::with_capacity(DICT_SIZE),
        }
    }
}

impl AwareV6 {
    pub fn update(&mut self, decoded: &[u8]) {
        self.dict.extend_from_slice(decoded);
        if self.dict.len() > DICT_SIZE {
            let drop = self.dict.len() - DICT_SIZE;
            self.dict.drain(0..drop);
        }
    }

    pub fn seed_list(&self) -> [u8; 256] {
        mtf::seed_list_from_dict(&self.dict)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct V6Block {
    pub prog: Vec<u8>,
    pub residual: Vec<u8>,
    pub orig_len: u32,
}

pub fn encode_v6(blocks: Vec<V6Block>, orig_total: u64) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&orig_total.to_le_bytes());
    out.extend_from_slice(&(blocks.len() as u32).to_le_bytes());
    out.extend_from_slice(&(DICT_SIZE as u32).to_le_bytes());
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

pub fn parse_v6(buf: &[u8]) -> Result<(u64, Vec<V6Block>), &'static str> {
    if buf.len() < MAGIC.len() + 8 + 4 + 4 + 8 {
        return Err("awarev6: truncated");
    }
    if !buf.starts_with(MAGIC) {
        return Err("awarev6: bad magic");
    }
    let (body, tail) = buf.split_at(buf.len() - 8);
    let expect = xxh3_64(body);
    let got = u64::from_le_bytes(tail.try_into().unwrap());
    if expect != got {
        return Err("awarev6: xxh3 mismatch");
    }
    let mut pos = MAGIC.len();
    let orig_total = u64::from_le_bytes(body[pos..pos + 8].try_into().unwrap());
    pos += 8;
    let nblocks = u32::from_le_bytes(body[pos..pos + 4].try_into().unwrap()) as usize;
    pos += 4;
    let _dict_size = u32::from_le_bytes(body[pos..pos + 4].try_into().unwrap());
    pos += 4;
    let mut blocks = Vec::with_capacity(nblocks);
    for _ in 0..nblocks {
        if pos + 4 > body.len() {
            return Err("awarev6: truncated prog_len");
        }
        let prog_len = u32::from_le_bytes(body[pos..pos + 4].try_into().unwrap()) as usize;
        pos += 4;
        if pos + prog_len + 8 > body.len() {
            return Err("awarev6: truncated prog");
        }
        let prog = body[pos..pos + prog_len].to_vec();
        pos += prog_len;
        let orig_len = u32::from_le_bytes(body[pos..pos + 4].try_into().unwrap());
        pos += 4;
        let res_len = u32::from_le_bytes(body[pos..pos + 4].try_into().unwrap()) as usize;
        pos += 4;
        if pos + res_len > body.len() {
            return Err("awarev6: truncated residual");
        }
        let residual = body[pos..pos + res_len].to_vec();
        pos += res_len;
        blocks.push(V6Block {
            prog,
            residual,
            orig_len,
        });
    }
    Ok((orig_total, blocks))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dict_caps_at_1mb() {
        let mut d = AwareV6::default();
        d.update(&vec![1u8; 700_000]);
        d.update(&vec![2u8; 700_000]);
        assert_eq!(d.dict.len(), DICT_SIZE);
        assert!(d.dict.iter().all(|&b| b == 1 || b == 2));
        assert_eq!(*d.dict.last().unwrap(), 2);
    }

    #[test]
    fn v6_roundtrip_bytes() {
        let blocks = vec![V6Block {
            prog: b"PRG1\x01\x00\x00".to_vec(),
            residual: b"xyz".to_vec(),
            orig_len: 3,
        }];
        let enc = encode_v6(blocks, 3);
        let (n, b) = parse_v6(&enc).unwrap();
        assert_eq!(n, 3);
        assert_eq!(b[0].residual, b"xyz");
    }
}
