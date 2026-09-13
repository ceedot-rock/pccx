//! GCR1: deterministic byte contexts + exact match + fixed-point calibration.
//!
//! The payload is intentionally self-contained and uses only decoded bytes as state.
//! It is a research profile, not a replacement for legacy Combined GC paths.

pub const MAGIC: &[u8; 4] = b"GCR1";
pub const VERSION: u8 = 1;
pub const FLAG_MATCH: u8 = 0x01;
pub const FLAG_CALIBRATION: u8 = 0x02;
pub const FLAG_SDF_RECORD: u8 = 0x04;
pub const FLAG_TAR_SPLIT: u8 = 0x08;
pub const FLAG_SDF_FIELD: u8 = 0x10;
pub const VALID_FLAGS: u8 = FLAG_MATCH | FLAG_CALIBRATION | FLAG_SDF_RECORD | FLAG_TAR_SPLIT | FLAG_SDF_FIELD;

const CONTEXT_BITS: usize = 21;
const CONTEXT_SIZE: usize = 1 << CONTEXT_BITS;
const CONTEXT_MASK: usize = CONTEXT_SIZE - 1;
const MATCH_BITS: usize = 21;
const MATCH_SIZE: usize = 1 << MATCH_BITS;
const MATCH_MASK: usize = MATCH_SIZE - 1;
const P_MIN: u32 = 64;
const P_MAX: u32 = 65_471;
const COUNT_LIMIT: u32 = 4_096;
const MIX_SCALE: i32 = 4_096;

#[derive(Clone, Copy, Default)]
struct CtxEntry {
    tag: u32,
    zeros: u16,
    ones: u16,
}

#[derive(Clone)]
struct Model {
    contexts: Vec<CtxEntry>,
    matches: Vec<u32>, // next-byte source position plus one; zero is empty.
    cal_zeros: [u16; 64],
    cal_ones: [u16; 64],
    match_weight: i32,
    record_phase: u16,
    sdf_field: u8,
    sdf_line_start: bool,
    sdf_in_name: bool,
    flags: u8,
}

