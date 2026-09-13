//! AWARE frame: analyze → search Program → residual → 5.1 container.

use crate::analyzer::{Analysis, Analyzer};
use crate::aware_container::{self, AwareBlock};
use crate::aware_v6::{self, AwareV6, V6Block};
use crate::cm;
use crate::dict;
use crate::nca;
use crate::pipeline;
use crate::vm::{Op, Program};
use crate::zrw;

pub const BLOCK: usize = 16_384;
/// 64K pack for small slices (gzip-window). 900K mid. 2MB for large files.
pub const PACK: usize = 65_536;
pub const PACK_900K: usize = 900_000;
pub const PACK_2M: usize = 2_000_000;
pub const PACK_4M: usize = 4_000_000;
pub const PACK_8M: usize = 8_000_000;

pub fn pack_size(n: usize) -> usize {
    crate::pack::pack_size(n as u64)
}

fn best_thin_wrap(data: &[u8]) -> (Vec<u8>, Program) {
    let mut best = pipeline::emit_thin_zlib(data);
    let mut prog = Program {
        ops: vec![Op::Zlib],
    };
    if data.len() >= 4096 {
        if let Some(xz) = crate::xz_thin::emit_thin_xz(data) {
            if xz.len() < best.len() {
                best = xz;
                prog = Program {
                    ops: vec![Op::Xz],
                };
            }
        }
        if let Some(bz) = crate::bz_thin::emit_thin_bz(data) {
            if bz.len() < best.len() {
                best = bz;
                prog = Program {
                    ops: vec![Op::Bzip],
                };
            }
        }
    }
    (best, prog)
}

fn pack_mtf_seed(chunk_len: usize, last: Option<&[u8; 256]>) -> [u8; 256] {
    if chunk_len < PACK {
        last.copied().unwrap_or_else(crate::mtf::identity_list)
    } else {
        crate::mtf::identity_list()
    }
}

fn seed_for_pack(chunk_len: usize, seed: &[u8; 256]) -> Option<&[u8; 256]> {
    if chunk_len < PACK {
        Some(seed)
    } else {
        None
    }
}

fn advance_mtf_state(chunk: &[u8], program: &Program, init: &[u8; 256]) -> Option<[u8; 256]> {
    if chunk.len() >= PACK || !program.ops.iter().any(|o| matches!(o, Op::Mtf)) {
        return None;
    }
    let bwt = crate::transforms::bwt_sa_indexed(chunk, crate::transforms::BWT_BIG);
    Some(crate::mtf::mtf_list_after(&bwt, init))
}

pub fn parse_block_arg(s: &str) -> Option<usize> {
    let t = s.trim().to_ascii_uppercase();
    if t.ends_with('K') {
        t[..t.len() - 1].parse::<usize>().ok().map(|k| k * 1000)
    } else if t.ends_with('M') {
        t[..t.len() - 1].parse::<usize>().ok().map(|m| m * 1_000_000)
    } else {
        t.parse().ok()
    }
}

pub struct CompressResult {
    pub bytes: Vec<u8>,
    pub program: Program,
    pub analysis: Analysis,
}

pub fn compress(data: &[u8]) -> CompressResult {
    compress_with_pack(data, pack_size(data.len()))
}

