//! Order-0 rANS. Real encoder + decoder.
//!
//! Header is sparse so peaked MTF residuals stay cheap.
//! `Op::Lz` is still DEFLATE. `Op::Ans` is this.

pub const MAGIC: &[u8; 4] = b"AN0\0";
pub const MAGIC1: &[u8; 4] = b"AN1\0";
pub const CTX1: usize = 64;
pub const RANS_L: u64 = 1 << 23;
pub const SCALE_BITS: u32 = 12;
pub const SCALE: u32 = 1 << SCALE_BITS;

pub struct RansEnc {
    pub state: u64,
}

impl RansEnc {
    pub fn new() -> Self {
        Self { state: RANS_L }
    }

    pub fn encode(&mut self, freq: u32, cum: u32, scale: u32, out: &mut Vec<u8>) {
        let freq = freq.max(1);
        let x_max = ((RANS_L / scale as u64) << 8) * freq as u64;
        while self.state >= x_max {
            out.push((self.state & 0xff) as u8);
            self.state >>= 8;
        }
        self.state = (self.state / freq as u64) * scale as u64
            + (self.state % freq as u64)
            + cum as u64;
    }

    pub fn flush(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.state.to_le_bytes());
    }
}

fn normalize(counts: &[u32; 256]) -> ([u32; 256], [u32; 256]) {
    let total: u32 = counts.iter().sum();
    let mut freq = [0u32; 256];
    if total == 0 {
        freq[0] = SCALE;
        let mut start = [0u32; 256];
        return (freq, start);
    }
    let mut used = 0u32;
    for c in counts {
        if *c > 0 {
            used += 1;
        }
    }
    let mut assigned = 0u32;
    for (s, &c) in counts.iter().enumerate() {
        if c == 0 {
            continue;
        }
        let f = ((c as u64 * SCALE as u64) / total as u64).max(1) as u32;
        freq[s] = f;
        assigned += f;
    }
    // Force sum == SCALE. Steal from / give to the largest bin.
    if assigned != SCALE {
        let mut big = 0usize;
        for s in 0..256 {
            if freq[s] > freq[big] {
                big = s;
            }
        }
        if assigned < SCALE {
            freq[big] += SCALE - assigned;
        } else {
            let extra = assigned - SCALE;
            if freq[big] > extra + 1 {
                freq[big] -= extra;
            } else {
                // spread the cut
                let mut left = extra;
                for s in 0..256 {
                    if left == 0 {
                        break;
                    }
                    if freq[s] > 1 {
                        let take = (freq[s] - 1).min(left);
                        freq[s] -= take;
                        left -= take;
                    }
                }
            }
        }
    }
    let _ = used;
    let mut start = [0u32; 256];
    let mut run = 0u32;
    for s in 0..256 {
        start[s] = run;
        run += freq[s];
    }
    debug_assert_eq!(run, SCALE);
    (freq, start)
}

fn find_sym(slot: u32, freq: &[u32; 256], start: &[u32; 256]) -> u8 {
    for s in 0..256 {
        if freq[s] == 0 {
            continue;
        }
        if slot >= start[s] && slot < start[s] + freq[s] {
            return s as u8;
        }
    }
    0
}

