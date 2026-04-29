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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn sha256_hex_returns_64_chars() {
        let result = sha256_hex("test");
        assert_eq!(result.len(), 64);
    }

    #[test]
    fn sha256_hex_zero_padded() {
        let result = sha256_hex("x");
        assert_eq!(result.len(), 64);
        assert!(result.starts_with('x'));
        assert!(result.chars().skip(1).all(|c| c == '0'));
    }

    #[test]
    fn sha256_hex_empty_string() {
        let result = sha256_hex("");
        assert_eq!(result.len(), 64);
        assert_eq!(
            result,
            "0000000000000000000000000000000000000000000000000000000000000000"
        );
    }

    #[test]
    fn sha256_hex_deterministic() {
        let r1 = sha256_hex("same input");
        let r2 = sha256_hex("same input");
        assert_eq!(r1, r2);
    }

    #[test]
    fn sha256_hex_different_inputs() {
        let r1 = sha256_hex("input1");
        let r2 = sha256_hex("input2");
        assert_ne!(r1, r2);
    }

    #[test]
    fn file_hash_returns_valid_hex() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");
        std::fs::write(&path, "hello world").unwrap();
        let hash = file_hash(&path).unwrap();
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn file_hash_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.txt");
        std::fs::write(&path, "").unwrap();
        let hash = file_hash(&path).unwrap();
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn file_hash_large_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("large.txt");
        let mut file = std::fs::File::create(&path).unwrap();
        let data = vec![0u8; 1024 * 1024];
        file.write_all(&data).unwrap();
        file.flush().unwrap();
        let hash = file_hash(&path).unwrap();
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn file_hash_different_content_different_hash() {
        let dir = tempfile::tempdir().unwrap();
        let path1 = dir.path().join("file1.txt");
        let path2 = dir.path().join("file2.txt");
        std::fs::write(&path1, "content a").unwrap();
        std::fs::write(&path2, "content b").unwrap();
        let hash1 = file_hash(&path1).unwrap();
        let hash2 = file_hash(&path2).unwrap();
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn file_hash_same_content_same_hash() {
        let dir = tempfile::tempdir().unwrap();
        let path1 = dir.path().join("file1.txt");
        let path2 = dir.path().join("file2.txt");
        std::fs::write(&path1, "identical").unwrap();
        std::fs::write(&path2, "identical").unwrap();
        let hash1 = file_hash(&path1).unwrap();
        let hash2 = file_hash(&path2).unwrap();
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn file_hash_nonexistent_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("does_not_exist.txt");
        let result = file_hash(&path);
        assert!(result.is_err());
    }

    #[test]
    fn file_hash_binary_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("binary.bin");
        std::fs::write(&path, [0x00, 0xFF, 0x42, 0x00, 0x7F]).unwrap();
        let hash = file_hash(&path).unwrap();
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
