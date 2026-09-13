//! PCCX 0.3.0 — closed crate. PCC1. Pulsar BWT + Combined GC in-tree.

pub const VERSION: &str = "pccx-0.3.0";
pub const MAGIC: &[u8; 4] = b"PCC1";
pub const VER: u8 = 2;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Zero = 0,
    Match = 1,
    Bwt = 2,
    Store = 3,
    Aware = 12,
}

pub fn version() -> &'static str {
    VERSION
}

fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xffff_ffffu32;
    for &b in data {
        c ^= b as u32;
        for _ in 0..8 {
            let bit = c & 1;
            c >>= 1;
            if bit != 0 {
                c ^= 0xedb8_8320;
            }
        }
    }
    !c
}

fn put_u32(out: &mut Vec<u8>, n: u32) {
    out.extend_from_slice(&n.to_le_bytes());
}

pub fn is_pccx(buf: &[u8]) -> bool {
    buf.len() >= 17 && buf.starts_with(MAGIC) && buf[4] == VER
}

fn lz_tokens(data: &[u8]) -> Vec<u8> {
    const W: usize = 1 << 20;
    const H: usize = 1 << 16;
    let n = data.len();
    let mut head = vec![-1i32; H];
    let mut prev = vec![-1i32; n];
    let mut out = Vec::with_capacity(n / 2 + 16);
    out.extend_from_slice(&(n as u32).to_le_bytes());
    let mut i = 0usize;
    while i < n {
        let mut best_l = 0usize;
        let mut best_d = 0usize;
        if i + 3 < n {
            let h = u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]) as usize % H;
            let mut p = head[h];
            let floor = i.saturating_sub(W) as i32;
            let mut steps = 0u32;
            while p >= floor && steps < 64 {
                let j = p as usize;
                let mut l = 0usize;
                while i + l < n && j + l < i && data[i + l] == data[j + l] && l < 255 {
                    l += 1;
                }
                if l >= 4 && l > best_l {
                    best_l = l;
                    best_d = i - j;
                }
                p = prev[j];
                steps += 1;
            }
            prev[i] = head[h];
            head[h] = i as i32;
        }
        if best_l >= 4 {
            out.push(1);
            out.push(best_l as u8);
            out.extend_from_slice(&(best_d as u32).to_le_bytes());
            for k in 1..best_l {
                if i + k + 3 < n {
                    let h = u32::from_le_bytes([data[i + k], data[i + k + 1], data[i + k + 2], data[i + k + 3]]) as usize % H;
                    prev[i + k] = head[h];
                    head[h] = (i + k) as i32;
                }
            }
            i += best_l;
        } else {
            out.push(0);
            out.push(data[i]);
            i += 1;
        }
    }
    out
}

fn lz_detok(buf: &[u8]) -> Result<Vec<u8>, &'static str> {
    if buf.len() < 4 {
        return Err("lz");
    }
    let n = u32::from_le_bytes(buf[0..4].try_into().unwrap()) as usize;
    let mut i = 4usize;
    let mut out = Vec::with_capacity(n);
    while out.len() < n {
        if i >= buf.len() {
            return Err("lz trunc");
        }
        match buf[i] {
            0 => {
                i += 1;
                if i >= buf.len() {
                    return Err("lz lit");
                }
                out.push(buf[i]);
                i += 1;
            }
            1 => {
                i += 1;
                if i + 5 > buf.len() {
                    return Err("lz m");
                }
                let len = buf[i] as usize;
                i += 1;
                let d = u32::from_le_bytes(buf[i..i + 4].try_into().unwrap()) as usize;
                i += 4;
                if d == 0 || d > out.len() {
                    return Err("lz dist");
                }
                for _ in 0..len {
                    out.push(out[out.len() - d]);
                }
            }
            _ => return Err("lz op"),
        }
    }
    if out.len() != n {
        return Err("lz n");
    }
    Ok(out)
}

