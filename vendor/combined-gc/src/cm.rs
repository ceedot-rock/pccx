//! Context mixer: o0 o1 o2 o4 o5 + match + bwt_rank.
//! Adaptive linear mix, lr=0.02 per bit. Forward range coder.
//!
//! Adaptive models cannot use reverse rANS (decoder would need P[n-1]
//! first). Range coder is the same ANS family, forward both ways.

const N_MODELS: usize = 8; // o0 o1 o2 o4 o5 match rank hangry
const LR: f32 = 0.06;
const MATCH_WIN: usize = 32_768;
const MATCH_HASH: usize = 1 << 16;
const SCALE: u32 = 4096;
const BOT: u32 = 1 << 16;

pub const MAGIC: &[u8; 4] = b"CM11";

struct RangeEnc {
    low: u32,
    high: u32,
    out: Vec<u8>,
}

impl RangeEnc {
    fn new() -> Self {
        Self {
            low: 0,
            high: 0xffff_ffff,
            out: Vec::new(),
        }
    }
    fn encode_bit(&mut self, bit: u8, p1: f32) {
        let p1 = p1.clamp(0.002, 0.998);
        let f1 = ((p1 * SCALE as f32) as u64).clamp(1, SCALE as u64 - 1);
        let span = self.high.wrapping_sub(self.low) as u64;
        let mid = self
            .low
            .wrapping_add((((span + 1) * (SCALE as u64 - f1)) / SCALE as u64) as u32);
        if bit == 0 {
            self.high = mid.wrapping_sub(1);
        } else {
            self.low = mid;
        }
        self.norm();
    }
    fn norm(&mut self) {
        while (self.low ^ self.high) < 0x0100_0000 {
            self.out.push((self.low >> 24) as u8);
            self.low <<= 8;
            self.high = (self.high << 8) | 0xff;
        }
    }
    fn finish(mut self) -> Vec<u8> {
        for _ in 0..4 {
            self.out.push((self.low >> 24) as u8);
            self.low <<= 8;
        }
        self.out
    }
}

struct RangeDec<'a> {
    low: u32,
    high: u32,
    code: u32,
    src: &'a [u8],
    pos: usize,
}

impl<'a> RangeDec<'a> {
    fn new(src: &'a [u8]) -> Result<Self, &'static str> {
        if src.len() < 4 {
            return Err("cm: short");
        }
        let mut code = 0u32;
        let mut pos = 0;
        for _ in 0..4 {
            code = (code << 8) | src[pos] as u32;
            pos += 1;
        }
        Ok(Self {
            low: 0,
            high: 0xffff_ffff,
            code,
            src,
            pos,
        })
    }
    fn decode_bit(&mut self, p1: f32) -> Result<u8, &'static str> {
        let p1 = p1.clamp(0.002, 0.998);
        let f1 = ((p1 * SCALE as f32) as u64).clamp(1, SCALE as u64 - 1);
        let span = self.high.wrapping_sub(self.low) as u64;
        let mid = self
            .low
            .wrapping_add((((span + 1) * (SCALE as u64 - f1)) / SCALE as u64) as u32);
        let bit = if self.code.wrapping_sub(self.low) < mid.wrapping_sub(self.low) {
            0u8
        } else {
            1u8
        };
        if bit == 0 {
            self.high = mid.wrapping_sub(1);
        } else {
            self.low = mid;
        }
        self.norm()?;
        Ok(bit)
    }
    fn norm(&mut self) -> Result<(), &'static str> {
        while (self.low ^ self.high) < 0x0100_0000 {
            if self.pos >= self.src.len() {
                return Err("cm: underrun");
            }
            self.code = (self.code << 8) | self.src[self.pos] as u32;
            self.pos += 1;
            self.low <<= 8;
            self.high = (self.high << 8) | 0xff;
        }
        Ok(())
    }
}

fn hash_bytes(b: &[u8]) -> u32 {
    let mut h = 2166136261u32;
    for &x in b {
        h ^= x as u32;
        h = h.wrapping_mul(16777619);
    }
    h
}

#[derive(Clone, Copy)]
struct BitCtr {
    n0: u16,
    n1: u16,
}

impl BitCtr {
    fn p1(&self) -> f32 {
        let a = self.n0 as f32 + 0.5;
        let b = self.n1 as f32 + 0.5;
        b / (a + b)
    }
    fn update(&mut self, bit: u8) {
        if bit == 1 {
            self.n1 = self.n1.saturating_add(1);
        } else {
            self.n0 = self.n0.saturating_add(1);
        }
        // age so recent bits matter
        if self.n0 as u32 + self.n1 as u32 > 48 {
            self.n0 = (self.n0 >> 1).max(1);
            self.n1 = (self.n1 >> 1).max(1);
        }
    }
}

