//! Core data types for content-addressed blob storage.

use std::fmt;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

use super::error::BlobStoreError;

// ---------------------------------------------------------------------------
// ContentAddress
// ---------------------------------------------------------------------------

/// Content address based on SHA-256 hash (64 lowercase hex characters).
///
/// # Invariant
///
/// `content_addr` is always exactly 64 characters of lowercase hex (0-9, a-f),
/// representing a full SHA-256 digest.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct ContentAddress(String);

impl<'de> Deserialize<'de> for ContentAddress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::new(&s).map_err(serde::de::Error::custom)
    }
}

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

// ---------------------------------------------------------------------------
// PackFileId
// ---------------------------------------------------------------------------

/// Unique identifier for a pack file.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PackFileId(String);

impl PackFileId {
    /// Construct a new `PackFileId`.
    ///
    /// # Errors
    ///
    /// Returns `BlobStoreError::InvalidArgument` if the string is empty.
    pub fn new(id: impl AsRef<str>) -> Result<Self, BlobStoreError> {
        let s = id.as_ref();
        if s.is_empty() {
            return Err(BlobStoreError::InvalidArgument {
                reason: "pack file ID cannot be empty".to_string(),
            });
        }
        Ok(Self(s.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PackFileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ---------------------------------------------------------------------------
// PackIndexEntry
// ---------------------------------------------------------------------------

/// Location of a blob within a pack file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackIndexEntry {
    content_addr: ContentAddress,
    pack_file_id: PackFileId,
    offset_bytes: u64,
    size_bytes: u64,
}

impl PackIndexEntry {
    /// Construct a new `PackIndexEntry`.
    #[must_use]
    pub const fn new(
        content_addr: ContentAddress,
        pack_file_id: PackFileId,
        offset_bytes: u64,
        size_bytes: u64,
    ) -> Self {
        Self {
            content_addr,
            pack_file_id,
            offset_bytes,
            size_bytes,
        }
    }

    #[must_use]
    pub const fn content_addr(&self) -> &ContentAddress {
        &self.content_addr
    }

    #[must_use]
    pub const fn pack_file_id(&self) -> &PackFileId {
        &self.pack_file_id
    }

    #[must_use]
    pub const fn offset_bytes(&self) -> u64 {
        self.offset_bytes
    }

    #[must_use]
    pub const fn size_bytes(&self) -> u64 {
        self.size_bytes
    }
}

// ---------------------------------------------------------------------------
// Hex helper (shared with encoding)
// ---------------------------------------------------------------------------

#[must_use]
pub(crate) const fn hex_nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        b'A'..=b'F' => byte - b'A' + 10,
        _ => 0,
    }
}
