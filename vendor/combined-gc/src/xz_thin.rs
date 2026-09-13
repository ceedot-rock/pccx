//! Thin whole-file LZMA2 via system `xz` — same role as ZLB1.
//!
//! Magic XZ1 | u32 orig_len | xz -9 stream.
//! Used only as a file-level bake-off. Not inside 2M packs.

use std::io::Write;
use std::process::{Command, Stdio};

pub const XZ_MAGIC: &[u8; 4] = b"XZ1\0";

pub fn xz_available() -> bool {
    Command::new("xz")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn xz_encode(data: &[u8]) -> Option<Vec<u8>> {
    let mut child = Command::new("xz")
        .args(["-9", "-c", "--stdout"])
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

pub fn xz_decode(payload: &[u8]) -> Result<Vec<u8>, &'static str> {
    let mut child = Command::new("xz")
        .args(["-d", "-c", "--stdout"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "xz1: spawn")?;
    {
        let mut stdin = child.stdin.take().ok_or("xz1: stdin")?;
        stdin.write_all(payload).map_err(|_| "xz1: write")?;
    }
    let out = child.wait_with_output().map_err(|_| "xz1: wait")?;
    if !out.status.success() {
        return Err("xz1: xz -d");
    }
    Ok(out.stdout)
}

pub fn emit_thin_xz(data: &[u8]) -> Option<Vec<u8>> {
    let payload = xz_encode(data)?;
    let mut out = Vec::with_capacity(8 + payload.len());
    out.extend_from_slice(XZ_MAGIC);
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&payload);
    Some(out)
}

pub fn parse_thin_xz(buf: &[u8]) -> Result<Vec<u8>, &'static str> {
    if buf.len() < 8 || &buf[..4] != XZ_MAGIC {
        return Err("xz1: magic");
    }
    let orig = u32::from_le_bytes(buf[4..8].try_into().unwrap()) as usize;
    let out = xz_decode(&buf[8..])?;
    if orig != 0 && out.len() != orig {
        return Err("xz1: length");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thin_xz_roundtrip_if_present() {
        if !xz_available() {
            return;
        }
        let src = b"abcABC0123456789".repeat(200);
        let enc = emit_thin_xz(&src).expect("encode");
        assert!(enc.starts_with(XZ_MAGIC));
        assert!(enc.len() < src.len());
        let back = parse_thin_xz(&enc).expect("decode");
        assert_eq!(back, src);
    }

    #[test]
    fn obj1_xz_beats_zlib_lock() {
        if !xz_available() {
            return;
        }
        let path = "/home/workdir/artifacts/corpora/obj1";
        let src = match std::fs::read(path) {
            Ok(s) => s,
            Err(_) => return,
        };
        let xz = emit_thin_xz(&src).expect("xz");
        assert!(xz.len() < 10_322, "xz {} vs zlib lock 10322", xz.len());
        assert_eq!(parse_thin_xz(&xz).unwrap(), src);
    }
}
