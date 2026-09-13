//! Hangry Taylor O4/O5 + QQ + Cont-1088 + InhibitorBus
//! from Drive sealed docs (models_live Full Source + ACTUAL_CODE 2026-08-25).
//!
//! Hangry residual is lossless (wrapping byte). QQ zeroing is lossy — used
//! for scoring / bin pick, not the default stored path.
//! Cont-1088 table: `cont1088_table` trained 2026-08-26 on 4736 synthetic
//! windows (zlib-9). Not dickens. Quantile bins on Hangry-O5 E.

use std::f64::consts::PI;

pub const PHI: f64 = 1.618033988749895;

/// Hangry as a mixer probability, not a stored byte.
/// O4 reciprocal Taylor of last byte / 255.
pub fn hangry_predictor(ctx: &[u8]) -> f64 {
    let x = *ctx.last().unwrap_or(&0) as f64 / 255.0;
    1.0 / (1.0 + x + x * x / 2.0 + x * x * x / 6.0 + x * x * x * x / 24.0)
}
pub const QQ_LIST: [i32; 6] = [1, 2, 4, 8, 16, 32];
pub const CONT_BINS: usize = 1088;

pub fn residual_energy(residual: &[i32]) -> f64 {
    if residual.is_empty() {
        return 0.0;
    }
    residual.iter().map(|&x| x.abs() as f64).sum::<f64>() / residual.len() as f64
}

pub fn phason(theta_mean: f64) -> f64 {
    0.5 * (1.0 - (2.0 * PI * (theta_mean.rem_euclid(1.0)) / PHI).cos())
}

pub fn wasserstein_proxy(mu: f64, sigma: f64) -> f64 {
    (mu * mu + sigma * sigma).sqrt()
}

pub fn gap_label(k: i32) -> f64 {
    PHI.powi(-k)
}

pub fn density_of(e: f64) -> f64 {
    (e.abs() * 0.6 + 0.3).min(2.5)
}

pub fn v_size(len: usize) -> f64 {
    ((len as f64) + 1.0).ln() / (1_000_000f64 + 1.0).ln() * 2.0
}

/// Drive QQ: first q in [1,2,4,8,16,32] with |d| < q → 0, else d.
/// Lossy if applied to stored residuals.
pub fn qq_soft_delta(d: i32, q_list: &[i32]) -> i32 {
    for &q in q_list {
        if d.abs() < q {
            return 0;
        }
    }
    d
}

pub fn qq_soft_delta_q(d: i32, q: i32) -> i32 {
    if d.abs() < q {
        0
    } else {
        d
    }
}

pub fn hangry_pred(prev: u8, order: u8) -> u8 {
    let x = prev as f64 / 255.0;
    let rec = match order {
        4 => 1.0 / (1.0 + x + x * x * 0.5 + x * x * x / 6.0),
        _ => 1.0 / (1.0 + x + x * x * 0.5 + x * x * x / 6.0 + x.powi(4) / 24.0),
    };
    (rec * 255.0) as u8
}

/// Lossless Hangry residual: wrapping_sub(pred). Same length as input.
pub fn hangry_residual(data: &[u8], order: u8) -> (Vec<u8>, Vec<i32>, f64) {
    let mut out = Vec::with_capacity(data.len());
    let mut signed = Vec::with_capacity(data.len());
    let mut prev = 0u8;
    for &b in data {
        let pred = hangry_pred(prev, order);
        let r = b as i16 - pred as i16;
        signed.push(r as i32);
        out.push(b.wrapping_sub(pred));
        prev = b;
    }
    let e = residual_energy(&signed);
    (out, signed, e)
}

