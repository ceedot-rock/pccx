//! 1.19.3 — row/lazy finder, compact tokens, zlib-ng backend.
//! zlib encodes the token stream. Not pick_best until it beats 10,314.

use crate::match_maker::{MatchMaker, Params, Token};
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::io::Write;

fn put_varint(out: &mut Vec<u8>, mut v: u32) {
    loop {
        let mut b = (v & 0x7f) as u8;
        v >>= 7;
        if v != 0 {
            b |= 0x80;
        }
        out.push(b);
        if v == 0 {
            break;
        }
    }
}

/// Packed flags + raw lits + varint (len-min, dist).
pub fn compact_tokens(tokens: &[Token], min_match: usize) -> Vec<u8> {
    let mm = min_match.max(3) as u32;
    let mut flags = Vec::new();
    let mut payload = Vec::new();
    let mut acc = 0u8;
    let mut nbit = 0u8;
    for t in tokens {
        match *t {
            Token::Lit(b) => {
                acc |= 0 << nbit;
                nbit += 1;
                payload.push(b);
            }
            Token::Match { dist, len } => {
                acc |= 1 << nbit;
                nbit += 1;
                put_varint(&mut payload, (len as u32).saturating_sub(mm));
                put_varint(&mut payload, dist as u32);
            }
        }
        if nbit == 8 {
            flags.push(acc);
            acc = 0;
            nbit = 0;
        }
    }
    if nbit > 0 {
        flags.push(acc);
    }
    let mut out = Vec::with_capacity(8 + flags.len() + payload.len());
    out.extend_from_slice(b"MMZ1");
    out.extend_from_slice(&(tokens.len() as u32).to_le_bytes());
    out.extend_from_slice(&(flags.len() as u32).to_le_bytes());
    out.extend_from_slice(&flags);
    out.extend_from_slice(&payload);
    out
}

fn zlib_best(data: &[u8]) -> Vec<u8> {
    let mut e = ZlibEncoder::new(Vec::new(), Compression::best());
    e.write_all(data).unwrap();
    e.finish().unwrap()
}

pub fn compress_finder_zlib(data: &[u8]) -> Vec<u8> {
    let params = Params::aware(data, data.len() as u64);
    let window = if data.len() < 128 * 1024 {
        64 * 1024
    } else {
        2 * 1024 * 1024
    };
    let mut mm = MatchMaker::new(params, window);
    let tokens = mm.lazy_parse(data);
    let compact = compact_tokens(&tokens, params.min_match);
    zlib_best(&compact)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zeros_shrink() {
        let data = vec![0u8; 21504];
        let out = compress_finder_zlib(&data);
        assert!(out.len() < data.len());
    }

    #[test]
    fn obj1_finder_zlib_vs_zlib() {
        let path = "/home/workdir/artifacts/corpora/obj1";
        let Ok(data) = std::fs::read(path) else {
            return;
        };
        let raw_z = zlib_best(&data);
        let fz = compress_finder_zlib(&data);
        eprintln!(
            "obj1 raw={} zlib={} finder+zlib={} lock=10322",
            data.len(),
            raw_z.len(),
            fz.len()
        );
        if let Ok(full) = std::fs::read("/home/workdir/artifacts/corpora/osdb") {
            let slice = &full[..full.len().min(64_000)];
            let z2 = zlib_best(slice);
            let f2 = compress_finder_zlib(slice);
            eprintln!(
                "osdb64 raw={} zlib={} finder+zlib={} lock=27445",
                slice.len(),
                z2.len(),
                f2.len()
            );
        }
    }
}
