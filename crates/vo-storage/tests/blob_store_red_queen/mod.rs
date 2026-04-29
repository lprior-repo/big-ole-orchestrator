//! Red Queen adversarial tests for blob publication protocol (ADR-040)
//!
//! Tests the blob publication invariants against:
//! - output_ref durability before blob (state machine transitions)
//! - dual representation consistency (Inline vs BlobRef)
//! - GC of referenced blobs (ref_count protection)
//! - concurrent publish race conditions
//!
//! Target: vo-storage/blob_store

#![allow(clippy::unwrap_used)]

mod blob_record_gc;
mod blob_ref;
mod blob_status;
mod concurrent_publish;
mod content_address;
mod encoding;
mod error_display;
mod helpers;
mod inlined_max_bytes;
mod output_policy;
mod output_ref;

pub use blob_record_gc::*;
pub use blob_ref::*;
pub use blob_status::*;
pub use concurrent_publish::*;
pub use content_address::*;
pub use encoding::*;
pub use error_display::*;
pub use helpers::*;
pub use inlined_max_bytes::*;
pub use output_policy::*;
pub use output_ref::*;