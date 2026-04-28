//! Storage interface for content-addressed blob storage with SHA-256 dedup.

use super::content_address::ContentAddress;
use super::error::BlobStoreError;
use super::blob_record::BlobRecord;

/// Partition name for the blob store pack index.
pub const BLOB_STORE_PARTITION: &str = "blob_store";

/// Partition name for blob records.
pub const BLOB_RECORD_PARTITION: &str = "blob_records";

/// Storage interface for content-addressed blob storage with SHA-256 dedup.
///
/// # Streaming Semantics
///
/// Implementations must support streaming upload and download without buffering
/// the full blob in memory. The `store_streaming` method accepts an async
/// reader that yields chunks; the implementation computes SHA-256 incrementally
/// and writes to the pack file as chunks arrive.
///
/// # GC Semantics
///
/// Implementations must support lazy GC that only collects blobs when:
/// 1. Reference count drops to zero
/// 2. TTL has expired (if set)
///
/// GC must not run concurrently with itself (`GcCycleInProgress` error).
pub trait BlobStore {
    /// Store a blob from a byte slice, computing SHA-256 for content address.
    ///
    /// The blob is stored with `DurablyStored` status immediately.
    /// Use [`BlobStore::stage_blob`] to create a blob in `Pending` status
    /// for the full ADR-040 publication protocol.
    ///
    /// If the content already exists (dedup), returns `BlobStoreError::DuplicateContent`.
    ///
    /// # Errors
    ///
    /// Returns `BlobStoreError::DuplicateContent` if content already exists.
    /// Returns `BlobStoreError::Storage` if the underlying storage fails.
    fn store(&self, data: &[u8]) -> Result<ContentAddress, BlobStoreError>;

    /// Stage a blob for later publication, creating it with `Pending` status.
    ///
    /// The blob data is written durably, but the metadata is created with
    /// `Pending` status. The caller MUST call [`BlobStore::mark_durable`] before
    /// publishing an `output_ref` referencing this blob (per ADR-040 §2).
    ///
    /// Use this for the full ADR-040 publication protocol:
    /// 1. `stage_blob` - creates blob as `Pending`
    /// 2. `mark_durable` - transitions to `DurablyStored`
    /// 3. `publish` - transitions to `Published`
    ///
    /// # Errors
    ///
    /// Returns `BlobStoreError::DuplicateContent` if content already exists.
    /// Returns `BlobStoreError::Storage` if the underlying storage fails.
    fn stage_blob(&self, data: &[u8]) -> Result<ContentAddress, BlobStoreError>;

    /// Store a blob from a streaming source, computing SHA-256 incrementally.
    ///
    /// The `reader` yields chunks of the blob data. The implementation must:
    /// 1. Compute SHA-256 as chunks arrive (without buffering full blob)
    /// 2. Write chunks to the current pack file
    /// 3. Return the final content address
    ///
    /// # Errors
    ///
    /// Returns `BlobStoreError::DuplicateContent` if content already exists.
    /// Returns `BlobStoreError::ChecksumMismatch` if data read does not match declared address.
    /// Returns `BlobStoreError::Storage` if the underlying storage fails.
    fn store_streaming<R>(&self, reader: R) -> Result<ContentAddress, BlobStoreError>
    where
        R: tokio::io::AsyncRead + Send + Unpin + 'static;

    /// Retrieve a blob by content address into a byte vector.
    ///
    /// # Errors
    ///
    /// Returns `BlobStoreError::ContentNotFound` if no blob exists for the address.
    /// Returns `BlobStoreError::Storage` if the underlying storage fails.
    fn retrieve(&self, addr: &ContentAddress) -> Result<Vec<u8>, BlobStoreError>;

    /// Retrieve a blob by content address via streaming.
    ///
    /// The `writer` receives chunks of the blob data. The implementation must:
    /// 1. Look up the pack file and offset from the pack index
    /// 2. Stream the blob data from the pack file to the writer
    ///
    /// # Errors
    ///
    /// Returns `BlobStoreError::ContentNotFound` if no blob exists for the address.
    /// Returns `BlobStoreError::PackFileNotFound` if the pack file is missing.
    /// Returns `BlobStoreError::ChecksumMismatch` if data read does not match declared address.
    /// Returns `BlobStoreError::Storage` if the underlying storage fails.
    fn retrieve_streaming<W>(&self, addr: &ContentAddress, writer: W) -> Result<(), BlobStoreError>
    where
        W: tokio::io::AsyncWrite + Send + Unpin + 'static;

