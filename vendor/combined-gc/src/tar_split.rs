//! Conservative reversible tar layout transform.
//!
//! Valid POSIX-style 512-byte headers are grouped together before their padded
//! payload blocks. The original member order, headers, payloads, and trailing
//! zero blocks are recovered exactly on decode. Inputs that do not fully parse
//! as this simple layout return `None` and must use another codec path.

pub const MAGIC: &[u8; 4] = b"TS01";
const BLOCK: usize = 512;

fn all_zero(block: &[u8]) -> bool { block.iter().all(|&b| b == 0) }

fn parse_size(header: &[u8]) -> Option<usize> {
    let field = &header[124..136];
    let mut value = 0usize;
    let mut seen = false;
    for &b in field {
        match b {
            b'0'..=b'7' => { seen = true; value = value.checked_mul(8)?.checked_add((b - b'0') as usize)?; }
            b' ' | 0 => { if seen { break; } }
            _ => return None,
        }
    }
    Some(value)
}

fn padded(n: usize) -> Option<usize> { n.checked_add(BLOCK - 1).map(|v| v / BLOCK * BLOCK) }

pub fn encode(input: &[u8]) -> Option<Vec<u8>> {
    if input.len() < BLOCK * 3 || input.len() % BLOCK != 0 { return None; }
    let mut at = 0usize;
    let mut count = 0u32;
    let mut headers = Vec::new();
    let mut payload = Vec::new();
    while at + BLOCK <= input.len() {
        let header = &input[at..at + BLOCK];
        if all_zero(header) { break; }
        // Empty name is not a normal member header and avoids treating arbitrary
        // block-aligned binary as a tar stream based on coincidental octal bytes.
        if header[..100].iter().all(|&b| b == 0) { return None; }
        let size = parse_size(header)?;
        let body = padded(size)?;
        let next = at.checked_add(BLOCK)?.checked_add(body)?;
        if next > input.len() || count == u32::MAX { return None; }
        headers.extend_from_slice(header);
        payload.extend_from_slice(&input[at + BLOCK..next]);
        count += 1;
        at = next;
    }
    if count == 0 || at + BLOCK > input.len() || !input[at..].chunks(BLOCK).all(all_zero) { return None; }
    let mut out = Vec::with_capacity(8 + headers.len() + payload.len() + input.len() - at);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&headers);
    out.extend_from_slice(&payload);
    out.extend_from_slice(&input[at..]);
    Some(out)
}

pub fn decode(input: &[u8], expected_len: usize) -> Result<Vec<u8>, &'static str> {
    if input.len() < 8 || &input[..4] != MAGIC { return Err("tar_split: magic"); }
    let count = u32::from_le_bytes(input[4..8].try_into().unwrap()) as usize;
    if count == 0 { return Err("tar_split: count"); }
    let header_len = count.checked_mul(BLOCK).ok_or("tar_split: size")?;
    let headers_end = 8usize.checked_add(header_len).ok_or("tar_split: size")?;
    if headers_end > input.len() { return Err("tar_split: truncated headers"); }
    let headers = &input[8..headers_end];
    let mut body_len = 0usize;
    for h in headers.chunks_exact(BLOCK) {
        if all_zero(h) || h[..100].iter().all(|&b| b == 0) { return Err("tar_split: header"); }
        body_len = body_len.checked_add(padded(parse_size(h).ok_or("tar_split: size")?).ok_or("tar_split: size")?).ok_or("tar_split: size")?;
    }
    let payload_end = headers_end.checked_add(body_len).ok_or("tar_split: size")?;
    if payload_end > input.len() { return Err("tar_split: truncated payload"); }
    let payload = &input[headers_end..payload_end];
    let tail = &input[payload_end..];
    if tail.len() < BLOCK || tail.len() % BLOCK != 0 || !tail.chunks(BLOCK).all(all_zero) { return Err("tar_split: tail"); }
    let mut out = Vec::with_capacity(expected_len);
    let mut body_at = 0usize;
    for h in headers.chunks_exact(BLOCK) {
        let body = padded(parse_size(h).ok_or("tar_split: size")?).ok_or("tar_split: size")?;
        out.extend_from_slice(h);
        out.extend_from_slice(&payload[body_at..body_at + body]);
        body_at += body;
    }
    out.extend_from_slice(tail);
    if out.len() != expected_len { return Err("tar_split: length"); }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(name: &[u8], size: usize) -> [u8; BLOCK] {
        let mut h = [0u8; BLOCK];
        h[..name.len()].copy_from_slice(name);
        let oct = format!("{:011o}\0", size);
        h[124..136].copy_from_slice(oct.as_bytes());
        h
    }

    #[test]
    fn groups_and_restores_members() {
        let mut tar = Vec::new();
        tar.extend_from_slice(&header(b"a.txt", 3)); tar.extend_from_slice(b"abc"); tar.resize(tar.len() + 509, 0);
        tar.extend_from_slice(&header(b"b.txt", 4)); tar.extend_from_slice(b"wxyz"); tar.resize(tar.len() + 508, 0);
        tar.resize(tar.len() + 1024, 0);
        let x = encode(&tar).unwrap();
        assert_eq!(decode(&x, tar.len()).unwrap(), tar);
    }

    #[test]
    fn rejects_non_tar() { assert!(encode(&[0u8; 1024]).is_none()); }
}
