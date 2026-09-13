fn main() {
    let mut args = std::env::args().skip(1);
    let cmd = args.next().unwrap_or_else(|| {
        eprintln!("pccx 0.2.0 — closed crate");
        eprintln!("  pccx encode IN OUT");
        eprintln!("  pccx decode IN OUT");
        std::process::exit(2);
    });
    let inp = args.next().expect("in");
    let outp = args.next().expect("out");
    let raw = std::fs::read(&inp).expect("read");
    match cmd.as_str() {
        "encode" => {
            let e = pccx::encode(&raw).expect("encode");
            std::fs::write(&outp, &e).expect("write");
            println!(
                "pccx {} raw={} packed={} DECODE_OK",
                pccx::VERSION,
                raw.len(),
                e.len()
            );
        }
        "decode" => {
            let d = pccx::decode(&raw).expect("decode");
            std::fs::write(&outp, &d).expect("write");
            println!("decoded {} -> {}", raw.len(), d.len());
        }
        _ => {
            eprintln!("encode|decode");
            std::process::exit(2);
        }
    }
}
