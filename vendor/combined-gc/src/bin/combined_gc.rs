use anyhow::{bail, Result};
use clap::Parser;
use combined_gc::codec::{self, Mode};
use combined_gc::frame;
use combined_gc::VERSION;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "combined_gc", version = VERSION)]
#[command(about = "Combined GC codec — smart pack default, --fast zlib, --max bake-off")]
struct Args {
    #[arg(short, long)]
    input: PathBuf,

    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Force zlib/xz only. No BWT.
    #[arg(long)]
    fast: bool,

    /// BWT / DELTA / XOR bake-off. Slow. Ratio table.
    #[arg(long)]
    max: bool,

    /// Print sizes vs gzip-9 (uses --max path so locks stay comparable)
    #[arg(long)]
    bench: bool,

    /// Decode ZRW / ZLB1 / XZ1 / AWAREv6
    #[arg(long, short = 'd')]
    decompress: bool,

    /// Superblock size for --max only, e.g. 900K
    #[arg(long)]
    block: Option<String>,
}

fn gzip9_len(data: &[u8]) -> usize {
    let mut enc = GzEncoder::new(Vec::new(), Compression::best());
    enc.write_all(data).ok();
    enc.finish().map(|v| v.len()).unwrap_or(data.len())
}

fn main() -> Result<()> {
    let args = Args::parse();
    let data = fs::read(&args.input)?;

    if args.decompress {
        let r = codec::decode_report(&data).map_err(|e| anyhow::anyhow!(e))?;
        println!(
            "decoded {} -> {} bytes  container={} family={:?}",
            args.input.display(),
            r.bytes.len(),
            r.container,
            r.family
        );
        if let Some(p) = args.output {
            fs::write(p, r.bytes)?;
        }
        return Ok(());
    }

    let mode = if args.fast {
        Mode::Fast
    } else if args.max || args.bench {
        Mode::Max
    } else {
        Mode::Smart
    };

    let result = match mode {
        Mode::Max => {
            let pack = args
                .block
                .as_deref()
                .and_then(frame::parse_block_arg)
                .unwrap_or_else(|| frame::pack_size(data.len()));
            frame::compress_with_pack(&data, pack)
        }
        Mode::Smart => codec::encode(&data, Mode::Smart),
        Mode::Fast => codec::encode(&data, Mode::Fast),
    };

    println!(
        "combined-gc {}  mode={}  file={}  raw={}  packed={}  program={}",
        VERSION,
        mode.as_str(),
        args.input.display(),
        data.len(),
        result.bytes.len(),
        result.program.describe()
    );

    if args.bench {
        let gz = gzip9_len(&data);
        let ratio = result.bytes.len() as f64 / data.len().max(1) as f64;
        println!("gzip-9 {}", gz);
        println!(
            "AWARE/raw {:.3}  vs gzip-9 {:+.1}%",
            ratio,
            (result.bytes.len() as f64 / gz.max(1) as f64 - 1.0) * 100.0
        );
        return Ok(());
    }

    if let Some(out) = args.output {
        fs::write(&out, &result.bytes)?;
        println!("wrote {}", out.display());
    } else if result.bytes.len() >= data.len() {
        bail!(
            "packed {} >= raw {} — pass --output to force",
            result.bytes.len(),
            data.len()
        );
    }
    Ok(())
}
