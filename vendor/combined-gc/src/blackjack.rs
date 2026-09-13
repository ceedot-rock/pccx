//! Δ² / Rice / Fib sketches from the V2 dump.
//!
//! Honest: Rice here writes unary as whole bytes, not a bit coder.
//! Fib is a bit-vector, not a packed bitstream. Not the $199 Blackjack SKU.

pub fn delta2_encode(x: &[i32]) -> Vec<u8> {
    if x.len() < 3 {
        return Vec::new();
    }
    let d1: Vec<i32> = x.windows(2).map(|w| w[1] - w[0]).collect();
    let d2: Vec<i32> = d1.windows(2).map(|w| w[1] - w[0]).collect();
    let mut out = Vec::new();
    for &v in &d2 {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

pub fn rice_encode(val: u32, k: u8) -> Vec<u8> {
    let q = val >> k;
    let r = val & ((1u32 << k) - 1);
    let mut out = Vec::new();
    for _ in 0..q {
        out.push(1);
    }
    out.push(0);
    out.push(r as u8);
    out
}

pub fn fib_encode(n: usize) -> Vec<u8> {
    let mut a = 1usize;
    let mut b = 2usize;
    let mut bits = Vec::new();
    let mut m = n;
    while m > 0 {
        if m >= b {
            bits.push(1);
            m -= b;
        } else {
            bits.push(0);
        }
        let c = a.saturating_add(b);
        a = b;
        b = c;
        if b == 0 {
            break;
        }
    }
    bits.push(1);
    bits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delta2_short_is_empty() {
        assert!(delta2_encode(&[1, 2]).is_empty());
        assert_eq!(delta2_encode(&[1, 2, 4]).len(), 4);
    }
}
