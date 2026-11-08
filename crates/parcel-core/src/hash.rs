//! Streaming SHA-256 helpers.
use crate::error::Result;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

pub const BUF: usize = 256 * 1024;

pub fn sha256_reader<R: Read>(mut r: R) -> Result<(String, u64)> {
    let mut h = Sha256::new();
    let mut buf = vec![0u8; BUF];
    let mut total = 0u64;
    loop {
        let n = r.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        total += n as u64;
    }
    Ok((hex::encode(h.finalize()), total))
}

pub fn sha256_file(path: &Path) -> Result<(String, u64)> {
    sha256_reader(std::fs::File::open(path)?)
}

pub fn sha256_bytes(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}
