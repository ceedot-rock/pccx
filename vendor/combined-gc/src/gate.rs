//! Cheap router. Not a codec.
//!
//! Branch order is entropy-high first so Random is reachable.
//! Original dump checked Floats (entropy > 6.5) before Random (> 7.9),
//! which made the Random arm dead. That one-line order is the only
//! behavioral fix vs the attached gate.rs.txt.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GatePath {
    Text,
    Binary,
    StructuredInts,
    Floats,
    Delta,
    Random,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GateStats {
    pub entropy: f64,
    pub printable_ratio: f64,
    pub zero_runs: usize,
    pub delta_var: f64,
}

pub fn cheap_stats(chunk: &[u8]) -> GateStats {
    let mut freq = [0u32; 256];
    for &b in chunk {
        freq[b as usize] += 1;
    }
    let len = chunk.len() as f64;
    let mut h = 0.0;
    for &f in &freq {
        if f > 0 {
            let p = f as f64 / len;
            h -= p * p.log2();
        }
    }
    let printable = if len > 0.0 {
        chunk.iter().filter(|&&b| (32..127).contains(&b)).count() as f64 / len
    } else {
        0.0
    };
    let zero_runs = if chunk.len() >= 4 {
        chunk.windows(4).filter(|w| *w == [0, 0, 0, 0]).count()
    } else {
        0
    };
    let delta_var = if chunk.len() > 1 {
        let diffs: Vec<i16> = chunk
            .windows(2)
            .map(|w| w[1] as i16 - w[0] as i16)
            .collect();
        let mean = diffs.iter().map(|&x| x as f64).sum::<f64>() / diffs.len() as f64;
        diffs.iter().map(|&x| (x as f64 - mean).powi(2)).sum::<f64>() / diffs.len() as f64
    } else {
        0.0
    };
    GateStats {
        entropy: h,
        printable_ratio: printable,
        zero_runs,
        delta_var,
    }
}

/// Seed program for the path. Frame still bake-off-encodes.
pub fn seed_ops(path: GatePath) -> &'static [crate::vm::Op] {
    use crate::vm::Op;
    match path {
        GatePath::Text => &[Op::Bwt, Op::Mtf, Op::Rle, Op::Ans],
        GatePath::StructuredInts => &[Op::Tru8Zero],
        GatePath::Floats => &[Op::XorFloat, Op::Bwt, Op::Lz],
        GatePath::Delta => &[Op::Delta { order: 1 }, Op::Lz],
        GatePath::Random => &[Op::Store],
        GatePath::Binary => &[Op::Lz],
    }
}

pub fn route(stats: &GateStats) -> (GatePath, [f32; 5]) {
    if stats.printable_ratio > 0.85 && stats.entropy < 5.0 {
        (GatePath::Text, [0.15, 0.15, 0.1, 0.7, 0.1])
    } else if stats.delta_var < 10.0 && stats.zero_runs > 5 {
        (GatePath::StructuredInts, [0.2, 0.5, 0.05, 0.05, 0.7])
    } else if stats.entropy > 7.9 {
        (GatePath::Random, [0.25, 0.25, 0.25, 0.25, 0.0])
    } else if stats.entropy > 6.5 {
        (GatePath::Floats, [0.2, 0.6, 0.6, 0.1, 0.25])
    } else {
        (GatePath::Binary, [0.33, 0.33, 0.33, 0.33, 0.33])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zeros_look_structured() {
        let stats = cheap_stats(&[0u8; 64]);
        assert!(stats.zero_runs > 5);
        assert!(stats.entropy < 0.01);
        let (path, _) = route(&stats);
        assert_eq!(path, GatePath::StructuredInts);
    }

    #[test]
    fn high_entropy_is_not_text() {
        let buf: Vec<u8> = (0..=255).collect();
        let stats = cheap_stats(&buf);
        assert!(stats.entropy > 7.9, "entropy={}", stats.entropy);
        let (path, _) = route(&stats);
        assert_eq!(path, GatePath::Random);
    }
}
