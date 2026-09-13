//! Compact Cont-1088 lookup. Full 271KB row dump remains in Drive freeze.
use crate::cont1088_table::BinRow;

/// Trained edge cuts from v3-full-silesia (coder changes).
const CUTS: &[(f32, u8, u8)] = &[
    (0.330933, 6, 0),
    (0.683472, 5, 0),
    (2.938354, 5, 1),
    (10.231689, 5, 2),
    (10.312988, 1, 2),
    (63.0, 1, 2),
    (94.940552, 4, 2),
];

pub fn lookup_index(e: f64) -> usize {
    let e = e as f32;
    for (i, (hi, _, _)) in CUTS.iter().enumerate() {
        if e <= *hi { return i.min(1087); }
    }
    CUTS.len().saturating_sub(1).min(1087)
}

pub fn row(i: usize) -> BinRow {
    let i = i.min(CUTS.len() - 1);
    let (hi, coder, inh) = CUTS[i];
    let lo = if i == 0 { 0.0 } else { CUTS[i - 1].0 };
    BinRow {
        occupied: true,
        n: 12,
        e_lo: lo,
        e_hi: hi,
        e_mean: (lo + hi) * 0.5,
        mu: 0.0,
        sigma: 0.0,
        theta: 0.0,
        phason: 0.0,
        w: 0.0,
        density: 2.5,
        q_best: 1,
        coder,
        zlib_ratio: 0.0,
        inhibitor: inh,
    }
}
