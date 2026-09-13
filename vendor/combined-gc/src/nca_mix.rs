//! Per-bit 5-expert mixer. Encode and decode run the same experts lockstep.
//! Container NCM1. Bake-off only — must beat the current winner to ship.

use crate::ans::{RansEnc, RANS_L, SCALE};
use crate::experts::{classify, Family, N_EXPERTS, TOP_K};

pub const MAGIC: &[u8; 4] = b"NCM1";
const P_HALF: u32 = SCALE / 2;
const ADAPT: u32 = 5; // p += (bit*SCALE - p) >> ADAPT

#[derive(Clone)]
struct Expert {
    id: u8,
    family: Family,
    table: [u16; 4096],
}

impl Expert {
    fn new(id: u8) -> Self {
        let family = match id {
            0..=9 => Family::Text,
            10..=19 => Family::Exe,
            20..=29 => Family::Dicom,
            30..=39 => Family::Markup,
            40..=49 => Family::Structured,
            _ => Family::Generic,
        };
        Self {
            id,
            family,
            table: [2048; 4096],
        }
    }

    fn matches(&self, last: u8, n: usize) -> bool {
        if n < 1 {
            return false;
        }
        match self.family {
            Family::Text => last.is_ascii_graphic() || last == b' ' || last == b'\n',
            Family::Exe => last == 0x90 || last == 0xE8 || last == 0xE9 || last == b'M',
            Family::Dicom => last == 0 || n > 128,
            Family::Markup => last == b'<' || last == b'>' || last == b'/',
            Family::Structured => n >= 4,
            Family::Generic => true,
        }
    }

    fn ctx(&self, win: u32, last: u8) -> usize {
        let mix = win
            .wrapping_mul(0x9E37_79B1)
            .wrapping_add(last as u32)
            .wrapping_add(self.id as u32 * 17);
        (mix as usize) & 4095
    }

    fn p(&self, win: u32, last: u8, n: usize) -> Option<u32> {
        if !self.matches(last, n) {
            return None;
        }
        Some(self.table[self.ctx(win, last)] as u32)
    }

    fn update(&mut self, win: u32, last: u8, n: usize, bit: u8) {
        if !self.matches(last, n) {
            return;
        }
        let i = self.ctx(win, last);
        let p = self.table[i] as u32;
        let target = if bit == 1 { SCALE } else { 0 };
        self.table[i] = (p + ((target.wrapping_sub(p)) >> ADAPT)) as u16;
        if self.table[i] == 0 {
            self.table[i] = 1;
        }
        if self.table[i] as u32 >= SCALE {
            self.table[i] = (SCALE - 1) as u16;
        }
    }
}

fn mix(ps: &[u32], ws: &[i32]) -> u32 {
    let mut num = 0i64;
    let mut den = 0i64;
    for (&p, &w) in ps.iter().zip(ws.iter()) {
        let ww = w.max(1) as i64;
        num += ww * p as i64;
        den += ww;
    }
    if den <= 0 {
        return P_HALF;
    }
    (num / den).clamp(1, (SCALE - 1) as i64) as u32
}

fn pick_ids(data: &[u8]) -> [u8; TOP_K] {
    let act = classify(data);
    let mut ids = [50u8, 51, 52, 53, 54];
    for i in 0..act.n.min(TOP_K) {
        ids[i] = act.hits[i].id;
    }
    // always keep one generic so mix is never empty
    if act.n < TOP_K {
        ids[act.n.min(TOP_K - 1)] = 50;
    }
    ids
}

pub fn encode(data: &[u8]) -> Vec<u8> {
    if data.is_empty() {
        let mut o = MAGIC.to_vec();
        o.extend_from_slice(&0u32.to_le_bytes());
        o.push(TOP_K as u8);
        o.extend_from_slice(&[50u8; 5]);
        o.extend_from_slice(&0u32.to_le_bytes());
        return o;
    }
    let ids = pick_ids(data);
    encode_real(data, ids)
}

