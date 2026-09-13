//! Compression program VM. A Program is the decompressor stored in the frame.
//!
//! Serialization (`PRG1`) is what goes in the AWAREv5.1 `prog` field.

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Op {
    Store,
    Delta { order: u8 },
    XorFloat,
    Rle,
    /// Stand-in for LZ_OPT: raw DEFLATE via flate2. Not a custom LZ.
    Lz,
    /// Order-0 rANS. Not DEFLATE.
    Ans,
    Bwt,
    Tru8Zero,
    BlackjackDelta2,
    Neural,
    /// Hangry Taylor reciprocal predictor. extra = order 4 or 5. Lossless residual.
    Hangry { order: u8 },
    /// QQ soft-delta. extra = q. LOSSY. Not in default NCA bank.
    Qq { q: u8 },
    /// Move-to-front. Invertible. Lives after BWT.
    Mtf,
    /// LZ against 1MB decoded dict + rANS. Frame-level; needs running dict.
    DictLz,
    /// o0/o1/o2/o4/o5 + match + bwt_rank mixer + range coder.
    MixCm,
    /// Thin zlib (RFC 1950). Used as a whole-file emit, not inside v6.
    Zlib,
    /// Exact 16K-block dedup on post-BWT/MTF bytes.
    Dedup,
    /// XOR_F then 4-byte lane split.
    XorF4,
    /// i16 delta + zigzag varint.
    DeltaVar,
    /// 256K LZP prefilter before BWT.
    Lzp,
    /// Weighted frequency count after BWT. Replaces MTF.
    Wfc,
    /// xz x86 BCJ prefilter (E8/E9 rel32 → abs, MSByte test).
    Bcj,
    /// Thin whole-file LZMA2 (XZ1 container).
    Xz,
    /// Plain u16 LE wrapping delta (keeps 2-byte words, no varint).
    Delta16,
    /// Thin whole-file bzip2 (BZ1 container).
    Bzip,
}

impl Op {
    fn tag(&self) -> u8 {
        match self {
            Op::Store => 0,
            Op::Delta { .. } => 1,
            Op::XorFloat => 2,
            Op::Rle => 3,
            Op::Lz => 4,
            Op::Ans => 5,
            Op::Bwt => 6,
            Op::Tru8Zero => 7,
            Op::BlackjackDelta2 => 8,
            Op::Neural => 9,
            Op::Hangry { .. } => 10,
            Op::Qq { .. } => 11,
            Op::Mtf => 12,
            Op::DictLz => 13,
            Op::MixCm => 14,
            Op::Zlib => 15,
            Op::Dedup => 16,
            Op::XorF4 => 17,
            Op::DeltaVar => 18,
            Op::Lzp => 19,
            Op::Wfc => 20,
            Op::Bcj => 21,
            Op::Xz => 22,
            Op::Delta16 => 23,
            Op::Bzip => 24,
        }
    }

    fn extra(&self) -> u8 {
        match self {
            Op::Delta { order } => *order,
            Op::Hangry { order } => *order,
            Op::Qq { q } => *q,
            _ => 0,
        }
    }

    fn from_tag(tag: u8, extra: u8) -> Option<Self> {
        Some(match tag {
            0 => Op::Store,
            1 => Op::Delta {
                order: extra.max(1),
            },
            2 => Op::XorFloat,
            3 => Op::Rle,
            4 => Op::Lz,
            5 => Op::Ans,
            6 => Op::Bwt,
            7 => Op::Tru8Zero,
            8 => Op::BlackjackDelta2,
            9 => Op::Neural,
            10 => Op::Hangry {
                order: if extra == 4 { 4 } else { 5 },
            },
            11 => Op::Qq {
                q: extra.max(1),
            },
            12 => Op::Mtf,
            13 => Op::DictLz,
            14 => Op::MixCm,
            15 => Op::Zlib,
            16 => Op::Dedup,
            17 => Op::XorF4,
            18 => Op::DeltaVar,
            19 => Op::Lzp,
            20 => Op::Wfc,
            21 => Op::Bcj,
            22 => Op::Xz,
            23 => Op::Delta16,
            24 => Op::Bzip,
            _ => return None,
        })
    }

    pub fn name(&self) -> &'static str {
        match self {
            Op::Store => "STORE",
            Op::Delta { order } if *order == 1 => "DELTA",
            Op::Delta { .. } => "DELTA2",
            Op::XorFloat => "XOR_F",
            Op::Rle => "RLE",
            Op::Lz => "DEFLATE",
            Op::Ans => "ANS",
            Op::Bwt => "BWT16K",
            Op::Tru8Zero => "T_ZERO",
            Op::BlackjackDelta2 => "BJ_DELTA2",
            Op::Neural => "NEURAL",
            Op::Hangry { order: 4 } => "HANGRY4",
            Op::Hangry { .. } => "HANGRY5",
            Op::Qq { .. } => "QQ",
            Op::Mtf => "MTF",
            Op::DictLz => "DICT_LZ",
            Op::MixCm => "MIX_CM",
            Op::Zlib => "ZLIB",
            Op::Dedup => "DEDUP",
            Op::XorF4 => "XOR_F4",
            Op::DeltaVar => "DELTA_V",
            Op::Lzp => "LZP",
            Op::Wfc => "WFC",
            Op::Bcj => "BCJ",
            Op::Xz => "XZ",
            Op::Delta16 => "DELTA16",
            Op::Bzip => "BZIP",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Program {
    pub ops: Vec<Op>,
}

impl Program {
    pub fn store() -> Self {
        Self {
            ops: vec![Op::Store],
        }
    }

    pub fn describe(&self) -> String {
        if self.ops.is_empty() {
            return "STORE".into();
        }
        self.ops
            .iter()
            .map(|o| o.name())
            .collect::<Vec<_>>()
            .join("+")
    }

    pub fn size(&self) -> usize {
        4 + self.ops.len() * 2
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = b"PRG1".to_vec();
        out.push(self.ops.len().min(255) as u8);
        for op in self.ops.iter().take(255) {
            out.push(op.tag());
            out.push(op.extra());
        }
        out
    }

    pub fn from_bytes(buf: &[u8]) -> Option<Self> {
        if buf.len() < 5 || &buf[0..4] != b"PRG1" {
            return None;
        }
        let n = buf[4] as usize;
        if buf.len() < 5 + n * 2 {
            return None;
        }
        let mut ops = Vec::with_capacity(n);
        let mut p = 5;
        for _ in 0..n {
            ops.push(Op::from_tag(buf[p], buf[p + 1])?);
            p += 2;
        }
        if ops.is_empty() {
            ops.push(Op::Store);
        }
        Some(Self { ops })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn program_roundtrip_bytes() {
        let p = Program {
            ops: vec![Op::XorFloat, Op::Delta { order: 1 }, Op::Lz],
        };
        let q = Program::from_bytes(&p.to_bytes()).unwrap();
        assert_eq!(p, q);
        assert_eq!(p.describe(), "XOR_F+DELTA+DEFLATE");
    }
}