pub fn compress_with_pack(data: &[u8], pack: usize) -> CompressResult {
    let analysis = Analyzer::analyze(data);
    if zrw::is_zero_run(data) {
        let program = Program {
            ops: vec![crate::vm::Op::Tru8Zero],
        };
        let residual = zrw::compress_zeros_int32_le(data.len());
        let bytes = aware_container::encode_aware(
            vec![AwareBlock {
                prog: program.to_bytes(),
                residual,
                orig_len: data.len() as u32,
            }],
            data.len() as u64,
        );
        return CompressResult {
            bytes,
            program,
            analysis,
        };
    }

    let (wrap_bytes, wrap_prog) = best_thin_wrap(data);
    if !nca::should_try_bwt(data, analysis.entropy) {
        return CompressResult {
            bytes: wrap_bytes,
            program: wrap_prog,
            analysis,
        };
    }

    // First-pack probe: if BWT projects worse than whole-file wrap, skip remaining SA.
    let pack = pack.max(1);
    if data.len() > pack && pack >= 4096 {
        let pack0 = &data[..pack];
        let a0 = Analyzer::analyze(pack0);
        if nca::should_try_bwt(pack0, a0.entropy) {
            let probe = nca::pick_best(pack0);
            let probe_res = pipeline::apply(&probe, pack0);
            let projected = (probe_res.len() as u64)
                .saturating_mul(data.len() as u64)
                / pack0.len().max(1) as u64;
            if projected + 8 >= wrap_bytes.len() as u64 {
                return CompressResult {
                    bytes: wrap_bytes,
                    program: wrap_prog,
                    analysis,
                };
            }
        }
    }

    if !analysis.is_compressible() {
        // Last chance: a cheap delta can still crush ramps that look random in H.
        let store = Program::store();
        let probe = Program {
            ops: vec![crate::vm::Op::Delta { order: 1 }, crate::vm::Op::Lz],
        };
        let store_fit = pipeline::fitness(&store, data);
        let probe_fit = pipeline::fitness(&probe, data);
        let program = if probe_fit + 16 < store_fit {
            probe
        } else {
            store
        };
        if matches!(program.ops.first(), Some(crate::vm::Op::Store)) {
            let bytes = aware_container::encode_aware(
                vec![AwareBlock {
                    prog: program.to_bytes(),
                    residual: data.to_vec(),
                    orig_len: data.len() as u32,
                }],
                data.len() as u64,
            );
            return CompressResult {
                bytes,
                program,
                analysis,
            };
        }
        let residual = pipeline::apply(&program, data);
        let bytes = aware_container::encode_aware(
            vec![AwareBlock {
                prog: program.to_bytes(),
                residual,
                orig_len: data.len() as u32,
            }],
            data.len() as u64,
        );
        return CompressResult {
            bytes,
            program,
            analysis,
        };
    }

    let mut blocks = Vec::new();
    let mut first_prog: Option<Program> = None;
    let mut hist = Vec::new();
    let mut v6 = AwareV6::default();
    let mut last_mtf: Option<[u8; 256]> = None;
    let hist_cap = dict::hist_cap_for(data);
    for chunk in data.chunks(pack) {
        let seed = pack_mtf_seed(chunk.len(), last_mtf.as_ref());
        let seed_ref = seed_for_pack(chunk.len(), &seed);
        let intra = nca::pick_best(chunk);
        let intra_res = pipeline::apply_seeded(&intra, chunk, seed_ref);
        let dlz_prog = Program {
            ops: vec![Op::DictLz],
        };
        let dlz_res = crate::lz_opt::encode(chunk, &hist);
        let dlz_bwt_prog = Program {
            ops: vec![Op::DictLz, Op::Bwt, Op::Mtf, Op::Ans],
        };
        let dlz_bwt_res = crate::lz_opt::encode_pre_bwt(chunk, &hist, seed_ref);
        let mut program = intra;
        let mut residual = intra_res;
        if dlz_res.len() + 8 < residual.len() {
            program = dlz_prog;
            residual = dlz_res;
        }
        if dlz_bwt_res.len() + 8 < residual.len() {
            program = dlz_bwt_prog;
            residual = dlz_bwt_res;
        }
        let xform_bank = if crate::analyzer::is_text_like(chunk) {
            Vec::new()
        } else {
            vec![
                Program {
                    ops: vec![Op::XorFloat, Op::DictLz, Op::Bwt, Op::Mtf, Op::Ans],
                },
                Program {
                    ops: vec![
                        Op::Delta { order: 2 },
                        Op::DictLz,
                        Op::Bwt,
                        Op::Mtf,
                        Op::Ans,
                    ],
                },
            ]
        };
        for p in &xform_bank {
            let r = pipeline::apply_seeded_dict(p, chunk, seed_ref, &[]);
            if r.len() + 8 < residual.len() {
                program = p.clone();
                residual = r;
            }
        }
        if chunk.len() <= 65_536 {
            let cm_res = cm::cm_encode(chunk);
            if cm_res.len() + 8 < residual.len() {
                program = Program {
                    ops: vec![Op::MixCm],
                };
                residual = cm_res;
            }
        }
        #[cfg(feature = "cm")]
        if crate::analyzer::is_text_like(chunk) && chunk.len() <= 2_000_000 {
            let pre = Program {
                ops: vec![Op::Bwt, Op::Mtf],
            };
            let mtf = pipeline::apply_seeded(&pre, chunk, seed_ref);
            let cm_res = cm::cm_encode(&mtf);
            if cm_res.len() + 8 < residual.len() {
                program = Program {
                    ops: vec![Op::Bwt, Op::Mtf, Op::MixCm],
                };
                residual = cm_res;
            }
        }
        let z = pipeline::zlib_encode(chunk);
        if z.len() + 1 < residual.len() {
            program = Program {
                ops: vec![Op::Zlib],
            };
            residual = z;
        }
        if !crate::analyzer::is_text_like(chunk) {
            let bcj_z = pipeline::apply(
                &Program {
                    ops: vec![Op::Bcj, Op::Zlib],
                },
                chunk,
            );
            if bcj_z.len() + 8 < residual.len() {
                program = Program {
                    ops: vec![Op::Bcj, Op::Zlib],
                };
                residual = bcj_z;
            }
        }
        if first_prog.is_none() {
            first_prog = Some(program.clone());
        }
        blocks.push(V6Block {
            prog: program.to_bytes(),
            residual,
            orig_len: chunk.len() as u32,
        });
        dict::push_dict_n(&mut hist, chunk, hist_cap);
        v6.update(chunk);
        last_mtf = advance_mtf_state(chunk, &program, &seed);
    }
    let bytes = aware_v6::encode_v6(blocks, data.len() as u64);
    let program = first_prog.unwrap_or_else(Program::store);
    // Whole-file zlib-ng bake-off. Beats v6 on small binaries when RLE/pack
    // split hides 3-byte hashes. Loses to BWT+ANS on text.
    let thin = pipeline::emit_thin_zlib(data);
    let (bytes, program) = if thin.len() < bytes.len() {
        (
            thin,
            Program {
                ops: vec![Op::Zlib],
            },
        )
    } else {
        (bytes, program)
    };
    // Whole-file LZMA2. Text included so webster can take xz; dickens BWT still wins +8.
    let (bytes, program) = if data.len() >= 4096 {
        if let Some(xz) = crate::xz_thin::emit_thin_xz(data) {
            if xz.len() < bytes.len() {
                (
                    xz,
                    Program {
                        ops: vec![Op::Xz],
                    },
                )
            } else {
                (bytes, program)
            }
        } else {
            (bytes, program)
        }
    } else {
        (bytes, program)
    };
    // Whole-file bzip2. Closes mr vs bzip2-9; dickens/xml BWT still wins +8.
    let (bytes, program) = if data.len() >= 4096 {
        if let Some(bz) = crate::bz_thin::emit_thin_bz(data) {
            if bz.len() < bytes.len() {
                (
                    bz,
                    Program {
                        ops: vec![Op::Bzip],
                    },
                )
            } else {
                (bytes, program)
            }
        } else {
            (bytes, program)
        }
    } else {
        (bytes, program)
    };
    // Per-bit NCA mix (NCM1). Lockstep decode. +8 bake-off.
    // Cap 2M so mozilla/samba don't pay 50MB of bit loops.
    let (bytes, program) = if data.len() <= 2_000_000 && analysis.entropy < 6.8 {
        let ncm = crate::nca_mix::encode(data);
        if ncm.len() + 8 < bytes.len() {
            (
                ncm,
                Program {
                    ops: vec![Op::Neural],
                },
            )
        } else {
            (bytes, program)
        }
    } else {
        (bytes, program)
    };
    CompressResult {
        bytes,
        program,
        analysis,
    }
}