struct Model {
    o0: [BitCtr; 256],
    o1: Vec<BitCtr>, // 256 * 256
    o2: Vec<BitCtr>, // 1<<16
    o4: Vec<BitCtr>,
    o5: Vec<BitCtr>,
    rank: Vec<BitCtr>, // 256 * 256  (rank_bucket << 8 | partial)
}

impl Model {
    fn new() -> Self {
        let z = BitCtr { n0: 1, n1: 1 };
        Self {
            o0: [z; 256],
            o1: vec![z; 256 * 256],
            o2: vec![z; 1 << 16],
            o4: vec![z; 1 << 16],
            o5: vec![z; 1 << 16],
            rank: vec![z; 256 * 256],
        }
    }
}

#[inline]
fn stretch(p: f32) -> f32 {
    let p = p.clamp(0.0001, 0.9999);
    (p / (1.0 - p)).ln()
}

#[inline]
fn squash(s: f32) -> f32 {
    1.0 / (1.0 + (-s.clamp(-16.0, 16.0)).exp())
}

struct Mixer {
    w: [f32; N_MODELS],
}

impl Mixer {
    fn new() -> Self {
        // o0 + match start live; rest learn.
        let mut w = [0.0; N_MODELS];
        w[0] = 0.4;
        w[5] = 1.2;
        Self { w }
    }
    fn mix(&self, p: &[f32; N_MODELS]) -> f32 {
        let mut s = 0.0;
        for i in 0..N_MODELS {
            s += self.w[i] * stretch(p[i]);
        }
        squash(s).clamp(0.002, 0.998)
    }
    fn update(&mut self, p: &[f32; N_MODELS], bit: u8, pm: f32) {
        let err = bit as f32 - pm;
        for i in 0..N_MODELS {
            self.w[i] = (self.w[i] + LR * err * stretch(p[i])).clamp(-8.0, 8.0);
        }
    }
}

/// SSE / APM: refine p using last-byte context + quantized logit.
struct Apm {
    t: [u16; 64 * 33],
}

impl Apm {
    fn new() -> Self {
        Self { t: [2048; 64 * 33] }
    }
    fn predict(&self, p: f32, ctx: usize) -> f32 {
        let x = ((stretch(p) + 8.0) * 2.0).clamp(0.0, 31.999);
        let i = x as usize;
        let w = x - i as f32;
        let base = (ctx & 63) * 33;
        let a = self.t[base + i] as f32 / 4096.0;
        let b = self.t[base + i + 1] as f32 / 4096.0;
        (a + (b - a) * w).clamp(0.002, 0.998)
    }
    fn update(&mut self, p: f32, ctx: usize, bit: u8) {
        let x = ((stretch(p) + 8.0) * 2.0).clamp(0.0, 31.999);
        let i = x as usize;
        let base = (ctx & 63) * 33;
        for off in [i, i + 1] {
            let cur = self.t[base + off] as i32;
            let target = if bit == 1 { 4095 } else { 1 };
            self.t[base + off] = (cur + (target - cur) / 16) as u16;
        }
    }
}

fn rank_of(hist: &[u8], b: u8) -> u8 {
    let take = hist.len().min(256);
    if take == 0 {
        return 0;
    }
    let sl = &hist[hist.len() - take..];
    let mut r = 0u32;
    for &x in sl {
        if x < b {
            r += 1;
        }
    }
    ((r * 255) / take as u32) as u8
}

fn hash4(hist: &[u8], i: usize) -> usize {
    if i + 3 >= hist.len() {
        return 0;
    }
    let x = u32::from_le_bytes([hist[i], hist[i + 1], hist[i + 2], hist[i + 3]]);
    (x.wrapping_mul(0x1e35a7bd) >> 16) as usize & (MATCH_HASH - 1)
}

struct State {
    model: Model,
    mix: Mixer,
    apm: Apm,
    hist: Vec<u8>,
    partial: u8,
    bits_in: u8,
    match_at: Option<usize>,
    head: Vec<i32>,
}

impl State {
    fn new() -> Self {
        Self {
            model: Model::new(),
            mix: Mixer::new(),
            apm: Apm::new(),
            hist: Vec::new(),
            partial: 0,
            bits_in: 0,
            match_at: None,
            head: vec![-1; MATCH_HASH],
        }
    }

