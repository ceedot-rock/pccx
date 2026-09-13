use combined_gc::clean_o1;
use combined_gc::clean_o16;
use combined_gc::clean_o8;
use combined_gc::mtf::mtf_rle0_encode;
use combined_gc::transforms::bwt_sa_indexed;

fn main() {
    let path = std::env::args().nth(1).expect("file");
    let data = std::fs::read(&path).expect("read");
    let bwt = bwt_sa_indexed(&data, 900_000);
    let mtf = mtf_rle0_encode(&bwt);
    let c8 = clean_o8::encode(&mtf);
    let c16 = clean_o16::encode(&mtf);
    let auto = clean_o1::encode_auto(&mtf);
    println!(
        "raw={} mtf={} cln8={} cln16={} auto={} auto_magic={:?}",
        data.len(),
        mtf.len(),
        c8.len(),
        c16.len(),
        auto.len(),
        std::str::from_utf8(&auto[..4])
    );
}