pub fn decompress(buf: &[u8]) -> Result<Vec<u8>, &'static str> {
    if buf.len() == 8 && buf.starts_with(zrw::MAGIC) {
        return zrw::decompress_zeros(buf);
    }
    if buf.starts_with(pipeline::ZLB_MAGIC) {
        return pipeline::parse_thin_zlib(buf);
    }
    if buf.starts_with(crate::xz_thin::XZ_MAGIC) {
        return crate::xz_thin::parse_thin_xz(buf);
    }
    if buf.starts_with(crate::bz_thin::BZ_MAGIC) {
        return crate::bz_thin::parse_thin_bz(buf);
    }
    if buf.starts_with(crate::nca_mix::MAGIC) {
        return crate::nca_mix::decode(buf);
    }
    if buf.starts_with(aware_v6::MAGIC) {
        let (orig_total, blocks) = aware_v6::parse_v6(buf)?;
        let mut out = Vec::with_capacity(orig_total as usize);
        let mut hist = Vec::new();
        let mut v6 = AwareV6::default();
        let mut last_mtf: Option<[u8; 256]> = None;
        let hist_cap = if orig_total > 4_000_000 {
            dict::DICT_CAP_LONG
        } else {
            dict::DICT_CAP
        };
        for b in blocks {
            let program = Program::from_bytes(&b.prog).ok_or("frame: bad program")?;
            let seed = pack_mtf_seed(b.orig_len as usize, last_mtf.as_ref());
            let seed_ref = seed_for_pack(b.orig_len as usize, &seed);
            let chunk = if program.ops.iter().any(|o| matches!(o, Op::MixCm)) {
                let mid = cm::cm_decode(&b.residual)?;
                if program.ops.iter().all(|o| matches!(o, Op::MixCm)) {
                    mid
                } else {
                    let rest: Vec<Op> = program
                        .ops
                        .iter()
                        .filter(|o| !matches!(o, Op::MixCm))
                        .cloned()
                        .collect();
                    pipeline::invert_seeded_dict(
                        &Program { ops: rest },
                        &mid,
                        b.orig_len as usize,
                        seed_ref,
                        &[],
                    )?
                }
            } else if program.ops.iter().any(|o| matches!(o, Op::DictLz))
                && b.residual.len() >= 4
                && (b.residual.starts_with(crate::lz_opt::MAGICP)
                    || b.residual.starts_with(crate::lz_opt::MAGIC2)
                    || b.residual.starts_with(crate::dict::MAGIC))
            {
                crate::lz_opt::decode_seeded(&b.residual, &hist, seed_ref)?
            } else {
                pipeline::invert_seeded_dict(
                    &program,
                    &b.residual,
                    b.orig_len as usize,
                    seed_ref,
                    &[],
                )?
            };
            dict::push_dict_n(&mut hist, &chunk, hist_cap);
            v6.update(&chunk);
            last_mtf = advance_mtf_state(&chunk, &program, &seed);
            out.extend_from_slice(&chunk);
        }
        out.truncate(orig_total as usize);
        return Ok(out);
    }
    let parsed = aware_container::parse_aware(buf)?;
    let mut out = Vec::with_capacity(parsed.orig_total as usize);
    let mut hist = Vec::new();
    for b in parsed.blocks {
        let program = Program::from_bytes(&b.prog).ok_or("frame: bad program")?;
        let chunk = if program.ops.iter().any(|o| matches!(o, Op::DictLz)) {
            crate::lz_opt::decode_seeded(&b.residual, &hist, None)?
        } else if program.ops.iter().any(|o| matches!(o, Op::MixCm)) {
            cm::cm_decode(&b.residual)?
        } else {
            pipeline::invert(&program, &b.residual, b.orig_len as usize)?
        };
        dict::push_dict(&mut hist, &chunk);
        out.extend_from_slice(&chunk);
    }
    out.truncate(parsed.orig_total as usize);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeded_two_pack_32k() {
        let phrase = b"the quick brown fox jumps over the lazy dog\n".repeat(700);
        let src = [phrase.as_slice(), phrase.as_slice()].concat();
        assert!(src.len() > 32_768 && src.len() < 90_000);
        let c = compress_with_pack(&src, 32_768);
        let back = decompress(&c.bytes).expect("decode");
        assert_eq!(back, src);
    }

    #[test]
    fn pack_2m_cap() {
        crate::transforms::set_last_sa_ms(0);
        assert_eq!(pack_size(3_000_000), PACK_2M);
        assert_eq!(pack_size(5_000_000), PACK_2M);
        assert_eq!(pack_size(9_000_000), PACK_2M);
        assert_eq!(pack_size(900_000), PACK_900K);
        crate::transforms::set_last_sa_ms(500);
        assert_eq!(pack_size(5_000_000), PACK_2M);
    }

    #[test]
    fn dict_two_block_roundtrip() {
        let phrase = b"the quick brown fox jumps over the lazy dog\n".repeat(1600);
        let src = [phrase.as_slice(), phrase.as_slice()].concat();
        let c = compress(&src);
        let back = decompress(&c.bytes).expect("decode");
        assert_eq!(back, src);
    }

    #[test]
    fn thin_zlib_obj_like() {
        // Binary-ish bytes so pick_best prefers DEFLATE and thin ZLB1 can win.
        let mut src = Vec::new();
        for i in 0..8000u32 {
            src.extend_from_slice(&i.to_le_bytes());
            src.push((i % 17) as u8);
        }
        let c = compress(&src);
        let back = decompress(&c.bytes).expect("decode");
        assert_eq!(back, src);
    }

    fn text_roundtrip() {
        let src = b"hello world this is text for the gate and the frame\n".repeat(8);
        let c = compress(&src);
        let back = decompress(&c.bytes).expect("decode");
        assert_eq!(back, src);
    }

    #[test]
    fn zeros_roundtrip() {
        let src = vec![0u8; 10_000];
        let c = compress(&src);
        assert_eq!(c.program.describe(), "T_ZERO");
        let back = decompress(&c.bytes).unwrap();
        assert_eq!(back, src);
        assert!(c.bytes.len() < 64, "zeros should stay tiny, got {}", c.bytes.len());
    }

    #[test]
    fn random_store_roundtrip() {
        let src: Vec<u8> = (0..=255).cycle().take(512).collect();
        let c = compress(&src);
        let back = decompress(&c.bytes).unwrap();
        assert_eq!(back, src);
    }
}
