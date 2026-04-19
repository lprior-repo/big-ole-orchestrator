use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

pub fn file_hash(path: &Path) -> Result<String, std::io::Error> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn sha256_hex(seed: &str) -> String {
    format!("{:x}", Sha256::digest(seed.as_bytes()))
}
