//! PCCX lab-ultimate picker. Overlay onto splb/src/pcc.rs pick_one.

fn pick_one(data: &[u8], class: detect::Class, plan: &crate::autonoma::Plan) -> Block {
    if let Some(z) = zero_block(data) {
        return z;
    }
    let mut best: Option<Block> = None;
    let n = data.len();
    let textish = plan.primary == crate::autonoma::Seat::Bwt;

    if textish || n <= 8 * 1024 * 1024 {
        if let Some(b) = try_bwt(data) {
            take_smaller(&mut best, block(Op::Bwt, data, b));
        }
    }
    if let Some(m) = try_match(data, class, plan.match_window, plan.ml4) {
        take_smaller(&mut best, block(Op::Match, data, m));
    }
    if plan.try_delta {
        if let Some(d) = try_delta_match(data, class, plan.match_window, plan.ml4) {
            take_smaller(&mut best, block(Op::Match, data, d));
        }
    }
    if n <= 12 * 1024 * 1024 {
        if let Some(a) = try_aware(data) {
            take_smaller(&mut best, block(Op::Aware, data, a));
        }
    }
    if let Some(c) = try_cmaq(data) {
        take_smaller(&mut best, block(Op::Cmaq, data, c));
    }
    let weak = best
        .as_ref()
        .map(|b| (b.blob.len() as f64) / (n as f64) > 0.30)
        .unwrap_or(true);
    if weak && n <= 4 * 1024 * 1024 {
        if let Some(z) = try_lz(data) {
            take_smaller(&mut best, block(Op::Lz, data, z));
        }
        if let Some(z) = crate::lzm::encode(data) {
            take_smaller(&mut best, block(Op::Lzm, data, z));
        }
        if let Some(z) = crate::structx::encode(data) {
            take_smaller(&mut best, block(Op::Str, data, z));
        }
    }
    best.unwrap_or_else(|| store_or_math(data))
}
