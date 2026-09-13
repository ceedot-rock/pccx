use combined_gc::frame;
use combined_gc::pipeline;
use combined_gc::vm::{Op, Program};

fn main() {
    let path = std::env::args().nth(1).expect("file");
    let pack: usize = std::env::args()
        .nth(2)
        .and_then(|s| frame::parse_block_arg(&s))
        .unwrap_or(2_000_000);
    let data = std::fs::read(&path).expect("read");
    let prog = Program {
        ops: vec![Op::Bwt, Op::Mtf, Op::Ans],
    };
    let mut total = 24usize; // v6 header-ish
    for chunk in data.chunks(pack) {
        let r = pipeline::apply(&prog, chunk);
        total += 16 + r.len();
        print!("{} ", r.len());
    }
    println!();
    println!(
        "file={} raw={} pack={} aware_est={} raw_ratio={:.4}",
        path,
        data.len(),
        pack,
        total,
        total as f64 / data.len() as f64
    );
}