pub fn rans_encode(data: &[u8]) -> Vec<u8> {
    let mut counts = [0u32; 256];
    for &b in data {
        counts[b as usize] += 1;
    }
    let (freq, start) = normalize(&counts);
    let mut enc = RansEnc::new();
    let mut stream = Vec::with_capacity(data.len() / 2 + 16);
    for &b in data.iter().rev() {
        let s = b as usize;
        enc.encode(freq[s], start[s], SCALE, &mut stream);
    }
    enc.flush(&mut stream);

    let used: Vec<(u8, u16)> = (0..256)
        .filter(|&s| freq[s] > 0)
        .map(|s| (s as u8, freq[s] as u16))
        .collect();

    let mut out = Vec::with_capacity(16 + used.len() * 3 + stream.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.push(SCALE_BITS as u8);
    out.extend_from_slice(&(used.len() as u16).to_le_bytes());
    for (s, f) in used {
        out.push(s);
        out.extend_from_slice(&f.to_le_bytes());
    }
    out.extend_from_slice(&(stream.len() as u32).to_le_bytes());
    out.extend_from_slice(&stream);
    out
}

fn ctx1(prev: u8) -> usize {
    (prev as usize).min(CTX1 - 1)
}

fn normalize_row(counts: &[u32; 256]) -> ([u32; 256], [u32; 256]) {
    normalize(counts)
}

pub fn rans_encode_order1(data: &[u8]) -> Vec<u8> {
    if data.is_empty() {
        return rans_encode(data);
    }
    let mut counts = vec![[0u32; 256]; CTX1];
    let mut prev = 0u8;
    for &b in data {
        counts[ctx1(prev)][b as usize] += 1;
        prev = b;
    }
    // Laplace on observed symbols in that context.
    for row in counts.iter_mut() {
        for c in row.iter_mut() {
            if *c > 0 {
                *c += 1;
            }
        }
        // empty context stays all-zero; decoder uses the order-0 table.
    }
    let mut global = [0u32; 256];
    for &b in data {
        global[b as usize] += 1;
    }
    let global_tbl = normalize_row(&global);
    let mut tables = Vec::with_capacity(CTX1);
    for row in &counts {
        if row.iter().all(|&c| c == 0) {
            tables.push(global_tbl);
        } else {
            tables.push(normalize_row(row));
        }
    }

    let mut prevs = vec![0u8; data.len()];
    prev = 0;
    for (i, &b) in data.iter().enumerate() {
        prevs[i] = prev;
        prev = b;
    }
    let mut enc = RansEnc::new();
    let mut stream = Vec::with_capacity(data.len() / 2 + 16);
    for i in (0..data.len()).rev() {
        let c = ctx1(prevs[i]);
        let s = data[i] as usize;
        let (freq, start) = &tables[c];
        enc.encode(freq[s], start[s], SCALE, &mut stream);
    }
    enc.flush(&mut stream);

    let mut out = Vec::with_capacity(32 + stream.len());
    out.extend_from_slice(MAGIC1);
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.push(SCALE_BITS as u8);
    out.push(CTX1 as u8);
    // order-0 fallback table first
    {
        let (freq, _) = &global_tbl;
        let used: Vec<(u8, u16)> = (0..256)
            .filter(|&s| freq[s] > 0)
            .map(|s| (s as u8, freq[s] as u16))
            .collect();
        out.extend_from_slice(&(used.len() as u16).to_le_bytes());
        for (s, f) in used {
            out.push(s);
            out.extend_from_slice(&f.to_le_bytes());
        }
    }
    for (ci, (freq, _)) in tables.iter().enumerate() {
        let empty = counts[ci].iter().all(|&c| c == 0);
        if empty {
            out.extend_from_slice(&0u16.to_le_bytes());
            continue;
        }
        let used: Vec<(u8, u16)> = (0..256)
            .filter(|&s| freq[s] > 0)
            .map(|s| (s as u8, freq[s] as u16))
            .collect();
        out.extend_from_slice(&(used.len() as u16).to_le_bytes());
        for (s, f) in used {
            out.push(s);
            out.extend_from_slice(&f.to_le_bytes());
        }
    }
    out.extend_from_slice(&(stream.len() as u32).to_le_bytes());
    out.extend_from_slice(&stream);
    out
}

pub fn rans_decode_order1(buf: &[u8]) -> Result<Vec<u8>, &'static str> {
    if buf.len() < 4 + 4 + 1 + 1 + 4 + 8 {
        return Err("ans1: truncated");
    }
    if &buf[..4] != MAGIC1 {
        return Err("ans1: bad magic");
    }
    let orig_len = u32::from_le_bytes(buf[4..8].try_into().unwrap()) as usize;
    if buf[8] as u32 != SCALE_BITS {
        return Err("ans1: scale");
    }
    let n_ctx = buf[9] as usize;
    if n_ctx != CTX1 {
        return Err("ans1: ctx");
    }
    let mut pos = 10usize;
    let mut freq = vec![[0u32; 256]; CTX1];
    let mut start = vec![[0u32; 256]; CTX1];
    if pos + 2 > buf.len() {
        return Err("ans1: global n_used");
    }
    let g_used = u16::from_le_bytes(buf[pos..pos + 2].try_into().unwrap()) as usize;
    pos += 2;
    let mut gfreq = [0u32; 256];
    for _ in 0..g_used {
        if pos + 3 > buf.len() {
            return Err("ans1: global freq");
        }
        let s = buf[pos] as usize;
        let f = u16::from_le_bytes(buf[pos + 1..pos + 3].try_into().unwrap()) as u32;
        gfreq[s] = f;
        pos += 3;
    }
    let mut grun = 0u32;
    let mut gstart = [0u32; 256];
    for s in 0..256 {
        gstart[s] = grun;
        grun += gfreq[s];
    }
    if grun != SCALE {
        return Err("ans1: global sum");
    }
    for c in 0..CTX1 {
        if pos + 2 > buf.len() {
            return Err("ans1: n_used");
        }
        let n_used = u16::from_le_bytes(buf[pos..pos + 2].try_into().unwrap()) as usize;
        pos += 2;
        if n_used == 0 {
            freq[c] = gfreq;
            start[c] = gstart;
            continue;
        }
        for _ in 0..n_used {
            if pos + 3 > buf.len() {
                return Err("ans1: freq");
            }
            let s = buf[pos] as usize;
            let f = u16::from_le_bytes(buf[pos + 1..pos + 3].try_into().unwrap()) as u32;
            freq[c][s] = f;
            pos += 3;
        }
        let mut run = 0u32;
        for s in 0..256 {
            start[c][s] = run;
            run += freq[c][s];
        }
        if run != SCALE {
            return Err("ans1: freq sum");
        }
    }
    if pos + 4 > buf.len() {
        return Err("ans1: stream len");
    }
    let slen = u32::from_le_bytes(buf[pos..pos + 4].try_into().unwrap()) as usize;
    pos += 4;
    if pos + slen > buf.len() {
        return Err("ans1: stream");
    }
    let stream = &buf[pos..pos + slen];
    if slen < 8 {
        return Err("ans1: no state");
    }
    let mut state = u64::from_le_bytes(stream[slen - 8..].try_into().unwrap());
    let mut cursor = slen - 8;
    let mask = (SCALE - 1) as u64;
    let mut out = vec![0u8; orig_len];
    let mut prev = 0u8;
    for i in 0..orig_len {
        let c = ctx1(prev);
        let slot = (state & mask) as u32;
        let s = find_sym(slot, &freq[c], &start[c]);
        out[i] = s;
        let f = freq[c][s as usize] as u64;
        let cum = start[c][s as usize] as u64;
        state = f * (state >> SCALE_BITS) + (state & mask) - cum;
        while state < RANS_L {
            if cursor == 0 {
                return Err("ans1: underrun");
            }
            cursor -= 1;
            state = (state << 8) | stream[cursor] as u64;
        }
        prev = s;
    }
    Ok(out)
}

