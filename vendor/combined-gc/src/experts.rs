//! 100 cheap structure experts. Classify 4K, activate top 5.
//! They do not replace BWT+MTF+ANS. They decide whether to pay SA.

pub const N_EXPERTS: usize = 100;
pub const TOP_K: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    Text,
    Exe,
    Dicom,
    Markup,
    Structured,
    Generic,
}

#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub id: u8,
    pub family: Family,
    pub score: u16,
}

#[derive(Clone, Debug)]
pub struct Activation {
    pub hits: [Hit; TOP_K],
    pub n: usize,
    pub family: Family,
}

impl Activation {
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    pub fn best(&self) -> Option<Family> {
        if self.n == 0 {
            None
        } else {
            Some(self.family)
        }
    }
}

fn score_text(s: &[u8]) -> u16 {
    if s.is_empty() {
        return 0;
    }
    let mut good = 0u32;
    let mut space = 0u32;
    for &b in s {
        if (32..=126).contains(&b) || b == b'\n' || b == b'\r' || b == b'\t' {
            good += 1;
        }
        if b == b' ' {
            space += 1;
        }
    }
    let n = s.len() as u32;
    let pr = good * 100 / n;
    if pr < 70 {
        return 0;
    }
    ((pr + space * 20 / n.max(1)) as u16).min(1000)
}

fn score_exe(s: &[u8]) -> u16 {
    let mut sc = 0u16;
    if s.len() >= 2 && s[0] == b'M' && s[1] == b'Z' {
        sc += 400;
    }
    if s.len() >= 4 && s.starts_with(b"\x7fELF") {
        sc += 400;
    }
    if s.windows(4).any(|w| w == b"\x90\x90\x90\x90") {
        sc += 80;
    }
    let mut nops = 0u32;
    for &b in s.iter().take(4096) {
        if b == 0x90 {
            nops += 1;
        }
    }
    if nops > 32 {
        sc += 40;
    }
    sc.min(1000)
}

fn score_dicom(s: &[u8]) -> u16 {
    let mut sc = 0u16;
    if s.len() > 132 && &s[128..132] == b"DICM" {
        sc += 500;
    }
    // (gggg,eeee) often has even offsets and many zeros in group
    let mut even_zero = 0u32;
    for chunk in s.chunks(2).take(256) {
        if chunk == [0, 0] {
            even_zero += 1;
        }
    }
    if even_zero > 40 {
        sc += 80;
    }
    sc.min(1000)
}

fn score_markup(s: &[u8]) -> u16 {
    let mut lt = 0u32;
    let mut gt = 0u32;
    for &b in s {
        if b == b'<' {
            lt += 1;
        }
        if b == b'>' {
            gt += 1;
        }
    }
    if lt < 8 || gt < 8 {
        return 0;
    }
    let pair = lt.min(gt);
    (pair * 8).min(1000) as u16
}

fn score_structured(s: &[u8]) -> u16 {
    if s.len() < 16 {
        return 0;
    }
    // repeating 2/4-byte stride — geo / int arrays
    let mut d2 = 0u32;
    for w in s.windows(4) {
        if w[0] == w[2] {
            d2 += 1;
        }
    }
    let n = (s.len() - 3) as u32;
    ((d2 * 200) / n.max(1)).min(400) as u16
}

fn score_generic(s: &[u8], order: u32) -> u16 {
    if s.len() < 64 {
        return 0;
    }
    let step = order.max(1) as usize;
    let mut same = 0u32;
    let mut tot = 0u32;
    let mut i = step;
    while i < s.len().min(4096) {
        if s[i] == s[i - step] {
            same += 1;
        }
        tot += 1;
        i += step;
    }
    if tot == 0 {
        return 0;
    }
    ((same * 300) / tot).min(300) as u16
}

/// 100 slots: 0-9 text, 10-19 exe, 20-29 dicom, 30-39 markup,
/// 40-49 structured, 50-99 generic order/sparse.
pub fn score_id(id: u8, sample: &[u8]) -> (Family, u16) {
    match id {
        0..=9 => (Family::Text, score_text(sample) / ((id % 3) as u16 + 1)),
        10..=19 => (Family::Exe, score_exe(sample) / ((id % 3) as u16 + 1)),
        20..=29 => (Family::Dicom, score_dicom(sample) / ((id % 3) as u16 + 1)),
        30..=39 => (Family::Markup, score_markup(sample) / ((id % 3) as u16 + 1)),
        40..=49 => (Family::Structured, score_structured(sample) / ((id % 3) as u16 + 1)),
        _ => {
            let order = 1 + ((id as u32 - 50) % 16);
            (Family::Generic, score_generic(sample, order))
        }
    }
}

pub fn classify(pack: &[u8]) -> Activation {
    let sample = if pack.len() > 4096 { &pack[..4096] } else { pack };
    let mut hits = [Hit {
        id: 0,
        family: Family::Generic,
        score: 0,
    }; TOP_K];
    let mut n = 0usize;
    for id in 0..N_EXPERTS as u8 {
        let (family, score) = score_id(id, sample);
        if score < 40 {
            continue;
        }
        if n < TOP_K {
            hits[n] = Hit { id, family, score };
            n += 1;
            hits[..n].sort_by(|a, b| b.score.cmp(&a.score));
        } else if score > hits[TOP_K - 1].score {
            hits[TOP_K - 1] = Hit { id, family, score };
            hits.sort_by(|a, b| b.score.cmp(&a.score));
        }
    }
    let family = if n == 0 {
        Family::Generic
    } else {
        hits[0].family
    };
    Activation { hits, n, family }
}

/// Pay SA only if H allows it and experts didn't call the pack "small exe / empty".
pub fn should_pay_sa(data: &[u8], entropy: f64) -> bool {
    if !crate::nca::should_try_bwt(data, entropy) {
        return false;
    }
    let act = classify(data);
    if act.is_empty() && entropy >= 6.8 {
        return false;
    }
    // Small executables: ZLIB already wins (obj1 10,322). Don't pay SA.
    if act.family == Family::Exe && data.len() < 128 * 1024 {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_activates_text_family() {
        let s = b"The Pickwick Papers, Chapter I. Mr Pickwick.\n".repeat(80);
        let a = classify(&s);
        assert!(!a.is_empty());
        assert_eq!(a.family, Family::Text);
    }

    #[test]
    fn mz_activates_exe() {
        let mut s = vec![0u8; 8192];
        s[0] = b'M';
        s[1] = b'Z';
        s[64..68].copy_from_slice(b"\x90\x90\x90\x90");
        let a = classify(&s);
        assert_eq!(a.family, Family::Exe);
        assert!(!should_pay_sa(&s, 6.0));
    }

    #[test]
    fn dicom_magic() {
        let mut s = vec![0u8; 256];
        s[128..132].copy_from_slice(b"DICM");
        let a = classify(&s);
        assert_eq!(a.family, Family::Dicom);
    }

    #[test]
    fn high_h_no_sa() {
        let flat: Vec<u8> = (0..8192).map(|i| i as u8).collect();
        assert!(!should_pay_sa(&flat, 8.0));
    }
}
