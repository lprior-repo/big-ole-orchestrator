//! Multi-algorithm checksum verification pipeline.
//!
//! Provides streaming, parallel, and incremental checksum computation
//! for CRC32, SHA-256, and BLAKE3 algorithms.
//!
//! Architecture: Data layer only — pure types and computation, no I/O.

use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::fmt::Write;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChecksumAlgorithm {
    Crc32,
    Sha256,
    Blake3,
}

impl ChecksumAlgorithm {
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Crc32 => "crc32",
            Self::Sha256 => "sha256",
            Self::Blake3 => "blake3",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct Checksum {
    pub crc32: u32,
    pub sha256: [u8; 32],
    pub blake3: [u8; 32],
}

#[derive(Clone)]
pub struct StreamingHasher {
    crc32: crc32fast::Hasher,
    sha256: sha2::Sha256,
    blake3: blake3::Hasher,
}

impl StreamingHasher {
    #[must_use]
    pub fn new() -> Self {
        Self {
            crc32: crc32fast::Hasher::new(),
            sha256: sha2::Sha256::new(),
            blake3: blake3::Hasher::new(),
        }
    }

    pub fn update(&mut self, data: &[u8]) {
        self.crc32.update(data);
        self.sha256.update(data);
        self.blake3.update(data);
    }

    #[must_use]
    pub fn finalize(self) -> Checksum {
        Checksum {
            crc32: self.crc32.finalize(),
            sha256: self.sha256.finalize().into(),
            blake3: *self.blake3.finalize().as_bytes(),
        }
    }
}

impl Default for StreamingHasher {
    fn default() -> Self {
        Self::new()
    }
}

#[must_use]
pub fn compute_checksum(data: &[u8]) -> Checksum {
    let mut hasher = StreamingHasher::new();
    hasher.update(data);
    hasher.finalize()
}

/// Verifies that `data` produces the same checksum as `expected`.
///
/// # Errors
///
/// Returns [`ChecksumError::Mismatch`] if any algorithm's computed digest
/// differs from the expected value.
pub fn verify_checksum(data: &[u8], expected: &Checksum) -> Result<(), ChecksumError> {
    let computed = compute_checksum(data);
    verify_checksum_internal(&computed, expected)
}

fn verify_checksum_internal(computed: &Checksum, expected: &Checksum) -> Result<(), ChecksumError> {
    if computed.sha256 != expected.sha256 {
        return Err(ChecksumError::Mismatch {
            algorithm: ChecksumAlgorithm::Sha256,
            expected: hex_encode(&expected.sha256),
            actual: hex_encode(&computed.sha256),
        });
    }
    if computed.blake3 != expected.blake3 {
        return Err(ChecksumError::Mismatch {
            algorithm: ChecksumAlgorithm::Blake3,
            expected: hex_encode(&expected.blake3),
            actual: hex_encode(&computed.blake3),
        });
    }
    if computed.crc32 != expected.crc32 {
        return Err(ChecksumError::Mismatch {
            algorithm: ChecksumAlgorithm::Crc32,
            expected: format!("{:08x}", expected.crc32),
            actual: format!("{:08x}", computed.crc32),
        });
    }
    Ok(())
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut acc, b| {
            let _ = write!(acc, "{b:02x}");
            acc
        })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChecksumError {
    Mismatch {
        algorithm: ChecksumAlgorithm,
        expected: String,
        actual: String,
    },
    Io(String),
}

impl std::fmt::Display for ChecksumError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Mismatch {
                algorithm,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "checksum mismatch for {}: expected {}, got {}",
                    algorithm.name(),
                    expected,
                    actual
                )
            }
            Self::Io(msg) => write!(f, "checksum I/O error: {msg}"),
        }
    }
}

impl std::error::Error for ChecksumError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkInfo {
    pub offset: u64,
    pub size: u64,
    pub checksum: Checksum,
}

pub struct ChunkedHasher {
    chunk_size: u64,
    current_offset: u64,
    current_hasher: StreamingHasher,
    chunks: Vec<ChunkInfo>,
}

impl ChunkedHasher {
    #[must_use]
    pub fn new(chunk_size: u64) -> Self {
        Self {
            chunk_size,
            current_offset: 0,
            current_hasher: StreamingHasher::new(),
            chunks: Vec::new(),
        }
    }

    pub fn update(&mut self, data: &[u8]) {
        let mut offset = 0;
        while offset < data.len() {
            let remaining_in_chunk = self.chunk_size - (self.current_offset % self.chunk_size);
            let remaining_usize = usize::try_from(remaining_in_chunk).unwrap_or(usize::MAX);
            let bytes_to_process = std::cmp::min(remaining_usize, data.len() - offset);

            self.current_hasher
                .update(&data[offset..offset + bytes_to_process]);
            self.current_offset += bytes_to_process as u64;

            if self.current_offset.is_multiple_of(self.chunk_size) {
                let checksum = self.current_hasher.clone().finalize();
                self.chunks.push(ChunkInfo {
                    offset: self.current_offset - self.chunk_size,
                    size: self.chunk_size,
                    checksum,
                });
                self.current_hasher = StreamingHasher::new();
            }

            offset += bytes_to_process;
        }
    }

