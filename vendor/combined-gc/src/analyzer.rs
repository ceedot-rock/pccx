//! WAVE 4 analyzer: Shannon entropy + type heuristics.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DataType {
    Text,
    Json,
    Float,
    Code,
    Binary,
    Random,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct Analysis {
    pub guessed_type: DataType,
    pub entropy: f64,
    pub zero_ratio: f64,
    pub ascii_ratio: f64,
}

impl Analysis {
    pub fn is_compressible(&self) -> bool {
        self.entropy < 7.5
            || self.zero_ratio > 0.3
            || matches!(
                self.guessed_type,
                DataType::Text | DataType::Json | DataType::Float | DataType::Code
            )
    }
}

pub struct Analyzer;

impl Analyzer {
    pub fn analyze(data: &[u8]) -> Analysis {
        if data.is_empty() {
            return Analysis {
                guessed_type: DataType::Unknown,
                entropy: 0.0,
                zero_ratio: 0.0,
                ascii_ratio: 0.0,
            };
        }
        let mut freq = [0u64; 256];
        let mut zeros = 0u64;
        let mut ascii = 0u64;
        for &b in data {
            freq[b as usize] += 1;
            if b == 0 {
                zeros += 1;
            }
            if (32..127).contains(&b) {
                ascii += 1;
            }
        }
        let n = data.len() as f64;
        let mut entropy = 0.0;
        for &c in &freq {
            if c > 0 {
                let p = c as f64 / n;
                entropy -= p * p.log2();
            }
        }
        let zero_ratio = zeros as f64 / n;
        let ascii_ratio = ascii as f64 / n;

        let guessed_type = if zero_ratio > 0.5 {
            DataType::Binary
        } else if ascii_ratio > 0.85 {
            if data.windows(2).any(|w| w == b"{\"" || w == b": ") {
                DataType::Json
            } else if data
                .windows(3)
                .any(|w| w == b"fn " || w == b"let" || w == b"-> ")
            {
                DataType::Code
            } else {
                DataType::Text
            }
        } else if data.len() % 4 == 0 && entropy > 4.0 && entropy < 7.0 {
            DataType::Float
        } else if entropy > 7.8 {
            DataType::Random
        } else {
            DataType::Binary
        };

        Analysis {
            guessed_type,
            entropy,
            zero_ratio,
            ascii_ratio,
        }
    }
}

pub fn is_text_like(pack: &[u8]) -> bool {
    if pack.is_empty() {
        return false;
    }
    let sample = &pack[..pack.len().min(4096)];
    let printable = sample
        .iter()
        .filter(|&&b| (32..127).contains(&b) || matches!(b, 9 | 10 | 13))
        .count();
    printable * 100 / sample.len() > 85
}

pub fn is_float32_le(pack: &[u8]) -> bool {
    if pack.len() < 16 {
        return false;
    }
    let sample = &pack[..pack.len().min(4096)];
    let mut tot = 0u32;
    let mut ok = 0u32;
    let mut i = 0;
    while i + 4 <= sample.len() {
        let bits = u32::from_le_bytes(sample[i..i + 4].try_into().unwrap());
        tot += 1;
        let exp = (bits >> 23) & 0xff;
        if exp != 0xff {
            let f = f32::from_bits(bits);
            if f.is_finite() && f.abs() < 1.0e20 {
                ok += 1;
            }
        }
        i += 4;
    }
    tot > 0 && ok * 100 / tot >= 95 && !is_text_like(pack)
}

pub fn is_int16_delta(pack: &[u8]) -> bool {
    if pack.len() < 16 {
        return false;
    }
    let sample = &pack[..pack.len().min(4096)];
    let mut n = 0u32;
    let mut mean = 0.0f64;
    let mut dlt = 0.0f64;
    let mut prev = 0i32;
    let mut i = 0;
    while i + 2 <= sample.len() {
        let v = i16::from_le_bytes(sample[i..i + 2].try_into().unwrap()) as i32;
        mean += v.abs() as f64;
        if n > 0 {
            dlt += (v - prev).abs() as f64;
        }
        prev = v;
        n += 1;
        i += 2;
    }
    if n < 8 {
        return false;
    }
    mean /= n as f64;
    dlt /= (n - 1) as f64;
    !is_text_like(pack) && mean > 1.0 && dlt / mean < 0.20
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_text() {
        let a = Analyzer::analyze(b"the quick brown fox jumps over the lazy dog again");
        assert_eq!(a.guessed_type, DataType::Text);
        assert!(a.is_compressible());
    }

    #[test]
    fn detector_text_geo_xray() {
        let text = b"the quick brown fox jumps over the lazy dog\n".repeat(80);
        assert!(is_text_like(&text));
        assert!(!is_float32_le(&text));
        assert!(!is_int16_delta(&text));
        let mut geo = Vec::new();
        for i in 0..1024u32 {
            geo.extend_from_slice(&((i as f32) * 0.001 + 1.0).to_le_bytes());
        }
        assert!(!is_text_like(&geo));
        assert!(is_float32_le(&geo));
        let mut xr = Vec::new();
        let mut v = 1000i16;
        for _ in 0..2048 {
            xr.extend_from_slice(&v.to_le_bytes());
            v = v.wrapping_add(1);
        }
        assert!(!is_text_like(&xr));
        assert!(is_int16_delta(&xr));
    }

    #[test]
    fn random_gives_up() {
        let buf: Vec<u8> = (0..=255).collect();
        let a = Analyzer::analyze(&buf);
        assert!(a.entropy > 7.9);
        assert_eq!(a.guessed_type, DataType::Random);
        assert!(!a.is_compressible());
    }
}
