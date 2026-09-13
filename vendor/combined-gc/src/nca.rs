//! Program search. Fitness = |program| + |residual|.
//! Deterministic (xxh3-seeded xorshift). No `rand` crate.

use crate::analyzer::{Analysis, DataType};
use crate::pipeline;
use crate::vm::{Op, Program};
use xxhash_rust::xxh3::xxh3_64;

pub struct NcaConfig {
    pub generations: usize,
}

impl Default for NcaConfig {
    fn default() -> Self {
        Self { generations: 4 }
    }
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn bool(&mut self, p: f64) -> bool {
        (self.next() as f64) / (u64::MAX as f64) < p
    }
    fn idx(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() as usize) % n
        }
    }
}

pub fn seed_program(analysis: &Analysis) -> Program {
    let ops = match analysis.guessed_type {
        DataType::Float => vec![Op::XorFloat, Op::Lz],
        DataType::Json | DataType::Text | DataType::Code => {
            vec![Op::Hangry { order: 5 }, Op::Lz]
        }
        _ if analysis.zero_ratio > 0.7 => vec![Op::Rle, Op::Lz],
        _ if analysis.entropy < 5.0 => vec![Op::Delta { order: 1 }, Op::Lz],
        DataType::Random => vec![Op::Store],
        _ => vec![Op::Lz],
    };
    Program { ops }
}

/// Skip SA(S||S) when the pack cannot beat ZLIB.
/// Dickens ~4.5, x-ray ~6.3, random ~8.0.
pub const MIN_BWT: usize = 4096;
pub const ENTROPY_CUTOFF: f64 = 7.2;

#[inline]
pub fn should_try_bwt(data: &[u8], entropy: f64) -> bool {
    data.len() >= MIN_BWT && entropy < ENTROPY_CUTOFF
}

fn has_bwt(p: &Program) -> bool {
    p.ops.iter().any(|o| matches!(o, Op::Bwt))
}

fn bank(analysis: &Analysis, data: &[u8]) -> Vec<Program> {
    let nbytes = data.len();
    let try_bwt = should_try_bwt(data, analysis.entropy);
    let mut v = vec![
        Program::store(),
        Program { ops: vec![Op::Lz] },
        Program {
            ops: vec![Op::Delta { order: 1 }, Op::Lz],
        },
        Program {
            ops: vec![Op::Delta { order: 2 }, Op::Lz],
        },
        Program {
            ops: vec![Op::Rle, Op::Lz],
        },
        Program {
            ops: vec![Op::XorFloat, Op::Lz],
        },
        Program {
            ops: vec![Op::Delta { order: 1 }, Op::Rle, Op::Lz],
        },
        seed_program(analysis),
        Program {
            ops: vec![Op::Hangry { order: 5 }, Op::Lz],
        },
    ];
    if nbytes >= 512 {
        v.push(Program {
            ops: vec![Op::Ans],
        });
        v.push(Program {
            ops: vec![Op::Rle, Op::Ans],
        });
        if try_bwt {
            v.push(Program {
                ops: vec![Op::Bwt, Op::Lz],
            });
            v.push(Program {
                ops: vec![Op::Bwt, Op::Mtf, Op::Lz],
            });
            v.push(Program {
                ops: vec![Op::Bwt, Op::Mtf, Op::Rle, Op::Lz],
            });
            v.push(Program {
                ops: vec![Op::Bwt, Op::Hangry { order: 5 }, Op::Lz],
            });
            v.push(Program {
                ops: vec![Op::Delta { order: 2 }, Op::Bwt, Op::Lz],
            });
            v.push(Program {
                ops: vec![Op::XorFloat, Op::Bwt, Op::Lz],
            });
            v.push(Program {
                ops: vec![Op::Bwt, Op::Mtf, Op::Ans],
            });
            v.push(Program {
                ops: vec![Op::Bwt, Op::Wfc, Op::Ans],
            });
            v.push(Program {
                ops: vec![Op::Lzp, Op::Bwt, Op::Mtf, Op::Ans],
            });
            v.push(Program {
                ops: vec![Op::Bwt, Op::Mtf, Op::Rle, Op::Ans],
            });
            v.push(Program {
                ops: vec![Op::Delta { order: 2 }, Op::Bwt, Op::Ans],
            });
            v.push(Program {
                ops: vec![Op::XorFloat, Op::Bwt, Op::Mtf, Op::Ans],
            });
            v.push(Program {
                ops: vec![Op::Delta { order: 2 }, Op::Bwt, Op::Mtf, Op::Ans],
            });
        }
    }
    if analysis.zero_ratio > 0.98 {
        v.push(Program {
            ops: vec![Op::Tru8Zero],
        });
    }
    if try_bwt && !crate::analyzer::is_text_like(data) {
        v.push(Program {
            ops: vec![Op::XorF4, Op::Bwt, Op::Lz],
        });
        v.push(Program {
            ops: vec![Op::XorF4, Op::Bwt, Op::Mtf, Op::Ans],
        });
        v.push(Program {
            ops: vec![Op::DeltaVar, Op::Bwt, Op::Lz],
        });
        v.push(Program {
            ops: vec![Op::DeltaVar, Op::Ans],
        });
        v.push(Program {
            ops: vec![Op::DeltaVar, Op::Bwt, Op::Mtf, Op::Ans],
        });
    }
    if crate::analyzer::is_text_like(data) {
        v.retain(|p| {
            matches!(
                p.ops.as_slice(),
                [Op::Store]
                    | [Op::Lz]
                    | [Op::Ans]
                    | [Op::Bwt, Op::Mtf, Op::Ans]
                    | [Op::Bwt, Op::Mtf, Op::Rle, Op::Ans]
            )
        });
        if try_bwt && !v.iter().any(has_bwt) {
            v.push(Program {
                ops: vec![Op::Bwt, Op::Mtf, Op::Ans],
            });
        }
    }
    v
}