    fn lookup_match(&self) -> Option<usize> {
        let n = self.hist.len();
        if n < 8 {
            return None;
        }
        let h = hash4(&self.hist, n - 4);
        let p = self.head[h];
        if p >= 0 {
            let src = p as usize;
            if src + 4 < n && self.hist[src..src + 4] == self.hist[n - 4..] {
                return Some(src + 4);
            }
        }
        None
    }

    fn insert_match(&mut self) {
        let n = self.hist.len();
        if n < 4 {
            return;
        }
        let start = n - 4;
        if n > MATCH_WIN + 4 {
            // old hashes simply age out; head may dangle, lookup checks slice
        }
        let h = hash4(&self.hist, start);
        self.head[h] = start as i32;
    }

    fn predict(&self) -> ([f32; N_MODELS], f32) {
        let part = ((self.bits_in as usize) << 5) | (self.partial as usize);
        let part8 = self.partial as usize;
        let last = *self.hist.last().unwrap_or(&0) as usize;
        let mut ctx2 = [0u8; 2];
        let n = self.hist.len();
        if n >= 2 {
            ctx2[0] = self.hist[n - 2];
            ctx2[1] = self.hist[n - 1];
        } else if n == 1 {
            ctx2[1] = self.hist[0];
        }
        let mut ctx4 = [0u8; 4];
        for k in 0..4 {
            if n > k {
                ctx4[3 - k] = self.hist[n - 1 - k];
            }
        }
        let mut ctx5 = [0u8; 5];
        for k in 0..5 {
            if n > k {
                ctx5[4 - k] = self.hist[n - 1 - k];
            }
        }
        let h2 = (hash_bytes(&ctx2) as usize ^ (part8 << 3)) & 0xffff;
        let h4 = (hash_bytes(&ctx4) as usize ^ (part8 << 5)) & 0xffff;
        let h5 = (hash_bytes(&ctx5) as usize ^ (part8 << 1)) & 0xffff;
        let rk = if n > 0 {
            rank_of(&self.hist, self.hist[n - 1])
        } else {
            0
        } as usize;

        let p0 = self.model.o0[part8].p1();
        let p1 = self.model.o1[last * 256 + part8].p1();
        let p2 = self.model.o2[h2].p1();
        let p4 = self.model.o4[h4].p1();
        let p5 = self.model.o5[h5].p1();
        let pm = if let Some(at) = self.match_at {
            if at < self.hist.len() {
                let nxt = self.hist[at];
                let shift = 7 - self.bits_in;
                if ((nxt >> shift) & 1) == 1 {
                    0.97
                } else {
                    0.03
                }
            } else {
                0.5
            }
        } else {
            0.5
        };
        let pr = self.model.rank[rk * 256 + part8].p1();
        let h = crate::models_live::hangry_predictor(&self.hist);
        let pred_b = (h * 255.0).round().clamp(0.0, 255.0) as u8;
        let shift = 7 - self.bits_in;
        let on = ((pred_b >> shift) & 1) == 1;
        let ph = if on {
            (0.5 + 0.4 * h as f32).clamp(0.05, 0.95)
        } else {
            (0.5 - 0.4 * h as f32).clamp(0.05, 0.95)
        };
        let ps = [p0, p1, p2, p4, p5, pm, pr, ph];
        let mixed = self.mix.mix(&ps);
        let ctx = last & 63;
        let sse = self.apm.predict(mixed, ctx);
        let p = if self.hist.len() < 24 {
            mixed
        } else {
            (0.65 * mixed + 0.35 * sse).clamp(0.002, 0.998)
        };
        let _ = part;
        (ps, p)
    }

