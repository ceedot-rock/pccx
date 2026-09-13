//! Thin whole-file bzip2 — same role as XZ1 / ZLB1.
//! Magic BZ1 | u32 orig_len | bzip2 -9 stream.

use std::io::Write;
use std::process::{Command, Stdio};

pub const BZ_MAGIC: &[u8; 4] = b"BZ1\0";

pub fn bz_available() -> bool {
    Command::new("bzip2")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn bz_encode(data: &[u8]) -> Option<Vec<u8>> {
    let mut child = Command::new("bzip2")
        .args(["-9", "-c"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    {
        let mut stdin = child.stdin.take()?;
        stdin.write_all(data).ok()?;
    }
    let out = child.wait_with_output().ok()?;
    if !out.status.success() || out.stdout.is_empty() {
        return None;
    }
    Some(out.stdout)
}

pub fn bz_decode(payload: &[u8]) -> Result<Vec<u8>, &'static str> {
    let mut child = Command::new("bzip2")
        .args(["-d", "-c"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "bz1: spawn")?;
    {
        let mut stdin = child.stdin.take().ok_or("bz1: stdin")?;
        stdin.write_all(payload).map_err(|_| "bz1: write")?;
    }
    let out = child.wait_with_output().map_err(|_| "bz1: wait")?;
    if !out.status.success() {
        return Err("bz1: bzip2 -d");
    }
    Ok(out.stdout)
}

pub fn emit_thin_bz(data: &[u8]) -> Option<Vec<u8>> {
    let payload = bz_encode(data)?;
    let mut out = Vec::with_capacity(8 + payload.len());
    out.extend_from_slice(BZ_MAGIC);
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&payload);
    Some(out)
}

pub fn parse_thin_bz(buf: &[u8]) -> Result<Vec<u8>, &'static str> {
    if buf.len() < 8 || &buf[..4] != BZ_MAGIC {
        return Err("bz1: magic");
    }
    let orig = u32::from_le_bytes(buf[4..8].try_into().unwrap()) as usize;
    let out = bz_decode(&buf[8..])?;
    if orig != 0 && out.len() != orig {
        return Err("bz1: length");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thin_bz_roundtrip() {
        if !bz_available() {
            return;
        }
        let src = b"iso_ir 100 repeating header\n".repeat(200);
        let enc = emit_thin_bz(&src).expect("encode");
        assert!(enc.starts_with(BZ_MAGIC));
        assert_eq!(parse_thin_bz(&enc).unwrap(), src.as_slice());
    }

    #[test]
    fn mr_bz_beats_aware_lock() {
        if !bz_available() {
            return;
        }
        let src = match std::fs::read("/home/workdir/artifacts/corpora/mr") {
            Ok(s) => s,
            Err(_) => return,
        };
        let enc = emit_thin_bz(&src).expect("bz");
        eprintln!("mr BZ1 {} vs AWARE 2481623 vs bzip2-9 2441280", enc.len());
        assert!(enc.len() < 2_481_623, "BZ1 {} not under AWARE", enc.len());
        assert_eq!(parse_thin_bz(&enc).unwrap(), src);
    }
}