/// Order-1 if smaller, else order-0.
pub fn rans_encode_auto(data: &[u8]) -> Vec<u8> {
    let a0 = rans_encode(data);
    if data.len() < 64 {
        return a0;
    }
    let a1 = rans_encode_order1(data);
    if a1.len() < a0.len() {
        a1
    } else {
        a0
    }
}

pub fn rans_decode(buf: &[u8]) -> Result<Vec<u8>, &'static str> {
    if buf.len() >= 4 && &buf[..4] == crate::mtf_binary::MAGIC {
        return crate::mtf_binary::decode(buf);
    }
    if buf.len() >= 4 && &buf[..4] == crate::clean_o32::MAGIC {
        return crate::clean_o32::decode(buf);
    }
    if buf.len() >= 4 && &buf[..4] == crate::clean_o24::MAGIC {
        return crate::clean_o24::decode(buf);
    }
    if buf.len() >= 4 && &buf[..4] == crate::clean_o16::MAGIC {
        return crate::clean_o16::decode(buf);
    }
    if buf.len() >= 4 && &buf[..4] == crate::clean_o8::MAGIC {
        return crate::clean_o8::decode(buf);
    }
    if buf.len() >= 4 && &buf[..4] == crate::clean_o1::MAGIC {
        return crate::clean_o1::decode(buf);
    }
    if buf.len() >= 4 && &buf[..4] == crate::order1_ans::MAGIC4 {
        return crate::order1_ans::decode(buf);
    }
    if buf.len() >= 4 && &buf[..4] == MAGIC1 {
        return rans_decode_order1(buf);
    }
    if buf.len() < 4 + 4 + 1 + 2 + 4 + 8 {
        return Err("ans: truncated");
    }
    if &buf[..4] != MAGIC {
        return Err("ans: bad magic");
    }
    let orig_len = u32::from_le_bytes(buf[4..8].try_into().unwrap()) as usize;
    let scale_bits = buf[8] as u32;
    if scale_bits != SCALE_BITS {
        return Err("ans: scale");
    }
    let n_used = u16::from_le_bytes(buf[9..11].try_into().unwrap()) as usize;
    let mut pos = 11usize;
    let mut freq = [0u32; 256];
    for _ in 0..n_used {
        if pos + 3 > buf.len() {
            return Err("ans: freq table");
        }
        let s = buf[pos] as usize;
        let f = u16::from_le_bytes(buf[pos + 1..pos + 3].try_into().unwrap()) as u32;
        freq[s] = f;
        pos += 3;
    }
    if pos + 4 > buf.len() {
        return Err("ans: stream len");
    }
    let slen = u32::from_le_bytes(buf[pos..pos + 4].try_into().unwrap()) as usize;
    pos += 4;
    if pos + slen > buf.len() {
        return Err("ans: stream");
    }
    let stream = &buf[pos..pos + slen];
    if slen < 8 {
        return Err("ans: no state");
    }
    let mut state = u64::from_le_bytes(stream[slen - 8..].try_into().unwrap());
    let mut cursor = slen - 8;
    let mut start = [0u32; 256];
    let mut run = 0u32;
    for s in 0..256 {
        start[s] = run;
        run += freq[s];
    }
    if run != SCALE {
        return Err("ans: freq sum");
    }
    let mask = (SCALE - 1) as u64;
    let mut out = vec![0u8; orig_len];
    for i in 0..orig_len {
        let slot = (state & mask) as u32;
        let s = find_sym(slot, &freq, &start);
        out[i] = s;
        let f = freq[s as usize] as u64;
        let c = start[s as usize] as u64;
        state = f * (state >> SCALE_BITS) + (state & mask) - c;
        while state < RANS_L {
            if cursor == 0 {
                return Err("ans: underrun");
            }
            cursor -= 1;
            state = (state << 8) | stream[cursor] as u64;
        }
    }
    Ok(out)
}