    /// Check if a blob exists for the given content address.
    ///
    /// # Errors
    ///
    /// Returns `BlobStoreError::Storage` if the underlying storage lookup fails.
    fn contains(&self, addr: &ContentAddress) -> Result<bool, BlobStoreError>;

    /// Increment the reference count for a blob.
    ///
    /// # Errors
    ///
    /// Returns `BlobStoreError::ContentNotFound` if no blob exists.
    /// Returns `BlobStoreError::Storage` if the underlying storage fails.
    fn increment_ref_count(&self, addr: &ContentAddress) -> Result<u64, BlobStoreError>;

    /// Decrement the reference count for a blob.
    ///
    /// When reference count reaches zero, the blob becomes eligible for GC
    /// (if TTL has expired or no TTL is set).
    ///
    /// # Errors
    ///
    /// Returns `BlobStoreError::ContentNotFound` if no blob exists.
    /// Returns `BlobStoreError::Storage` if the underlying storage fails.
    fn decrement_ref_count(&self, addr: &ContentAddress) -> Result<u64, BlobStoreError>;

    /// Get blob metadata by content address.
    ///
    /// # Errors
    ///
    /// Returns `BlobStoreError::ContentNotFound` if no blob exists.
    /// Returns `BlobStoreError::Storage` if the underlying storage fails.
    fn get_metadata(&self, addr: &ContentAddress) -> Result<BlobRecord, BlobStoreError>;

    /// List all blobs eligible for GC (reference count = 0 and expired).
    ///
    /// Returns content addresses of blobs that are safe to collect.
    ///
    /// # Errors
    ///
    /// Returns `BlobStoreError::Storage` if the underlying storage lookup fails.
    fn list_gc_candidates(&self, now_ms: u64) -> Result<Vec<ContentAddress>, BlobStoreError>;

    /// Run lazy GC to collect unreferenced and expired blobs.
    ///
    /// Returns the count of blobs collected.
    ///
    /// # Errors
    ///
    /// Returns `BlobStoreError::GcCycleInProgress` if GC is already running.
    /// Returns `BlobStoreError::Storage` if the underlying storage fails.
    fn run_gc(&self, now_ms: u64) -> Result<u64, BlobStoreError>;

    /// Mark a staged blob as durably stored.
    ///
    /// Transitions blob status from `Pending` to `DurablyStored`.
    /// After this call, the blob is guaranteed durable and the Engine
    /// may publish an `output_ref` referencing it (per ADR-040 §2).
    ///
    /// # Errors
    ///
    /// Returns `BlobStoreError::ContentNotFound` if no blob exists.
    /// Returns `BlobStoreError::InvalidPublicationStatus` if blob is not in `Pending` status.
    /// Returns `BlobStoreError::Storage` if the underlying storage fails.
    fn mark_durable(&self, addr: &ContentAddress) -> Result<(), BlobStoreError>;

    /// Publish a durably stored blob, making it referenceable by `output_ref`.
    ///
    /// Transitions blob status from `DurablyStored` to `Published`.
    /// After this call, the blob has crossed the publication boundary
    /// and is part of the exact-once replay contract (ADR-040 §4).
    ///
    /// # Errors
    ///
    /// Returns `BlobStoreError::ContentNotFound` if no blob exists.
    /// Returns `BlobStoreError::InvalidPublicationStatus` if blob is not in `DurablyStored` status.
    /// Returns `BlobStoreError::Storage` if the underlying storage fails.
    fn publish(&self, addr: &ContentAddress) -> Result<(), BlobStoreError>;

    /// Mark a blob as failed.
    ///
    /// Transitions blob status from `Pending` to `Failed`.
    /// Used when blob persistence fails and cannot be retried.
    ///
    /// # Errors
    ///
    /// Returns `BlobStoreError::ContentNotFound` if no blob exists.
    /// Returns `BlobStoreError::InvalidPublicationStatus` if blob is not in `Pending` status.
    /// Returns `BlobStoreError::Storage` if the underlying storage fails.
    fn mark_failed(&self, addr: &ContentAddress) -> Result<(), BlobStoreError>;
}
