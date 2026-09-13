//! PCCX nominal picker.
//! Window = champ 4 MiB. Text = BWT then GC if BWT is not already crushed.
//! Binary = MATCH. No zoo on large files.

fn pick_one(data: &[u8], class: detect::Class, plan: &crate::autonoma::Plan) -> Block {
    if let Some(z) = zero_block(data) {
        return z;
    }
    let mut best: Option<Block> = None;
    let n = data.len();
    let textish = plan.primary == crate::autonoma::Seat::Bwt;
    let w = crate::pcc::NOMINAL_WINDOW;

    if textish {
        if let Some(b) = try_bwt(data) {
            take_smaller(&mut best, block(Op::Bwt, data, b));
        }
        let crushed = best
            .as_ref()
            .map(|b| (b.blob.len() as f64) / (n as f64) < 0.18)
            .unwrap_or(false);
        if !crushed {
            if let Some(a) = try_aware(data) {
                take_smaller(&mut best, block(Op::Aware, data, a));
            }
        }
        let weak = best
            .as_ref()
            .map(|b| (b.blob.len() as f64) / (n as f64) > 0.40)
            .unwrap_or(true);
        if weak {
            if let Some(m) = try_match(data, class, w, plan.ml4) {
                take_smaller(&mut best, block(Op::Match, data, m));
            }
        }
    } else {
        if let Some(m) = try_match(data, class, w, plan.ml4) {
            take_smaller(&mut best, block(Op::Match, data, m));
        }
        if plan.try_delta {
            if let Some(d) = try_delta_match(data, class, w, plan.ml4) {
                take_smaller(&mut best, block(Op::Match, data, d));
            }
        }
        let weak = best
            .as_ref()
            .map(|b| (b.blob.len() as f64) / (n as f64) > 0.45)
            .unwrap_or(true);
        if weak && n <= 2 * 1024 * 1024 {
            if let Some(a) = try_aware(data) {
                take_smaller(&mut best, block(Op::Aware, data, a));
            }
        }
    }
    best.unwrap_or_else(|| store_or_math(data))
}