fn mutate(p: &Program, rng: &mut Rng) -> Program {
    let candidates = [
        Op::Delta { order: 1 },
        Op::Delta { order: 2 },
        Op::XorFloat,
        Op::Hangry { order: 5 },
        Op::Rle,
        Op::Lz,
        Op::Ans,
        Op::Bwt,
        Op::Store,
    ];
    let mut ops = p.ops.clone();
    if ops.is_empty() {
        ops.push(Op::Store);
    }
    if rng.bool(0.5) {
        let i = rng.idx(ops.len());
        ops[i] = candidates[rng.idx(candidates.len())].clone();
    } else if rng.bool(0.4) && ops.len() < 4 {
        ops.insert(rng.idx(ops.len()), candidates[rng.idx(candidates.len())].clone());
    } else if ops.len() > 1 {
        ops.remove(rng.idx(ops.len()));
    }
    if !ops.iter().any(|o| matches!(o, Op::Lz | Op::Zlib | Op::Ans | Op::Store | Op::Tru8Zero)) {
        ops.push(Op::Lz);
    }
    Program { ops }
}

pub fn evolve(data: &[u8], analysis: &Analysis, cfg: &NcaConfig) -> Program {
    if data.is_empty() || !analysis.is_compressible() {
        return Program::store();
    }
    let sample = if data.len() > 4096 { &data[..4096] } else { data };
    let mut rng = Rng(xxh3_64(sample) | 1);
    let trained = crate::models_live::trained_program(crate::models_live::analyze_block(data).coder);
    let mut pop: Vec<(Program, usize)> = bank(analysis, sample)
        .into_iter()
        .map(|p| {
            let f = pipeline::fitness(&p, sample);
            (p, f)
        })
        .collect();
    pop.push((
        trained.clone(),
        pipeline::fitness(&trained, sample),
    ));
    for _ in 0..cfg.generations {
        pop.sort_by_key(|x| x.1);
        let elite = pop[0].clone();
        let mut next = vec![elite.clone()];
        while next.len() < pop.len() {
            let parent = &pop[rng.idx((pop.len() / 2).max(1))].0;
            let child = mutate(parent, &mut rng);
            let f = pipeline::fitness(&child, sample);
            next.push((child, f));
        }
        pop = next;
    }
    pop.sort_by_key(|x| x.1);
    pop[0].0.clone()
}

/// Evaluate the bank on this exact buffer. No mutation. Used per 16K block.
pub fn pick_best(data: &[u8]) -> Program {
    if data.is_empty() {
        return Program::store();
    }
    if crate::zrw::is_zero_run(data) {
        return Program {
            ops: vec![Op::Tru8Zero],
        };
    }
    let analysis = crate::analyzer::Analyzer::analyze(data);
    let trained = crate::models_live::trained_program(crate::models_live::analyze_block(data).coder);
    let mut best = trained.clone();
    let mut best_f = pipeline::fitness(&best, data);
    for p in bank(&analysis, data).into_iter().chain(std::iter::once(trained)) {
        let f = pipeline::fitness(&p, data);
        if f < best_f {
            best_f = f;
            best = p;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::Analyzer;

    #[test]
    fn should_try_bwt_text_yes_random_no() {
        let text = b"The Pickwick Papers chapter one Mr Pickwick.\n".repeat(200);
        let a = Analyzer::analyze(&text);
        assert!(should_try_bwt(&text, a.entropy), "H={}", a.entropy);
        let flat: Vec<u8> = (0..8192).map(|i| i as u8).collect();
        let b = Analyzer::analyze(&flat);
        assert!(
            !should_try_bwt(&flat, b.entropy),
            "flat H={} should skip BWT",
            b.entropy
        );
        assert!(!should_try_bwt(&text[..100], a.entropy));
    }

    #[test]
    fn zeros_pick_tzero_or_rle() {
        let data = vec![0u8; 256];
        let a = Analyzer::analyze(&data);
        let p = evolve(&data, &a, &NcaConfig { generations: 2 });
        let desc = p.describe();
        assert!(
            desc.contains("T_ZERO") || desc.contains("RLE") || desc.contains("DEFLATE"),
            "got {desc}"
        );
    }
}
