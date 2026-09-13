//! xz/7-zip x86 BCJ — relative CALL/JMP (E8/E9) → absolute.
//!
//! Matches liblzma `simple/x86.c` (MSByte test + 3-byte prev_mask).
//! Naive "convert every E8" lost 382KB on mozilla+zlib. This one only
//! converts when the displacement MSByte is 0x00 or 0xFF.

const MASK_ALLOWED: [bool; 8] = [true, true, true, false, true, false, false, false];
const MASK_BIT: [u8; 8] = [0, 1, 2, 2, 3, 3, 3, 3];

#[inline]
fn test_msbyte(b: u8) -> bool {
    b == 0x00 || b == 0xFF
}

fn convert(data: &[u8], encode: bool) -> Vec<u8> {
    let mut buf = data.to_vec();
    let size = buf.len();
    if size < 5 {
        return buf;
    }
    let mut prev_pos: isize = -1;
    let mut prev_mask: u32 = 0;
    let mut i = 0usize;
    while i + 4 < size {
        if buf[i] != 0xE8 && buf[i] != 0xE9 {
            i += 1;
            continue;
        }
        let dist = i as isize - prev_pos;
        prev_pos = i as isize;
        if dist > 3 {
            prev_mask = 0;
        } else {
            prev_mask = (prev_mask << dist as u32) & 7;
            if prev_mask != 0 {
                let idx = i + 4 - MASK_BIT[prev_mask as usize] as usize;
                let b = buf[idx];
                if !MASK_ALLOWED[prev_mask as usize] || test_msbyte(b) {
                    prev_mask = ((prev_mask << 1) | 1) & 7;
                    i += 1;
                    continue;
                }
            }
        }
        prev_mask = ((prev_mask << 1) | 1) & 7;
        if test_msbyte(buf[i + 4]) {
            let mut src = u32::from_le_bytes([buf[i + 1], buf[i + 2], buf[i + 3], buf[i + 4]]);
            let pos = i as u32;
            let mut dest = src;
            for _ in 0..8 {
                dest = if encode {
                    src.wrapping_add(pos.wrapping_add(5))
                } else {
                    src.wrapping_sub(pos.wrapping_add(5))
                };
                if prev_mask == 0 {
                    break;
                }
                let bit = MASK_BIT[prev_mask as usize];
                let shift = 24 - 8 * bit as u32;
                let b = (dest >> shift) as u8;
                if !test_msbyte(b) {
                    break;
                }
                let xor_mask = (1u32 << (32 - 8 * bit as u32)) - 1;
                src = dest ^ xor_mask;
            }
            buf[i + 1..i + 5].copy_from_slice(&dest.to_le_bytes());
            i += 5;
        } else {
            i += 1;
        }
    }
    buf
}

pub fn encode(data: &[u8]) -> Vec<u8> {
    convert(data, true)
}

pub fn decode(data: &[u8]) -> Vec<u8> {
    convert(data, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_on_text() {
        let s = b"the quick brown fox jumps over the lazy dog\n".repeat(20);
        let e = encode(&s);
        assert_eq!(decode(&e), s);
    }

    #[test]
    fn call_rel32_roundtrip() {
        // e8 + rel32 that looks like a near call (MSByte 0xFF)
        let mut s = vec![0x90u8; 32];
        s[8] = 0xE8;
        s[9..13].copy_from_slice(&(-16i32).to_le_bytes());
        let e = encode(&s);
        assert_eq!(decode(&e), s);
        // filter should have rewritten the displacement
        assert_ne!(&e[9..13], &s[9..13]);
    }
}