fn solid(data: &[u8]) -> Option<u8> {
    if data.is_empty() {
        return None;
    }
    let s = data[0];
    if data.iter().all(|&b| b == s) {
        Some(s)
    } else {
        None
    }
}

fn pack(raw_len: u32, crc: u32, op: Op, blob: &[u8]) -> Vec<u8> {
    let mut out = Vec::from(*MAGIC);
    out.push(VER);
    put_u32(&mut out, raw_len);
    put_u32(&mut out, crc);
    put_u32(&mut out, 1);
    out.push(op as u8);
    put_u32(&mut out, raw_len);
    put_u32(&mut out, blob.len() as u32);
    out.extend_from_slice(blob);
    out
}

pub fn encode(data: &[u8]) -> Option<Vec<u8>> {
    if data.is_empty() {
        return None;
    }
    let crc = crc32(data);
    let mut best_op = Op::Store;
    let mut best = data.to_vec();
    if let Some(s) = solid(data) {
        let b = vec![s];
        if b.len() < best.len() {
            best_op = Op::Zero;
            best = b;
        }
    }
    let lz = lz_tokens(data);
    if lz.len() < best.len() {
        best_op = Op::Match;
        best = lz;
    }
    let bwt = pulsar::bwt_ans::compress(data);
    if bwt.len() < best.len() {
        best_op = Op::Bwt;
        best = bwt;
    }
    let gc = combined_gc::frame::compress(data).bytes;
    let foreign = (gc.len() >= 2 && gc.starts_with(&[0x1f, 0x8b]))
        || (gc.len() >= 4 && (gc.starts_with(b"XZ1\0") || gc.starts_with(b"ZLB1") || gc.starts_with(b"BZh")));
    if !foreign && gc.len() < best.len() {
        if combined_gc::frame::decompress(&gc).ok().as_deref() == Some(data) {
            best_op = Op::Aware;
            best = gc;
        }
    }
    if best.len() >= data.len() && best_op != Op::Zero {
        return None;
    }
    let out = pack(data.len() as u32, crc, best_op, &best);
    if out.len() >= data.len() {
        return None;
    }
    match decode(&out) {
        Ok(back) if back == data => Some(out),
        _ => None,
    }
}

pub fn decode(buf: &[u8]) -> Result<Vec<u8>, &'static str> {
    if !is_pccx(buf) {
        return Err("pccx");
    }
    let raw_len = u32::from_le_bytes(buf[5..9].try_into().unwrap()) as usize;
    let crc = u32::from_le_bytes(buf[9..13].try_into().unwrap());
    let nb = u32::from_le_bytes(buf[13..17].try_into().unwrap());
    if nb != 1 {
        return Err("pccx blocks");
    }
    let op = buf[17];
    let bl = u32::from_le_bytes(buf[22..26].try_into().unwrap()) as usize;
    if 26 + bl != buf.len() {
        return Err("pccx len");
    }
    let blob = &buf[26..];
    let out = match op {
        0 => {
            if blob.len() != 1 {
                return Err("zero");
            }
            vec![blob[0]; raw_len]
        }
        1 => lz_detok(blob)?,
        2 => pulsar::bwt_ans::decompress(blob).map_err(|_| "bwt")?,
        12 => combined_gc::frame::decompress(blob)?,
        3 => {
            if blob.len() != raw_len {
                return Err("store");
            }
            blob.to_vec()
        }
        _ => return Err("op"),
    };
    if out.len() != raw_len {
        return Err("n");
    }
    if crc32(&out) != crc {
        return Err("crc");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zeros() {
        let s = vec![0u8; 1000];
        let e = encode(&s).unwrap();
        assert_eq!(decode(&e).unwrap(), s);
        assert!(e.len() < 40);
    }
    #[test]
    fn repeat() {
        let s = b"the cat sat on the mat. the cat sat. ".repeat(80);
        let e = encode(&s).unwrap();
        assert_eq!(decode(&e).unwrap(), s);
        assert!(e.len() < s.len());
    }
}
