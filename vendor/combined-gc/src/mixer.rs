//! Online mixer: P_mix = Σ w_k P_k, lr = 0.02, 8-bit header.

pub struct Mixer {
    pub weights: Vec<f32>,
    pub lr: f32,
}

impl Mixer {
    pub fn new(n: usize) -> Self {
        let n = n.max(1);
        Self {
            weights: vec![1.0 / n as f32; n],
            lr: 0.02,
        }
    }

    pub fn mix(&self, probs: &[f64]) -> f64 {
        self.weights
            .iter()
            .zip(probs.iter())
            .map(|(w, p)| *w as f64 * *p)
            .sum()
    }

    pub fn update(&mut self, probs: &[f64], p_mix: f64) {
        for (w, p) in self.weights.iter_mut().zip(probs.iter()) {
            *w *= 1.0 + self.lr * (*p as f32) / (p_mix as f32 + 1e-6);
        }
        let s: f32 = self.weights.iter().sum();
        if s > 0.0 {
            for w in self.weights.iter_mut() {
                *w /= s;
            }
        }
    }
}

pub fn write_header(weights: &[f32]) -> Vec<u8> {
    weights.iter().map(|&w| (w.clamp(0.0, 1.0) * 255.0) as u8).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixes_uniform() {
        let m = Mixer::new(3);
        let p = m.mix(&[0.2, 0.3, 0.5]);
        assert!((p - (0.2 + 0.3 + 0.5) / 3.0).abs() < 1e-6);
    }
}
