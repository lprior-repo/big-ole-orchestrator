//! Blob retention index module.
//!
//! This module provides the retention index for tracking blob reference counts
//! and GC eligibility.

pub mod retention;

pub use retention::{
    decode_retention_entry, decode_retention_key, encode_retention_entry, encode_retention_key,
    RetentionEntry, RetentionIndexError, RetentionStore, RETENTION_PARTITION,
};
