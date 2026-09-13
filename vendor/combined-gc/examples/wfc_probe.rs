use combined_gc::clean_o1;
use combined_gc::mtf::{mtf_encode, mtf_rle0_encode};
use combined_gc::transforms::bwt_sa_indexed;
use combined_gc::mtf::rle0_from_ranks;
use combined_gc::wfc::{rank_stats, wfc_encode, wfc_freq_encode, wfc_rle0_encode};

fn main() {
    let path = std::env::args().nth(1).expect("file");
    let data = std::fs::read(&path).expect("read");
    let bwt = bwt_sa_indexed(&data, 900_000);
    let mtf_r = mtf_encode(&bwt);
    let wfc_r = wfc_encode(&bwt);
    let wfc_f = wfc_freq_encode(&bwt);
    let (hm, pm, am) = rank_stats(&mtf_r);
    let (hw, pw, aw) = rank_stats(&wfc_r);
    let (hf, pf, af) = rank_stats(&wfc_f);
    let mtf0 = mtf_rle0_encode(&bwt);
    let wfc0 = wfc_rle0_encode(&bwt);
    let wfcf0 = rle0_from_ranks(&wfc_f);
    let mtf_ans = clean_o1::encode_auto(&mtf0);
    let wfc_ans = clean_o1::encode_auto(&wfc0);
    let wfcf_ans = clean_o1::encode_auto(&wfcf0);
    println!("raw={} bwt={}", data.len(), bwt.len());
    println!(
        "MTF  H={:.3} avg_rank={:.3} P0-3={:.3}/{:.3}/{:.3}/{:.3} rle0={} ans={}",
        hm, am, pm[0], pm[1], pm[2], pm[3], mtf0.len(), mtf_ans.len()
    );
    println!(
        "WFC  H={:.3} avg_rank={:.3} P0-3={:.3}/{:.3}/{:.3}/{:.3} rle0={} ans={}",
        hw, aw, pw[0], pw[1], pw[2], pw[3], wfc0.len(), wfc_ans.len()
    );
    println!(
        "WFCf H={:.3} avg_rank={:.3} P0-3={:.3}/{:.3}/{:.3}/{:.3} rle0={} ans={}",
        hf, af, pf[0], pf[1], pf[2], pf[3], wfcf0.len(), wfcf_ans.len()
    );
    let h_gap = (hm - hw) * mtf_r.len() as f64 / 8.0;
    println!(
        "entropy_gap_bytes={:.0} ans_delta={} winner={}",
        h_gap,
        wfc_ans.len() as i64 - mtf_ans.len() as i64,
        if wfc_ans.len() + 8 < mtf_ans.len() {
            "WFC"
        } else if mtf_ans.len() <= wfc_ans.len() {
            "MTF"
        } else {
            "WFC-within-8"
        }
    );
}