impl Model {
    fn new(flags: u8) -> Result<Self, &'static str> {
        if flags & !VALID_FLAGS != 0 { return Err("gcr1: flags"); }
        Ok(Self {
            contexts: vec![CtxEntry::default(); CONTEXT_SIZE],
            matches: vec![0; MATCH_SIZE],
            cal_zeros: [1; 64],
            cal_ones: [1; 64],
            // Give the enabled expert a modest initial vote; online updates remain causal.
            match_weight: if flags & FLAG_MATCH != 0 { 512 } else { 0 },
            record_phase: 0,
            sdf_field: 0,
            sdf_line_start: true,
            sdf_in_name: false,
            flags,
        })
    }

    fn ctx_key(history: &[u8], byte_pos: usize, prefix: u16, record_phase: u16, sdf_field: u8) -> (usize, u32) {
        let mut state = 0u32;
        let start = history.len().saturating_sub(4);
        for &b in &history[start..] { state = state.rotate_left(5) ^ b as u32; }
        state ^= ((byte_pos as u32) & 15) << 24;
        state ^= ((record_phase & 31) as u32) << 8;
        state ^= (sdf_field as u32) << 16;
        state ^= (prefix as u32).wrapping_mul(0x9e37_79b9);
        state = state.wrapping_mul(0x85eb_ca6b).rotate_left(13);
        let tag = state | 1;
        (state as usize & CONTEXT_MASK, tag)
    }

    fn match_key(history: &[u8]) -> Option<(usize, [u8; 4])> {
        if history.len() < 4 { return None; }
        let n = history.len();
        let key = [history[n - 4], history[n - 3], history[n - 2], history[n - 1]];
        let x = u32::from_le_bytes(key).wrapping_mul(0x1e35_a7bd).rotate_left(11);
        Some((x as usize & MATCH_MASK, key))
    }

    fn probability_zero(&mut self, history: &[u8], byte_pos: usize, prefix: u16, bit: usize) -> (u32, bool, bool) {
        let phase = if self.flags & FLAG_SDF_RECORD != 0 { self.record_phase } else { 0 };
        let field = if self.flags & FLAG_SDF_FIELD != 0 { self.sdf_field } else { 0 };
        let (slot, tag) = Self::ctx_key(history, byte_pos, prefix, phase, field);
        let entry = &mut self.contexts[slot];
        if entry.tag != tag {
            *entry = CtxEntry { tag, zeros: 1, ones: 1 };
        }
        let denom = entry.zeros as u32 + entry.ones as u32;
        let p_ctx = clamp_p((entry.zeros as u32 * 65_536) / denom.max(1));

        let mut match_valid = false;
        let mut match_predicts_zero = false;
        let mut p = p_ctx;
        if self.flags & FLAG_MATCH != 0 {
            if let Some((slot, key)) = Self::match_key(history) {
                let stored = self.matches[slot];
                if stored != 0 {
                    let source = stored as usize - 1;
                    // source is the next byte after the stored four-byte suffix.
                    if source < history.len() && source >= 4 && history[source - 4..source] == key {
                        match_valid = true;
                        match_predicts_zero = ((history[source] >> (7 - bit)) & 1) == 0;
                        let p_match = if match_predicts_zero { 58_982 } else { 6_553 };
                        let w = self.match_weight.clamp(0, MIX_SCALE) as u32;
                        p = ((p_ctx as u64 * (MIX_SCALE as u64 - w as u64) + p_match as u64 * w as u64) >> 12) as u32;
                    }
                }
            }
        }
        if self.flags & FLAG_CALIBRATION != 0 {
            let bin = (p >> 10).min(63) as usize;
            let denom = self.cal_zeros[bin] as u32 + self.cal_ones[bin] as u32;
            let p_cal = clamp_p((self.cal_zeros[bin] as u32 * 65_536) / denom.max(1));
            // Calibration influence grows from zero to 25% over 64 observations.
            let a = ((denom.min(64) * 1024) / 64) as u32;
            p = ((p as u64 * (4096 - a) as u64 + p_cal as u64 * a as u64) >> 12) as u32;
        }
        (clamp_p(p), match_valid, match_predicts_zero)
    }

    fn update_bit(&mut self, history: &[u8], byte_pos: usize, prefix: u16, zero: bool, match_valid: bool, match_predicts_zero: bool, p_before_cal: u32) {
        let phase = if self.flags & FLAG_SDF_RECORD != 0 { self.record_phase } else { 0 };
        let field = if self.flags & FLAG_SDF_FIELD != 0 { self.sdf_field } else { 0 };
        let (slot, tag) = Self::ctx_key(history, byte_pos, prefix, phase, field);
        let entry = &mut self.contexts[slot];
        if entry.tag != tag { *entry = CtxEntry { tag, zeros: 1, ones: 1 }; }
        if zero { entry.zeros = entry.zeros.saturating_add(1); } else { entry.ones = entry.ones.saturating_add(1); }
        let total = entry.zeros as u32 + entry.ones as u32;
        if total >= COUNT_LIMIT {
            entry.zeros = ((entry.zeros as u32 + 1) >> 1).max(1) as u16;
            entry.ones = ((entry.ones as u32 + 1) >> 1).max(1) as u16;
        }
        if self.flags & FLAG_MATCH != 0 && match_valid {
            if zero == match_predicts_zero {
                self.match_weight = (self.match_weight + ((MIX_SCALE - self.match_weight).max(1) >> 7)).min(MIX_SCALE);
            } else {
                self.match_weight = (self.match_weight - (self.match_weight.max(1) >> 6)).max(0);
            }
        }
        if self.flags & FLAG_CALIBRATION != 0 {
            let bin = (p_before_cal >> 10).min(63) as usize;
            if zero { self.cal_zeros[bin] = self.cal_zeros[bin].saturating_add(1); } else { self.cal_ones[bin] = self.cal_ones[bin].saturating_add(1); }
            let total = self.cal_zeros[bin] as u32 + self.cal_ones[bin] as u32;
            if total >= COUNT_LIMIT {
                self.cal_zeros[bin] = ((self.cal_zeros[bin] as u32 + 1) >> 1).max(1) as u16;
                self.cal_ones[bin] = ((self.cal_ones[bin] as u32 + 1) >> 1).max(1) as u16;
            }
        }
    }

    fn update_sdf_field(&mut self, history: &[u8]) {
        if self.flags & FLAG_SDF_FIELD == 0 || history.is_empty() { return; }
        let b = *history.last().unwrap();
        if history.ends_with(b"\n$$$$\n") {
            self.sdf_field = 0;
            self.sdf_line_start = true;
            self.sdf_in_name = false;
            return;
        }
        if b == b'\n' {
            self.sdf_line_start = true;
            self.sdf_in_name = false;
            return;
        }
        if self.sdf_line_start {
            self.sdf_line_start = false;
            self.sdf_in_name = b == b'>';
            if self.sdf_in_name { self.sdf_field = 0; }
            return;
        }
        if self.sdf_in_name && b != b' ' && b != b'\r' {
            self.sdf_field = self.sdf_field.wrapping_mul(131).wrapping_add(b);
        }
    }

    fn update_record_phase(&mut self, history: &[u8]) {
        if self.flags & FLAG_SDF_RECORD == 0 { return; }
        self.record_phase = self.record_phase.wrapping_add(1);
        if history.ends_with(b"\n$$$$\n") { self.record_phase = 0; }
    }

    fn update_match_index(&mut self, history: &[u8]) {
        if self.flags & FLAG_MATCH == 0 || history.len() < 5 { return; }
        // The four-byte suffix ending immediately before the most recently decoded byte has a
        // known continuation. Index that earlier suffix, not the current trailing suffix whose
        // continuation is still unknown at this point.
        let n = history.len();
        let key = [history[n - 5], history[n - 4], history[n - 3], history[n - 2]];
        let x = u32::from_le_bytes(key).wrapping_mul(0x1e35_a7bd).rotate_left(11);
        let slot = x as usize & MATCH_MASK;
        let source = n - 1;
        if source < u32::MAX as usize {
            self.matches[slot] = source as u32 + 1;
        }
    }
}