pub fn rans_decode_len(buf: &[u8], orig_len: usize) -> Result<Vec<u8>, &'static str> {
    let v = rans_decode(buf)?;
    if orig_len != 0 && v.len() != orig_len {
        return Err("ans: length");
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rt(src: &[u8]) {
        let c = rans_encode(src);
        let d = rans_decode(&c).expect("decode");
        assert_eq!(d, src, "ans roundtrip n={}", src.len());
    }

    #[test]
    fn hello() {
        rt(b"hello world this is a rANS test ");
        rt(&b"mississippi ".repeat(80));
    }

    #[test]
    fn zeros_and_alphabet() {
        rt(&vec![0u8; 1000]);
        let a: Vec<u8> = (0..=255).collect();
        rt(&a);
        rt(&a.repeat(4));
    }

    #[test]
    fn empty_and_one() {
        rt(b"");
        rt(b"x");
    }

    #[test]
    fn order1_mtf_like() {
        let mut src = Vec::new();
        for _ in 0..400 {
            src.extend_from_slice(&[0, 0, 0, 0, 1, 0, 0, 2, 0, 1, 0, 0]);
        }
        let c1 = rans_encode_order1(&src);
        let d = rans_decode_order1(&c1).expect("o1");
        assert_eq!(d, src);
        let auto = rans_encode_auto(&src);
        assert_eq!(rans_decode(&auto).unwrap(), src);
    }
}
