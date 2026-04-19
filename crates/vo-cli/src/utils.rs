use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn file_hash_on_real_file() {
        let mut tmpfile = NamedTempFile::new().unwrap();
        tmpfile.write_all(b"hello world").unwrap();
        tmpfile.flush().unwrap();

        let result = file_hash(tmpfile.path());
        assert!(result.is_ok());
        let hash = result.unwrap();
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn file_hash_on_empty_file() {
        let tmpfile = NamedTempFile::new().unwrap();

        let result = file_hash(tmpfile.path());
        assert!(result.is_ok());
        let hash = result.unwrap();
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn file_hash_on_missing_file_returns_error() {
        let result = file_hash(Path::new("/nonexistent/path/to/file.txt"));
        assert!(result.is_err());
    }

    #[test]
    fn sha256_hex_known_input() {
        let hash = sha256_hex("");
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn sha256_hex_hello_world() {
        let hash = sha256_hex("hello world");
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn sha256_hex_different_inputs_different_hashes() {
        let hash1 = sha256_hex("a");
        let hash2 = sha256_hex("b");
        assert_ne!(hash1, hash2);
    }
}

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