fn clamp_p(p: u32) -> u32 { p.clamp(P_MIN, P_MAX) }

const RC_TOP: u32 = 0x01_00_00_00;

/// LZMA-style binary range encoder. `low` is deliberately 64-bit so carries
/// are emitted through `shift_low` rather than silently wrapping.
struct RangeEncoder {
    low: u64,
    range: u32,
    cache: u8,
    cache_size: usize,
    out: Vec<u8>,
}

impl RangeEncoder {
    fn new() -> Self {
        Self { low: 0, range: u32::MAX, cache: 0, cache_size: 1, out: Vec::new() }
    }

    fn shift_low(&mut self) {
        if self.low < 0xff00_0000 || (self.low >> 32) != 0 {
            let carry = (self.low >> 32) as u8;
            let mut byte = self.cache;
            loop {
                self.out.push(byte.wrapping_add(carry));
                byte = 0xff;
                self.cache_size -= 1;
                if self.cache_size == 0 { break; }
            }
            self.cache = (self.low >> 24) as u8;
        }
        self.cache_size += 1;
        self.low = (self.low & 0x00ff_ffff) << 8;
    }

    fn bit(&mut self, p_zero: u32, zero: bool) {
        let unit = self.range >> 16;
        let bound = unit * clamp_p(p_zero);
        if zero {
            self.range = bound;
        } else {
            self.low = self.low.wrapping_add(bound as u64);
            self.range -= bound;
        }
        while self.range < RC_TOP {
            self.range <<= 8;
            self.shift_low();
        }
    }

    fn finish(mut self) -> Vec<u8> {
        for _ in 0..5 { self.shift_low(); }
        self.out
    }
}

struct RangeDecoder<'a> {
    range: u32,
    code: u32,
    src: &'a [u8],
    pos: usize,
}

impl<'a> RangeDecoder<'a> {
    fn new(src: &'a [u8]) -> Result<Self, &'static str> {
        if src.len() < 5 { return Err("gcr1: truncated"); }
        let mut code = 0u32;
        for &byte in &src[..5] { code = (code << 8) | byte as u32; }
        Ok(Self { range: u32::MAX, code, src, pos: 5 })
    }

    fn bit(&mut self, p_zero: u32) -> Result<bool, &'static str> {
        let unit = self.range >> 16;
        let bound = unit * clamp_p(p_zero);
        let zero = self.code < bound;
        if zero {
            self.range = bound;
        } else {
            self.code -= bound;
            self.range -= bound;
        }
        while self.range < RC_TOP {
            if self.pos >= self.src.len() { return Err("gcr1: truncated"); }
            self.code = (self.code << 8) | self.src[self.pos] as u32;
            self.pos += 1;
            self.range <<= 8;
        }
        Ok(zero)
    }
}