fn encode_real(data: &[u8], ids: [u8; TOP_K]) -> Vec<u8> {
    let mut experts: Vec<Expert> = ids.iter().map(|&id| Expert::new(id)).collect();
    let mut ws = [8i32; TOP_K];
    let mut rec: Vec<(u8, u32)> = Vec::with_capacity(data.len() * 8);
    let mut win = 0u32;
    let mut last = 0u8;
    let mut n = 0usize;

    for &byte in data {
        for k in (0..8).rev() {
            let bit = (byte >> k) & 1;
            let mut ps = [P_HALF; TOP_K];
            for (i, e) in experts.iter().enumerate() {
                if let Some(p) = e.p(win, last, n) {
                    ps[i] = p.clamp(1, SCALE - 1);
                }
            }
            let p1 = mix(&ps, &ws);
            rec.push((bit, p1));
            let err = if bit == 1 {
                SCALE as i32 - p1 as i32
            } else {
                -(p1 as i32)
            };
            for i in 0..TOP_K {
                ws[i] += (err * (ps[i] as i32 - P_HALF as i32)) >> 16;
                ws[i] = ws[i].clamp(1, 4096);
                experts[i].update(win, last, n, bit);
            }
        }
        win = (win << 8) | byte as u32;
        last = byte;
        n += 1;
    }

    // rANS is LIFO: encode last bit first so decode walks the file forward.
    let mut stream = Vec::with_capacity(data.len());
    let mut enc = RansEnc::new();
    for &(bit, p1) in rec.iter().rev() {
        let (freq, cum) = if bit == 1 {
            (p1, 0)
        } else {
            (SCALE - p1, p1)
        };
        enc.encode(freq, cum, SCALE, &mut stream);
    }
    enc.flush(&mut stream);

    let mut out = Vec::with_capacity(16 + stream.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.push(TOP_K as u8);
    out.extend_from_slice(&ids);
    out.extend_from_slice(&(stream.len() as u32).to_le_bytes());
    out.extend_from_slice(&stream);
    out
}

pub fn decode(buf: &[u8]) -> Result<Vec<u8>, &'static str> {
    if buf.len() < 4 + 4 + 1 + TOP_K + 4 || &buf[..4] != MAGIC {
        return Err("ncm1: magic");
    }
    let orig = u32::from_le_bytes(buf[4..8].try_into().unwrap()) as usize;
    if orig == 0 {
        return Ok(Vec::new());
    }
    let n_act = buf[8] as usize;
    if n_act != TOP_K {
        return Err("ncm1: k");
    }
    let mut ids = [0u8; TOP_K];
    ids.copy_from_slice(&buf[9..9 + TOP_K]);
    let slen = u32::from_le_bytes(buf[9 + TOP_K..13 + TOP_K].try_into().unwrap()) as usize;
    let stream = &buf[13 + TOP_K..];
    if stream.len() < slen || slen < 8 {
        return Err("ncm1: stream");
    }
    let stream = &stream[..slen];

    let mut experts: Vec<Expert> = ids.iter().map(|&id| Expert::new(id.min((N_EXPERTS - 1) as u8))).collect();
    let mut ws = [8i32; TOP_K];

    let mut cursor = slen;
    let mut state = u64::from_le_bytes(stream[slen - 8..slen].try_into().unwrap());
    cursor -= 8;
    while state < RANS_L && cursor > 0 {
        cursor -= 1;
        state = (state << 8) | stream[cursor] as u64;
    }

    let mut out = vec![0u8; orig];
    let mut win = 0u32;
    let mut last = 0u8;
    let mut n = 0usize;

    for i in 0..orig {
        let mut byte = 0u8;
        for _k in 0..8 {
            let mut ps = [P_HALF; TOP_K];
            for (j, e) in experts.iter().enumerate() {
                if let Some(p) = e.p(win, last, n) {
                    ps[j] = p.clamp(1, SCALE - 1);
                }
            }
            let p1 = mix(&ps, &ws);
            let slot = (state & (SCALE as u64 - 1)) as u32;
            let bit = if slot < p1 { 1u8 } else { 0u8 };
            let (freq, cum) = if bit == 1 {
                (p1, 0)
            } else {
                (SCALE - p1, p1)
            };
            state = freq as u64 * (state >> 12) + (state & (SCALE as u64 - 1)) - cum as u64;
            while state < RANS_L {
                if cursor == 0 {
                    break;
                }
                cursor -= 1;
                state = (state << 8) | stream[cursor] as u64;
            }
            byte = (byte << 1) | bit;
            let err = if bit == 1 {
                SCALE as i32 - p1 as i32
            } else {
                -(p1 as i32)
            };
            for j in 0..TOP_K {
                ws[j] += (err * (ps[j] as i32 - P_HALF as i32)) >> 16;
                ws[j] = ws[j].clamp(1, 4096);
                experts[j].update(win, last, n, bit);
            }
        }
        out[i] = byte;
        win = (win << 8) | byte as u32;
        last = byte;
        n += 1;
    }
    Ok(out)
}

pub fn emit_thin(data: &[u8]) -> Vec<u8> {
    encode(data)
}

pub fn parse_thin(buf: &[u8]) -> Result<Vec<u8>, &'static str> {
    decode(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ncm1_roundtrip_text() {
        let src = b"Mr Pickwick walked into town and saw the inn.\n".repeat(40);
        let enc = encode(&src);
        let back = decode(&enc).expect("dec");
        assert_eq!(back, src.as_slice());
    }

    #[test]
    #[test]
    fn ncm1_size_probe() {
        let paths = [
            "/home/workdir/artifacts/corpora/obj1",
            "/tmp/dickens900.bin",
            "/home/workdir/artifacts/corpora/geo",
        ];
        for p in paths {
            let src = match std::fs::read(p) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let enc = encode(&src);
            let back = decode(&enc).expect("rt");
            assert_eq!(back, src);
            eprintln!("NCM1 {} raw={} packed={}", p, src.len(), enc.len());
        }
    }

    fn ncm1_roundtrip_binary() {
        let src: Vec<u8> = (0u8..=255).cycle().take(2048).collect();
        let enc = encode(&src);
        let back = decode(&enc).expect("dec");
        assert_eq!(back, src);
    }
}
