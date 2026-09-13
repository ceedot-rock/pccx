use combined_gc::clean_o1;
use combined_gc::mtf::{mtf_encode, mtf_rle0_encode};
use combined_gc::transforms::bwt_sa_indexed;

fn main() {
    let path = std::env::args().nth(1).expect("file");
    let data = std::fs::read(&path).expect("read");
    let bwt = bwt_sa_indexed(&data, 900_000);
    let rle0 = mtf_rle0_encode(&bwt);
    let raw = mtf_encode(&bwt);
    let a = clean_o1::encode_auto(&rle0);
    let b = clean_o1::encode_auto(&raw);
    println!(
        "raw={} bwt={} mtf_rle0={} mtf_raw={} ans_rle0={} ans_raw={}",
        data.len(),
        bwt.len(),
        rle0.len(),
        raw.len(),
        a.len(),
        b.len()
    );
}
