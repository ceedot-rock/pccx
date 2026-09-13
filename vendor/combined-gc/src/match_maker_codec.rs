//! 1.19.2 — rANS on LZ tokens. Finder stays in match_maker.
//! Not in pick_best until it beats obj1 10,322 / osdb 27,445.

use crate::ans;
use crate::match_maker::{MatchMaker, Params, Token};

pub struct TokenStreams {
    pub flags: Vec<u8>,
    pub lits: Vec<u8>,
    pub lens: Vec<u16>,
    pub dists: Vec<u32>,
}

pub fn split_tokens(tokens: &[Token], min_match: usize) -> TokenStreams {
    let mut flags = Vec::with_capacity(tokens.len());
    let mut lits = Vec::new();
    let mut lens = Vec::new();
    let mut dists = Vec::new();
    let mm = min_match.max(3) as u16;
    for t in tokens {
        match *t {
            Token::Lit(b) => {
                flags.push(0);
                lits.push(b);
            }
            Token::Match { dist, len } => {
                flags.push(1);
                let base = (len as u16).saturating_sub(mm);
                lens.push(base);
                dists.push(dist as u32);
            }
        }
    }
    TokenStreams {
        flags,
        lits,
        lens,
        dists,
    }
}

fn len_to_code(len: u16) -> (u8, u16) {
    match len {
        0..=15 => (len as u8, 0),
        16..=23 => (16, len - 16),
        24..=39 => (17, len - 24),
        40..=71 => (18, len - 40),
        _ => (19, len.saturating_sub(72)),
    }
}

fn dist_to_code(dist: u32) -> (u8, u32) {
    if dist <= 1 {
        return (0, 0);
    }
    let log = 32 - dist.leading_zeros();
    let code = log.min(31) as u8;
    let extra = dist - (1u32 << (log - 1));
    (code, extra)
}

fn blob(data: &[u8]) -> Vec<u8> {
    if data.is_empty() {
        return Vec::new();
    }
    ans::rans_encode(data)
}

pub fn encode_tokens_rans(tokens: &[Token], min_match: usize) -> Vec<u8> {
    let s = split_tokens(tokens, min_match);
    let mut lens_codes = Vec::with_capacity(s.lens.len());
    let mut dist_codes = Vec::with_capacity(s.dists.len());
    let mut extra = Vec::new();
    for &l in &s.lens {
        let (c, e) = len_to_code(l);
        lens_codes.push(c);
        if c >= 16 {
            extra.push(e.min(255) as u8);
        }
    }
    for &d in &s.dists {
        let (c, e) = dist_to_code(d);
        dist_codes.push(c);
        if c <= 1 {
            continue;
        } else if c <= 8 {
            extra.push(e as u8);
        } else if c <= 16 {
            extra.extend_from_slice(&(e as u16).to_le_bytes());
        } else {
            extra.extend_from_slice(&e.to_le_bytes());
        }
    }
    let f = blob(&s.flags);
    let l = blob(&s.lits);
    let n = blob(&lens_codes);
    let d = blob(&dist_codes);
    let mut out = Vec::with_capacity(32 + f.len() + l.len() + n.len() + d.len() + extra.len());
    out.extend_from_slice(b"MMR1");
    out.extend_from_slice(&(tokens.len() as u32).to_le_bytes());
    out.extend_from_slice(&(s.lits.len() as u32).to_le_bytes());
    out.extend_from_slice(&(s.lens.len() as u32).to_le_bytes());
    out.extend_from_slice(&(f.len() as u32).to_le_bytes());
    out.extend_from_slice(&(l.len() as u32).to_le_bytes());
    out.extend_from_slice(&(n.len() as u32).to_le_bytes());
    out.extend_from_slice(&(d.len() as u32).to_le_bytes());
    out.extend_from_slice(&(extra.len() as u32).to_le_bytes());
    out.extend_from_slice(&f);
    out.extend_from_slice(&l);
    out.extend_from_slice(&n);
    out.extend_from_slice(&d);
    out.extend_from_slice(&extra);
    out
}

pub fn try_lz_rans(data: &[u8]) -> Vec<u8> {
    let params = Params::aware(data, data.len() as u64);
    let window = if data.len() < 128 * 1024 {
        64 * 1024
    } else {
        2 * 1024 * 1024
    };
    let mut mm = MatchMaker::new(params, window);
    let tokens = mm.lazy_parse(data);
    encode_tokens_rans(&tokens, params.min_match)
}

pub fn compress_obj1_aware(data: &[u8]) -> Vec<u8> {
    let params = Params::aware(data, data.len() as u64);
    let mut mm = MatchMaker::new(params, 64 * 1024);
    let tokens = mm.lazy_parse(data);
    encode_tokens_rans(&tokens, params.min_match)
}

fn zlib_best(data: &[u8]) -> Vec<u8> {
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use std::io::Write;
    let mut e = ZlibEncoder::new(Vec::new(), Compression::best());
    e.write_all(data).unwrap();
    e.finish().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::match_maker::encode_tokens;

    #[test]
    fn codec_beats_dump() {
        let data = b"abcabcabcabc".repeat(1000);
        let params = Params::aware(&data, data.len() as u64);
        let mut mm = MatchMaker::new(params, 64 * 1024);
        let tokens = mm.lazy_parse(&data);
        let dump = encode_tokens(&tokens).len();
        let rans = encode_tokens_rans(&tokens, params.min_match).len();
        assert!(rans < dump, "rans {} vs dump {}", rans, dump);
    }

    #[test]
    fn obj1_path_uses_64k() {
        let data = vec![0u8; 21504];
        let out = compress_obj1_aware(&data);
        assert!(out.len() < 21504);
    }

    #[test]
    fn obj1_rans_vs_zlib() {
        let path = "/home/workdir/artifacts/corpora/obj1";
        let Ok(data) = std::fs::read(path) else {
            return;
        };
        let z = zlib_best(&data);
        let r = try_lz_rans(&data);
        eprintln!(
            "obj1 raw={} zlib={} lz-rans={} vs lock 10322",
            data.len(),
            z.len(),
            r.len()
        );
        let osdb = std::fs::read("/home/workdir/artifacts/corpora/osdb").ok();
        if let Some(full) = osdb {
            let slice = &full[..full.len().min(64_000)];
            let z2 = zlib_best(slice);
            let r2 = try_lz_rans(slice);
            eprintln!(
                "osdb64 raw={} zlib={} lz-rans={} vs lock 27445",
                slice.len(),
                z2.len(),
                r2.len()
            );
        }
    }
}