pub fn encode(data: &[u8], flags: u8) -> Result<Vec<u8>, &'static str> {
    let mut model = Model::new(flags)?;
    let mut coder = RangeEncoder::new();
    let mut history = Vec::with_capacity(data.len());
    for (byte_pos, &byte) in data.iter().enumerate() {
        let mut prefix = 1u16;
        for bit in 0..8 {
            let (p, match_valid, match_zero) = model.probability_zero(&history, byte_pos, prefix, bit);
            let zero = ((byte >> (7 - bit)) & 1) == 0;
            coder.bit(p, zero);
            model.update_bit(&history, byte_pos, prefix, zero, match_valid, match_zero, p);
            prefix = (prefix << 1) | (!zero as u16);
        }
        history.push(byte);
        model.update_sdf_field(&history);
        model.update_record_phase(&history);
        model.update_match_index(&history);
    }
    if data.len() > u32::MAX as usize { return Err("gcr1: input too large"); }
    let coded = coder.finish();
    let mut out = Vec::with_capacity(10 + coded.len());
    out.extend_from_slice(MAGIC);
    out.push(VERSION);
    out.push(flags);
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&coded);
    Ok(out)
}

pub fn decode(buf: &[u8], expected_flags: u8) -> Result<Vec<u8>, &'static str> {
    if buf.len() < 10 || &buf[..4] != MAGIC { return Err("gcr1: magic"); }
    if buf[4] != VERSION { return Err("gcr1: version"); }
    let flags = buf[5];
    if flags != expected_flags { return Err("gcr1: profile"); }
    let orig_len = u32::from_le_bytes(buf[6..10].try_into().unwrap()) as usize;
    let mut model = Model::new(flags)?;
    let mut coder = RangeDecoder::new(&buf[10..])?;
    let mut history = Vec::with_capacity(orig_len);
    for byte_pos in 0..orig_len {
        let mut prefix = 1u16;
        let mut byte = 0u8;
        for bit in 0..8 {
            let (p, match_valid, match_zero) = model.probability_zero(&history, byte_pos, prefix, bit);
            let zero = coder.bit(p)?;
            model.update_bit(&history, byte_pos, prefix, zero, match_valid, match_zero, p);
            byte = (byte << 1) | (!zero as u8);
            prefix = (prefix << 1) | (!zero as u16);
        }
        history.push(byte);
        model.update_sdf_field(&history);
        model.update_record_phase(&history);
        model.update_match_index(&history);
    }
    Ok(history)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rt(data: &[u8], flags: u8) {
        let c = encode(data, flags).unwrap();
        let d = decode(&c, flags).unwrap();
        assert_eq!(d, data);
    }

    #[test]
    fn roundtrips_all_profiles() {
        let samples = [
            Vec::new(), vec![0], (0..=255).collect(),
            b"the quick brown fox jumps over the lazy dog\n".repeat(80),
            vec![0xaa; 4097],
        ];
        for flags in [0, FLAG_MATCH, VALID_FLAGS] {
            for sample in &samples { rt(sample, flags); }
        }
    }

    #[test]
    fn rejects_bad_header() {
        assert!(decode(b"bad", 0).is_err());
        let mut c = encode(b"hello", 0).unwrap();
        c[4] = 9;
        assert!(decode(&c, 0).is_err());
        let mut c = encode(b"hello", 0).unwrap();
        c[5] = 0x80;
        assert!(decode(&c, 0).is_err());
    }

    #[test]
    fn rejects_truncation_and_profile_mismatch() {
        let src = b"gcr1 corruption fixture".repeat(64);
        let c = encode(&src, VALID_FLAGS).unwrap();
        for end in 0..c.len() {
            assert!(decode(&c[..end], VALID_FLAGS).is_err());
        }
        assert!(decode(&c, FLAG_MATCH).is_err());
    }

    #[test]
    fn match_profile_roundtrips_repetition() {
        let data = b"abcdabcdabcdabcdabcdabcdabcdabcd".repeat(128);
        let matched = encode(&data, FLAG_MATCH).unwrap();
        assert_eq!(decode(&matched, FLAG_MATCH).unwrap(), data);
    }
}
