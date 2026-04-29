//! Content address based on SHA-256 hash (64 lowercase hex characters).

use std::fmt;
use std::fmt::Write as _;

use super::error::BlobStoreError;

/// Content address based on SHA-256 hash (64 lowercase hex characters).
///
/// # Invariant
///
/// `content_addr` is always exactly 64 characters of lowercase hex (0-9, a-f),
/// representing a full SHA-256 digest.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize)]
#[expect(clippy::unsafe_derive_deserialize)]
#[derive(serde::Deserialize)]
pub struct ContentAddress(String);

impl ContentAddress {
    const LENGTH: usize = 64;

    /// Construct a `ContentAddress` from a hex string.
    ///
    /// # Errors
    ///
    /// Returns `BlobStoreError::InvalidArgument` if the string is not exactly
    /// 64 lowercase hex characters.
    pub fn new(addr: impl AsRef<str>) -> Result<Self, BlobStoreError> {
        let s = addr.as_ref();
        if s.len() != Self::LENGTH {
            return Err(BlobStoreError::InvalidArgument {
                reason: format!(
                    "content address must be {} chars, got {}",
                    Self::LENGTH,
                    s.len()
                ),
            });
        }
        if !s
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        {
            return Err(BlobStoreError::InvalidArgument {
                reason: "content address must be lowercase hex (0-9, a-f)".to_string(),
            });
        }
        Ok(Self(s.to_string()))
    }

    /// Construct a `ContentAddress` from raw SHA-256 bytes.
    #[must_use]
    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        Self(bytes.iter().fold(String::with_capacity(64), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        }))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn as_bytes(&self) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        for (i, chunk) in self.0.as_bytes().chunks(2).enumerate() {
            let high = hex_nibble(chunk[0]);
            let low = hex_nibble(chunk[1]);
            bytes[i] = (high << 4) | low;
        }
        bytes
    }
}

impl fmt::Display for ContentAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[must_use]
const fn hex_nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        b'A'..=b'F' => byte - b'A' + 10,
        _ => 0,
    }
}

/// Encode a `ContentAddress` as UTF-8 bytes for use as a storage key.
#[must_use]
pub fn encode_content_address(addr: &ContentAddress) -> Vec<u8> {
    addr.as_str().as_bytes().to_vec()
}

/// Decode UTF-8 bytes into a `ContentAddress`.
///
/// # Errors
///
/// Returns `BlobStoreError::CorruptPackIndex` if bytes are not valid UTF-8
/// or if the resulting string is not a valid content address.
pub fn decode_content_address(bytes: &[u8]) -> Result<ContentAddress, BlobStoreError> {
    let s = std::str::from_utf8(bytes).map_err(|e| BlobStoreError::CorruptPackIndex {
        reason: format!("invalid UTF-8: {e}"),
    })?;
    ContentAddress::new(s).map_err(|e| BlobStoreError::CorruptPackIndex {
        reason: e.to_string(),
    })
}

/// Validate that a string is a valid SHA-256 content address (64 lowercase hex chars).
///
/// # Errors
///
/// Returns `BlobStoreError::InvalidArgument` if the string is not a valid content address.
pub fn validate_content_address(addr: &str) -> Result<(), BlobStoreError> {
    ContentAddress::new(addr).map(|_| ())
}