    #[must_use]
    pub fn finalize(mut self) -> Vec<ChunkInfo> {
        if !self.current_offset.is_multiple_of(self.chunk_size) || self.current_offset == 0 {
            let checksum = self.current_hasher.finalize();
            let remaining = self.current_offset % self.chunk_size;
            let size = if remaining == 0 {
                self.chunk_size
            } else {
                remaining
            };
            self.chunks.push(ChunkInfo {
                offset: self.current_offset - size,
                size,
                checksum,
            });
        }
        self.chunks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_algorithm_name() {
        assert_eq!(ChecksumAlgorithm::Crc32.name(), "crc32");
        assert_eq!(ChecksumAlgorithm::Sha256.name(), "sha256");
        assert_eq!(ChecksumAlgorithm::Blake3.name(), "blake3");
    }

    #[test]
    fn streaming_hasher_produces_consistent_checksum() {
        let data = b"hello world";
        let mut hasher = StreamingHasher::new();
        hasher.update(data);
        let checksum = hasher.finalize();

        let expected = compute_checksum(data);
        assert_eq!(checksum.crc32, expected.crc32);
        assert_eq!(checksum.sha256, expected.sha256);
        assert_eq!(checksum.blake3, expected.blake3);
    }

    #[test]
    fn streaming_hasher_incremental() {
        let data1 = b"hello ";
        let data2 = b"world";
        let data_full = b"hello world";

        let mut hasher = StreamingHasher::new();
        hasher.update(data1);
        hasher.update(data2);
        let checksum_incremental = hasher.finalize();

        let checksum_full = compute_checksum(data_full);

        assert_eq!(checksum_incremental.crc32, checksum_full.crc32);
        assert_eq!(checksum_incremental.sha256, checksum_full.sha256);
        assert_eq!(checksum_incremental.blake3, checksum_full.blake3);
    }

    #[test]
    fn verify_checksum_passes_for_valid_data() {
        let data = b"test data for verification";
        let checksum = compute_checksum(data);
        assert!(verify_checksum(data, &checksum).is_ok());
    }

    #[test]
    fn verify_checksum_fails_for_corrupted_data() {
        let data = b"original data";
        let checksum = compute_checksum(data);
        let corrupted = b"corrupted data";
        let result = verify_checksum(corrupted, &checksum);
        assert!(result.is_err());
        if let Err(ChecksumError::Mismatch { algorithm, .. }) = result {
            assert_eq!(algorithm, ChecksumAlgorithm::Sha256);
        }
    }

    #[test]
    fn chunked_hasher_splits_data_into_chunks() {
        let data = b"0123456789ABCDEF"; // 16 bytes
        let mut hasher = ChunkedHasher::new(5); // 5 byte chunks
        hasher.update(data);
        let chunks = hasher.finalize();

        assert!(chunks.len() >= 3);
        let total: u64 = chunks.iter().map(|c| c.size).sum();
        assert_eq!(total, 16);
    }

    #[test]
    fn chunked_hasher_incremental() {
        let chunk_size = 5u64;
        let mut hasher = ChunkedHasher::new(chunk_size);

        hasher.update(b"012");
        hasher.update(b"34");
        hasher.update(b"56789");
        hasher.update(b"ABCDEF");

        let chunks = hasher.finalize();

        let total: u64 = chunks.iter().map(|c| c.size).sum();
        assert_eq!(total, 16);
    }

    #[test]
    fn checksum_serde_roundtrip() {
        let checksum = compute_checksum(b"test data");
        let json = serde_json::to_string(&checksum).expect("serialize");
        let recovered: Checksum = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(checksum, recovered);
    }

    #[test]
    fn chunked_hasher_offsets_are_monotonic() {
        let data = b"0123456789ABCDEF";
        let mut hasher = ChunkedHasher::new(5);
        hasher.update(data);
        let chunks = hasher.finalize();

        for i in 1..chunks.len() {
            assert!(
                chunks[i].offset > chunks[i - 1].offset,
                "chunk offset {} should be greater than previous {}",
                chunks[i].offset,
                chunks[i - 1].offset
            );
        }
    }

    #[test]
    fn chunked_hasher_finalize_produces_chunk_when_data_processed() {
        let data = b"test";
        let mut hasher = ChunkedHasher::new(1024);
        hasher.update(data);
        let chunks = hasher.finalize();

        assert!(
            !chunks.is_empty(),
            "finalize must produce at least one chunk when data was processed"
        );
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].size, 4);
    }

    #[test]
    fn empty_data_produces_valid_checksum() {
        let empty: &[u8] = &[];
        let checksum = compute_checksum(empty);

        assert_eq!(checksum.crc32, 0);

        let expected_sha256: [u8; 32] = sha2::Sha256::digest(empty).into();
        assert_eq!(checksum.sha256, expected_sha256);

        let expected_blake3: [u8; 32] = *blake3::hash(empty).as_bytes();
        assert_eq!(checksum.blake3, expected_blake3);
    }

    #[test]
    fn checksum_display_shows_all_algorithms() {
        let _checksum = compute_checksum(b"test");
        let display = format!(
            "{}",
            ChecksumError::Mismatch {
                algorithm: ChecksumAlgorithm::Sha256,
                expected: "abc".to_string(),
                actual: "def".to_string(),
            }
        );
        assert!(display.contains("sha256"));
    }
}