    fn commit_bit(&mut self, bit: u8, ps: &[f32; N_MODELS], pm: f32) {
        self.mix.update(ps, bit, pm);
        let last = *self.hist.last().unwrap_or(&0) as usize;
        self.apm.update(pm, last & 63, bit);
        let part8 = self.partial as usize;
        let last = *self.hist.last().unwrap_or(&0) as usize;
        let n = self.hist.len();
        let mut ctx2 = [0u8; 2];
        if n >= 2 {
            ctx2[0] = self.hist[n - 2];
            ctx2[1] = self.hist[n - 1];
        } else if n == 1 {
            ctx2[1] = self.hist[0];
        }
        let mut ctx4 = [0u8; 4];
        for k in 0..4 {
            if n > k {
                ctx4[3 - k] = self.hist[n - 1 - k];
            }
        }
        let mut ctx5 = [0u8; 5];
        for k in 0..5 {
            if n > k {
                ctx5[4 - k] = self.hist[n - 1 - k];
            }
        }
        let h2 = (hash_bytes(&ctx2) as usize ^ (part8 << 3)) & 0xffff;
        let h4 = (hash_bytes(&ctx4) as usize ^ (part8 << 5)) & 0xffff;
        let h5 = (hash_bytes(&ctx5) as usize ^ (part8 << 1)) & 0xffff;
        let rk = if n > 0 {
            rank_of(&self.hist, self.hist[n - 1])
        } else {
            0
        } as usize;

        self.model.o0[part8].update(bit);
        self.model.o1[last * 256 + part8].update(bit);
        self.model.o2[h2].update(bit);
        self.model.o4[h4].update(bit);
        self.model.o5[h5].update(bit);
        self.model.rank[rk * 256 + part8].update(bit);

        self.partial = (self.partial << 1) | bit;
        self.bits_in += 1;
        if self.bits_in == 8 {
            self.hist.push(self.partial);
            if let Some(at) = self.match_at.as_mut() {
                *at += 1;
                if *at >= self.hist.len() {
                    self.match_at = None;
                }
            }
            if self.match_at.is_none() {
                self.match_at = self.lookup_match();
            }
            self.insert_match();
            self.partial = 0;
            self.bits_in = 0;
            if self.hist.len() > MATCH_WIN {
                let drop = self.hist.len() - MATCH_WIN;
                self.hist.drain(0..drop);
                self.match_at = None;
                self.head.fill(-1);
            }
        }
    }
}

pub fn cm_encode(data: &[u8]) -> Vec<u8> {
    let mut st = State::new();
    let mut enc = RangeEnc::new();
    for &b in data {
        for s in (0..8).rev() {
            let bit = (b >> s) & 1;
            let (ps, pm) = st.predict();
            enc.encode_bit(bit, pm);
            st.commit_bit(bit, &ps, pm);
        }
    }
    let stream = enc.finish();
    let mut out = Vec::with_capacity(8 + stream.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&stream);
    out
}

pub fn cm_decode(buf: &[u8]) -> Result<Vec<u8>, &'static str> {
    if buf.len() < 8 {
        return Err("cm: truncated");
    }
    if &buf[..4] != MAGIC {
        return Err("cm: magic");
    }
    let orig = u32::from_le_bytes(buf[4..8].try_into().unwrap()) as usize;
    let mut dec = RangeDec::new(&buf[8..])?;
    let mut st = State::new();
    let mut out = vec![0u8; orig];
    for i in 0..orig {
        let mut b = 0u8;
        for _ in 0..8 {
            let (ps, pm) = st.predict();
            let bit = dec.decode_bit(pm)?;
            b = (b << 1) | bit;
            st.commit_bit(bit, &ps, pm);
        }
        out[i] = b;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rt(src: &[u8]) {
        let c = cm_encode(src);
        let d = cm_decode(&c).expect("cm decode");
        assert_eq!(d, src);
    }

    #[test]
    fn hello() {
        rt(b"hello world this is mixer text ");
        rt(&b"mississippi ".repeat(40));
    }

    #[test]
    fn zeros_and_one() {
        rt(b"x");
        let z = vec![0u8; 200];
        let c = cm_encode(&z);
        assert!(c.len() < 80, "zeros cm {}", c.len());
        rt(&z);
    }

    #[test]
    fn hash_finds_repeat() {
        let mut st = State::new();
        let s = b"abcdabcd";
        for &b in s {
            for sft in (0..8).rev() {
                let bit = (b >> sft) & 1;
                let (ps, pm) = st.predict();
                st.commit_bit(bit, &ps, pm);
            }
        }
        assert!(
            st.match_at.is_some(),
            "hist={} match={:?}",
            st.hist.len(),
            st.match_at
        );
    }

    #[test]
    fn logit_roundtrip_functions() {
        for p in [0.01f32, 0.25, 0.5, 0.75, 0.99] {
            let q = squash(stretch(p));
            assert!((q - p).abs() < 1e-4, "{p} -> {q}");
        }
    }

    #[test]
    fn match_repeats_smaller_than_raw() {
        let src = b"the quick brown fox jumps over the lazy dog. ".repeat(200);
        let c = cm_encode(&src);
        assert!(
            c.len() + 8 < src.len(),
            "cm {} vs raw {}",
            c.len(),
            src.len()
        );
        rt(&src);
    }
}
