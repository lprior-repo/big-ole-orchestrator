//! Blob retention index — tracks reference counts for garbage collection eligibility.
//!
//! Architecture: Data → Calc → Actions
//!
//! ## Data Layer
//!
//! - [`RetentionIndexError`]: Error taxonomy for retention operations
//! - [`RetentionEntry`]: Persisted reference count entry
//!
//! ## Calc Layer
//!
//! - [`encode_retention_key`], [`decode_retention_key`]: Content address encoding
//! - [`encode_retention_entry`], [`decode_retention_entry`]: Entry encoding
//!
//! ## Actions Layer
//!
//! - [`RetentionStore`] trait: Storage interface for retention index
//!
//! ## Invariants
//!
//! 1. Reference count is always non-negative (saturates at u64::MAX)
//! 2. A blob is GC-eligible iff its reference count is exactly zero
//! 3. Decrementing a zero ref count returns an error (not a no-op)

use crate::blob_store::ContentAddress;

pub const RETENTION_PARTITION: &str = "blob_retention";

#[non_exhaustive]
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum RetentionIndexError {
    #[error("content not found: {content_addr}")]
    ContentNotFound { content_addr: String },
    #[error("retention storage error: {reason}")]
    Storage { reason: String },
    #[error("retention codec error: {reason}")]
    Codec { reason: String },
    #[error("reference count already at zero for: {content_addr}")]
    RefCountZero { content_addr: String },
    #[error("invalid retention argument: {reason}")]
    InvalidArgument { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RetentionEntry {
    content_addr: ContentAddress,
    ref_count: u64,
}

impl RetentionEntry {
    #[must_use]
    pub const fn new(content_addr: ContentAddress, ref_count: u64) -> Self {
        Self {
            content_addr,
            ref_count,
        }
    }

    #[must_use]
    pub const fn content_addr(&self) -> &ContentAddress {
        &self.content_addr
    }

    #[must_use]
    pub const fn ref_count(&self) -> u64 {
        self.ref_count
    }

    #[must_use]
    pub const fn is_gc_eligible(&self) -> bool {
        self.ref_count == 0
    }

    #[must_use]
    pub fn increment_ref_count(&self) -> Self {
        Self {
            content_addr: self.content_addr.clone(),
            ref_count: self.ref_count.saturating_add(1),
        }
    }

    pub fn decrement_ref_count(&self) -> Result<Self, RetentionIndexError> {
        if self.ref_count == 0 {
            return Err(RetentionIndexError::RefCountZero {
                content_addr: self.content_addr.to_string(),
            });
        }
        Ok(Self {
            content_addr: self.content_addr.clone(),
            ref_count: self.ref_count.saturating_sub(1),
        })
    }
}

#[must_use]
pub fn encode_retention_key(addr: &ContentAddress) -> Vec<u8> {
    addr.as_str().as_bytes().to_vec()
}

pub fn decode_retention_key(bytes: &[u8]) -> Result<ContentAddress, RetentionIndexError> {
    let s = std::str::from_utf8(bytes).map_err(|e| RetentionIndexError::Codec {
        reason: format!("invalid UTF-8: {e}"),
    })?;
    ContentAddress::new(s).map_err(|e| RetentionIndexError::Codec {
        reason: e.to_string(),
    })
}

#[must_use]
pub fn encode_retention_entry(entry: &RetentionEntry) -> Vec<u8> {
    serde_json::to_vec(entry).expect("RetentionEntry should always be serializable")
}

pub fn decode_retention_entry(bytes: &[u8]) -> Result<RetentionEntry, RetentionIndexError> {
    serde_json::from_slice(bytes).map_err(|e| RetentionIndexError::Codec {
        reason: format!("JSON parse error: {e}"),
    })
}

pub trait RetentionStore {
    fn get(&self, addr: &ContentAddress) -> Result<Option<RetentionEntry>, RetentionIndexError>;

    fn increment(&self, addr: &ContentAddress) -> Result<u64, RetentionIndexError>;

    fn decrement(&self, addr: &ContentAddress) -> Result<u64, RetentionIndexError>;

    fn query_gc_eligible(&self) -> Result<Vec<ContentAddress>, RetentionIndexError>;

    fn is_gc_eligible(&self, addr: &ContentAddress) -> Result<bool, RetentionIndexError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_SHA256: &str = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";

    #[test]
    fn retention_entry_constructs_with_valid_fields() {
        let addr = ContentAddress::new(VALID_SHA256).unwrap();
        let entry = RetentionEntry::new(addr.clone(), 1);
        assert_eq!(entry.content_addr(), &addr);
        assert_eq!(entry.ref_count(), 1);
    }

    #[test]
    fn retention_entry_increment_ref_count() {
        let addr = ContentAddress::new(VALID_SHA256).unwrap();
        let entry = RetentionEntry::new(addr, 1);
        let incremented = entry.increment_ref_count();
        assert_eq!(incremented.ref_count(), 2);
    }

    #[test]
    fn retention_entry_increment_saturates_at_max() {
        let addr = ContentAddress::new(VALID_SHA256).unwrap();
        let entry = RetentionEntry::new(addr, u64::MAX);
        let incremented = entry.increment_ref_count();
        assert_eq!(incremented.ref_count(), u64::MAX);
    }

    #[test]
    fn retention_entry_decrement_ref_count_success() {
        let addr = ContentAddress::new(VALID_SHA256).unwrap();
        let entry = RetentionEntry::new(addr, 2);
        let decremented = entry.decrement_ref_count().unwrap();
        assert_eq!(decremented.ref_count(), 1);
    }

    #[test]
    fn retention_entry_decrement_ref_count_fails_at_zero() {
        let addr = ContentAddress::new(VALID_SHA256).unwrap();
        let entry = RetentionEntry::new(addr.clone(), 0);
        let result = entry.decrement_ref_count();
        assert!(matches!(
            result,
            Err(RetentionIndexError::RefCountZero { .. })
        ));
    }

    #[test]
    fn retention_entry_decrement_saturates_at_zero() {
        let addr = ContentAddress::new(VALID_SHA256).unwrap();
        let entry = RetentionEntry::new(addr, 1);
        let decremented = entry.decrement_ref_count().unwrap();
        assert_eq!(decremented.ref_count(), 0);
    }

    #[test]
    fn retention_entry_is_gc_eligible_when_zero() {
        let addr = ContentAddress::new(VALID_SHA256).unwrap();
        let entry = RetentionEntry::new(addr.clone(), 0);
        assert!(entry.is_gc_eligible());
    }

    #[test]
    fn retention_entry_is_not_gc_eligible_when_nonzero() {
        let addr = ContentAddress::new(VALID_SHA256).unwrap();
        let entry = RetentionEntry::new(addr, 1);
        assert!(!entry.is_gc_eligible());
    }

    #[test]
    fn encode_decode_retention_key_roundtrip() {
        let addr = ContentAddress::new(VALID_SHA256).unwrap();
        let encoded = encode_retention_key(&addr);
        let decoded = decode_retention_key(&encoded).unwrap();
        assert_eq!(decoded, addr);
    }

    #[test]
    fn encode_decode_retention_entry_roundtrip() {
        let addr = ContentAddress::new(VALID_SHA256).unwrap();
        let entry = RetentionEntry::new(addr.clone(), 5);
        let encoded = encode_retention_entry(&entry);
        let decoded = decode_retention_entry(&encoded).unwrap();
        assert_eq!(decoded.content_addr(), &addr);
        assert_eq!(decoded.ref_count(), 5);
    }

    #[test]
    fn retention_index_error_display() {
        let err = RetentionIndexError::ContentNotFound {
            content_addr: "abc".to_string(),
        };
        assert!(err.to_string().contains("content not found"));
        assert!(err.to_string().contains("abc"));
    }

    #[test]
    fn retention_index_error_ref_count_zero_display() {
        let err = RetentionIndexError::RefCountZero {
            content_addr: "abc".to_string(),
        };
        assert!(err.to_string().contains("reference count already at zero"));
        assert!(err.to_string().contains("abc"));
    }
}