pub fn hangry_undelta(residual: &[u8], order: u8) -> Vec<u8> {
    let mut out = Vec::with_capacity(residual.len());
    let mut prev = 0u8;
    for &r in residual {
        let pred = hangry_pred(prev, order);
        let b = pred.wrapping_add(r);
        out.push(b);
        prev = b;
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Inhibitor {
    Allow,
    Throttle,
    Block,
    SuggestAlt,
}

pub fn inhibitor_bus(e: f64, density: f64) -> Inhibitor {
    if e < 1.0 && density < 1.5 {
        Inhibitor::Allow
    } else if e > 2.0 || density >= 1.85 {
        Inhibitor::Block
    } else if e > 1.2 {
        Inhibitor::Throttle
    } else {
        Inhibitor::SuggestAlt
    }
}

pub fn mirrored_cost(delta_e: f64, delta_c: f64) -> f64 {
    (delta_e + delta_c).abs()
}

#[derive(Debug, Clone)]
pub struct ContState {
    pub e: f64,
    pub mu: f64,
    pub sigma: f64,
    pub theta: f64,
    pub phason: f64,
    pub wasserstein: f64,
    pub gap4: f64,
    pub gap5: f64,
    pub density: f64,
    pub v: f64,
    pub bin: usize,
    pub inhibitor: Inhibitor,
    pub q_best: i32,
    pub coder: u8,
    pub trained: bool,
}

pub fn cont_state(signed: &[i32], orig_len: usize) -> ContState {
    let e = residual_energy(signed);
    let n = signed.len().max(1) as f64;
    let mu = signed.iter().map(|&x| x as f64).sum::<f64>() / n;
    let var = signed
        .iter()
        .map(|&x| {
            let d = x as f64 - mu;
            d * d
        })
        .sum::<f64>()
        / n;
    let sigma = var.sqrt();
    let theta = (mu / 255.0).rem_euclid(1.0);
    let ph = phason(theta);
    let w = wasserstein_proxy(mu, sigma);
    let density = density_of(e);
    let idx = crate::cont1088_table::lookup_index(e);
    let row = &crate::cont1088_table::ROWS[idx];
    ContState {
        e,
        mu,
        sigma,
        theta,
        phason: ph,
        wasserstein: w,
        gap4: gap_label(4),
        gap5: gap_label(5),
        density,
        v: v_size(orig_len),
        bin: idx,
        inhibitor: match row.inhibitor {
            0 => Inhibitor::Allow,
            1 => Inhibitor::Throttle,
            2 => Inhibitor::Block,
            _ => Inhibitor::SuggestAlt,
        },
        q_best: row.q_best as i32,
        coder: row.coder,
        trained: row.occupied,
    }
}

pub fn analyze_block(data: &[u8]) -> ContState {
    // Same feature as tools/train_cont1088.py: wrapping-delta residual, not Hangry.
    let mut signed = Vec::with_capacity(data.len());
    let mut prev = 0u8;
    for &b in data {
        let d = b.wrapping_sub(prev);
        signed.push(if d < 128 { d as i32 } else { d as i32 - 256 });
        prev = b;
    }
    cont_state(&signed, data.len())
}

pub fn trained_program(coder: u8) -> crate::vm::Program {
    use crate::vm::{Op, Program};
    match coder {
        6 => Program {
            ops: vec![Op::Tru8Zero],
        },
        5 => Program {
            ops: vec![Op::Rle, Op::Lz],
        },
        4 => Program {
            ops: vec![Op::XorFloat, Op::Lz],
        },
        3 => Program {
            ops: vec![Op::Hangry { order: 5 }, Op::Lz],
        },
        2 => Program {
            ops: vec![Op::Delta { order: 1 }, Op::Lz],
        },
        1 => Program { ops: vec![Op::Lz] },
        7 => Program {
            ops: vec![Op::Bwt, Op::Mtf, Op::Ans],
        },
        8 => Program {
            ops: vec![Op::Ans],
        },
        _ => Program::store(),
    }
}

#[allow(dead_code)]
fn pick_q(signed: &[i32]) -> i32 {
    let e0 = residual_energy(signed);
    let mut best_q = 1;
    let mut best_m = f64::MAX;
    for &q in &QQ_LIST {
        let soft: Vec<i32> = signed.iter().map(|&d| qq_soft_delta_q(d, q)).collect();
        let e1 = residual_energy(&soft);
        let zeros = soft.iter().filter(|&&d| d == 0).count() as f64 / signed.len().max(1) as f64;
        // Cheap stand-in for ΔC: more zeros → lower code cost.
        let delta_e = e1 - e0;
        let delta_c = -zeros;
        let m = mirrored_cost(delta_e, delta_c);
        if m < best_m {
            best_m = m;
            best_q = q;
        }
    }
    best_q
}

pub struct HangryTaylor {
    pub order: u8,
}

impl HangryTaylor {
    pub fn o4_5_reciprocal(x: f64) -> f64 {
        1.0 / (1.0 + x + x * x / 2.0 + x * x * x / 6.0 + x.powi(4) / 24.0)
    }

    pub fn compress(&self, data: &[u8]) -> Vec<u8> {
        hangry_residual(data, self.order.max(4)).0
    }
}

pub struct Cont1088 {
    pub bins: usize,
}

impl Cont1088 {
    pub fn new() -> Self {
        Self { bins: CONT_BINS }
    }
    pub fn cluster(&self, residuals: &[i32]) -> Vec<(f64, f64)> {
        let e = residual_energy(residuals);
        vec![(e, e * 0.5); self.bins.min(residuals.len().max(1))]
    }
}

impl Default for Cont1088 {
    fn default() -> Self {
        Self::new()
    }
}

pub struct LoominNCA {
    pub grid: [[f64; 8]; 8],
}

impl LoominNCA {
    pub fn new() -> Self {
        Self {
            grid: [[0.0; 8]; 8],
        }
    }
    pub fn step(&mut self) {
        for _ in 0..30 {
            let mut new_grid = self.grid;
            for i in 0..8 {
                for j in 0..8 {
                    let sum = self.grid[(i + 7) % 8][j]
                        + self.grid[(i + 1) % 8][j]
                        + self.grid[i][(j + 7) % 8]
                        + self.grid[i][(j + 1) % 8];
                    new_grid[i][j] = (self.grid[i][j] + sum / 4.0 * PHI) % 1.0;
                }
            }
            self.grid = new_grid;
        }
    }
}

impl Default for LoominNCA {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phi_constant() {
        assert!((PHI - 1.618033988749895).abs() < 1e-12);
    }

    #[test]
    fn hangry_same_len() {
        let h = HangryTaylor { order: 5 };
        let out = h.compress(b"hello");
        assert_eq!(out.len(), 5);
    }

    #[test]
    fn hangry_predictor_is_prob() {
        let p0 = hangry_predictor(&[0]);
        let p1 = hangry_predictor(&[255]);
        assert!((p0 - 1.0).abs() < 1e-12);
        assert!(p1 > 0.3 && p1 < 0.5);
        assert!(p0 > p1);
    }

    #[test]
    fn hangry_roundtrip() {
        let src = b"the quick brown fox jumps over the lazy dog 0123456789";
        let (r, _, e) = hangry_residual(src, 5);
        let back = hangry_undelta(&r, 5);
        assert_eq!(back, src);
        assert!(e >= 0.0);
    }

    #[test]
    fn qq_zeros_small() {
        assert_eq!(qq_soft_delta(0, &QQ_LIST), 0);
        assert_eq!(qq_soft_delta(1, &QQ_LIST), 0);
        assert_eq!(qq_soft_delta(40, &QQ_LIST), 40);
    }

    #[test]
    fn cont_bins_in_range() {
        let src = vec![7u8; 64];
        let st = analyze_block(&src);
        assert!(st.bin < CONT_BINS);
        assert_eq!(st.gap4, gap_label(4));
        assert!(st.trained);
        assert_eq!(crate::cont1088_table::OCCUPIED, 1088);
    }
}
