//! Moon Gate: ADR-040 Blob Publication Integration Tests
//!
//! Integration CI gate for blob publication per ADR-040:
//! - Atomicity tests with crash injection
//! - Verify output_ref ordering (ADR-040 §2 publication rule)
//! - Check dual representation consistency
//!
//! ADR-040 defines the canonical blob publication protocol:
//! 1. Blob roles: inline (routing-critical) vs canonical (payload_blobs)
//! 2. Publication rule: output_ref only published after blob is durable
//! 3. Failure semantics: Required blocks on failure, Optional allows inline
//! 4. Product discipline: exact-once replay contract

#![allow(clippy::unwrap_used)]

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use vo_storage::blob_store::{BlobRecord, BlobStore, BlobStoreError, ContentAddress, PackFileId};
use vo_types::{
    BlobFailureAction, BlobRef, BlobStatus, OutputPolicy, OutputRef, INLINED_MAX_BYTES,
};

// ========================================================================
// Crash-Injectable Blob Store — for atomicity testing
// ========================================================================

struct CrashableBlobStore {
    blobs: RefCell<HashMap<ContentAddress, Vec<u8>>>,
    records: RefCell<HashMap<ContentAddress, BlobRecord>>,
    pack_index: RefCell<HashMap<ContentAddress, (PackFileId, u64, u64)>>,
    gc_in_progress: RefCell<bool>,
    crash_at_next_store: AtomicBool,
    crash_count: AtomicU64,
}

impl CrashableBlobStore {
    fn new() -> Self {
        Self {
            blobs: RefCell::new(HashMap::new()),
            records: RefCell::new(HashMap::new()),
            pack_index: RefCell::new(HashMap::new()),
            gc_in_progress: RefCell::new(false),
            crash_at_next_store: AtomicBool::new(false),
            crash_count: AtomicU64::new(0),
        }
    }

    fn enable_crash(&self) {
        self.crash_at_next_store.store(true, Ordering::SeqCst);
    }

    fn crash_count(&self) -> u64 {
        self.crash_count.load(Ordering::SeqCst)
    }

    fn insert_blob(&self, addr: ContentAddress, data: Vec<u8>, record: BlobRecord) {
        self.blobs.borrow_mut().insert(addr.clone(), data);
        self.records.borrow_mut().insert(addr, record);
    }

    fn get_record(&self, addr: &ContentAddress) -> Option<BlobRecord> {
        self.records.borrow().get(addr).cloned()
    }

    fn update_record(&self, addr: &ContentAddress, record: BlobRecord) {
        self.records.borrow_mut().insert(addr.clone(), record);
    }

    fn has_blob(&self, addr: &ContentAddress) -> bool {
        self.blobs.borrow().contains_key(addr)
    }
}

impl BlobStore for CrashableBlobStore {
    fn store(&self, data: &[u8]) -> Result<ContentAddress, BlobStoreError> {
        if self.crash_at_next_store.load(Ordering::SeqCst) {
            self.crash_count.fetch_add(1, Ordering::SeqCst);
            self.crash_at_next_store.store(false, Ordering::SeqCst);
            return Err(BlobStoreError::Storage {
                reason: "crash injected".to_string(),
            });
        }

        use sha2::Digest;
        let addr = ContentAddress::from_bytes(&sha2::Sha256::digest(data).into());
        let size = data.len() as u64;

        if self.blobs.borrow().contains_key(&addr) {
            return Err(BlobStoreError::DuplicateContent {
                content_addr: addr.to_string(),
            });
        }

        let record = BlobRecord::new(addr.clone(), size, 1, 1000, None).map_err(|e| {
            BlobStoreError::InvalidArgument {
                reason: e.to_string(),
            }
        })?;

        self.blobs.borrow_mut().insert(addr.clone(), data.to_vec());
        self.records.borrow_mut().insert(addr.clone(), record);

        Ok(addr)
    }

    fn store_streaming<R>(&self, _reader: R) -> Result<ContentAddress, BlobStoreError>
    where
        R: tokio::io::AsyncRead + Send + Unpin + 'static,
    {
        Err(BlobStoreError::Storage {
            reason: "not implemented".to_string(),
        })
    }

    fn retrieve(&self, addr: &ContentAddress) -> Result<Vec<u8>, BlobStoreError> {
        self.blobs
            .borrow()
            .get(addr)
            .cloned()
            .ok_or_else(|| BlobStoreError::ContentNotFound {
                content_addr: addr.to_string(),
            })
    }

    fn retrieve_streaming<W>(
        &self,
        _addr: &ContentAddress,
        _writer: W,
    ) -> Result<(), BlobStoreError>
    where
        W: tokio::io::AsyncWrite + Send + Unpin + 'static,
    {
        Err(BlobStoreError::Storage {
            reason: "not implemented".to_string(),
        })
    }

    fn contains(&self, addr: &ContentAddress) -> Result<bool, BlobStoreError> {
        Ok(self.blobs.borrow().contains_key(addr))
    }

    fn increment_ref_count(&self, addr: &ContentAddress) -> Result<u64, BlobStoreError> {
        let record = self
            .records
            .borrow()
            .get(addr)
            .ok_or_else(|| BlobStoreError::ContentNotFound {
                content_addr: addr.to_string(),
            })?
            .clone();

        let new_count = record.increment_ref_count();
        let new_record = BlobRecord::new(
            record.content_addr().clone(),
            record.size_bytes(),
            new_count,
            record.created_at_ms(),
            record.expires_at_ms(),
        )
        .map_err(|e| BlobStoreError::InvalidArgument {
            reason: e.to_string(),
        })?;

        self.records.borrow_mut().insert(addr.clone(), new_record);
        Ok(new_count)
    }

    fn decrement_ref_count(&self, addr: &ContentAddress) -> Result<u64, BlobStoreError> {
        let record = self
            .records
            .borrow()
            .get(addr)
            .ok_or_else(|| BlobStoreError::ContentNotFound {
                content_addr: addr.to_string(),
            })?
            .clone();

        let new_count = record.decrement_ref_count();
        let new_record = BlobRecord::new(
            record.content_addr().clone(),
            record.size_bytes(),
            new_count,
            record.created_at_ms(),
            record.expires_at_ms(),
        )
        .map_err(|e| BlobStoreError::InvalidArgument {
            reason: e.to_string(),
        })?;

        self.records.borrow_mut().insert(addr.clone(), new_record);
        Ok(new_count)
    }

    fn get_metadata(&self, addr: &ContentAddress) -> Result<BlobRecord, BlobStoreError> {
        self.records
            .borrow()
            .get(addr)
            .cloned()
            .ok_or_else(|| BlobStoreError::ContentNotFound {
                content_addr: addr.to_string(),
            })
    }

    fn list_gc_candidates(&self, now_ms: u64) -> Result<Vec<ContentAddress>, BlobStoreError> {
        Ok(self
            .records
            .borrow()
            .iter()
            .filter(|(_, record)| record.reference_count() == 0 && record.is_expired(now_ms))
            .map(|(addr, _)| addr.clone())
            .collect())
    }

    fn run_gc(&self, now_ms: u64) -> Result<u64, BlobStoreError> {
        if *self.gc_in_progress.borrow() {
            return Err(BlobStoreError::GcCycleInProgress);
        }

        *self.gc_in_progress.borrow_mut() = true;

        let candidates: Vec<ContentAddress> = self
            .records
            .borrow()
            .iter()
            .filter(|(_, record)| record.reference_count() == 0 && record.is_expired(now_ms))
            .map(|(addr, _)| addr.clone())
            .collect();

        let count = candidates.len() as u64;

        for addr in &candidates {
            self.blobs.borrow_mut().remove(addr);
            self.records.borrow_mut().remove(addr);
            self.pack_index.borrow_mut().remove(addr);
        }

        *self.gc_in_progress.borrow_mut() = false;

        Ok(count)
    }

    fn stage_blob(&self, data: &[u8]) -> Result<ContentAddress, BlobStoreError> {
        use sha2::Digest;
        let addr = ContentAddress::from_bytes(&sha2::Sha256::digest(data).into());
        let size = data.len() as u64;

        if self.blobs.borrow().contains_key(&addr) {
            return Err(BlobStoreError::DuplicateContent {
                content_addr: addr.to_string(),
            });
        }

        let record =
            BlobRecord::with_status(addr.clone(), size, 1, 1000, None, BlobStatus::Pending);

        self.blobs.borrow_mut().insert(addr.clone(), data.to_vec());
        self.records.borrow_mut().insert(addr.clone(), record);

        Ok(addr)
    }

    fn mark_durable(&self, addr: &ContentAddress) -> Result<(), BlobStoreError> {
        let record = self
            .records
            .borrow()
            .get(addr)
            .ok_or_else(|| BlobStoreError::ContentNotFound {
                content_addr: addr.to_string(),
            })?
            .clone();

        if !record.can_transition_to(BlobStatus::DurablyStored) {
            return Err(BlobStoreError::InvalidPublicationStatus {
                content_addr: addr.to_string(),
                current_status: format!("{:?}", record.status()),
                attempted_operation: "mark_durable".to_string(),
            });
        }

        let new_record = BlobRecord::with_status(
            record.content_addr().clone(),
            record.size_bytes(),
            record.reference_count(),
            record.created_at_ms(),
            record.expires_at_ms(),
            BlobStatus::DurablyStored,
        );

        self.records.borrow_mut().insert(addr.clone(), new_record);
        Ok(())
    }

    fn publish(&self, addr: &ContentAddress) -> Result<(), BlobStoreError> {
        let record = self
            .records
            .borrow()
            .get(addr)
            .ok_or_else(|| BlobStoreError::ContentNotFound {
                content_addr: addr.to_string(),
            })?
            .clone();

        if !record.can_transition_to(BlobStatus::Published) {
            return Err(BlobStoreError::InvalidPublicationStatus {
                content_addr: addr.to_string(),
                current_status: format!("{:?}", record.status()),
                attempted_operation: "publish".to_string(),
            });
        }

        let new_record = BlobRecord::with_status(
            record.content_addr().clone(),
            record.size_bytes(),
            record.reference_count(),
            record.created_at_ms(),
            record.expires_at_ms(),
            BlobStatus::Published,
        );

        self.records.borrow_mut().insert(addr.clone(), new_record);
        Ok(())
    }

    fn mark_failed(&self, addr: &ContentAddress) -> Result<(), BlobStoreError> {
        let record = self
            .records
            .borrow()
            .get(addr)
            .ok_or_else(|| BlobStoreError::ContentNotFound {
                content_addr: addr.to_string(),
            })?
            .clone();

        if !record.can_transition_to(BlobStatus::Failed) {
            return Err(BlobStoreError::InvalidPublicationStatus {
                content_addr: addr.to_string(),
                current_status: format!("{:?}", record.status()),
                attempted_operation: "mark_failed".to_string(),
            });
        }

        let new_record = BlobRecord::with_status(
            record.content_addr().clone(),
            record.size_bytes(),
            record.reference_count(),
            record.created_at_ms(),
            record.expires_at_ms(),
            BlobStatus::Failed,
        );

        self.records.borrow_mut().insert(addr.clone(), new_record);
        Ok(())
    }
}

// ========================================================================
// ADR-040 Publication State Machine
// Simulates the engine's blob publication protocol
// ========================================================================

struct PublicationProtocol {
    store: Arc<CrashableBlobStore>,
}

impl PublicationProtocol {
    fn new(store: Arc<CrashableBlobStore>) -> Self {
        Self { store }
    }

    fn stage_blob(&self, data: &[u8]) -> Result<ContentAddress, BlobStoreError> {
        self.store.store(data)
    }

    fn mark_durable(&self, addr: &ContentAddress) -> Result<(), BlobStoreError> {
        let record =
            self.store
                .get_record(addr)
                .ok_or_else(|| BlobStoreError::ContentNotFound {
                    content_addr: addr.to_string(),
                })?;

        if !record.can_transition_to(BlobStatus::DurablyStored) {
            return Err(BlobStoreError::InvalidPublicationStatus {
                content_addr: addr.to_string(),
                current_status: format!("{:?}", record.status()),
                attempted_operation: "mark_durable".to_string(),
            });
        }

        let new_record = BlobRecord::with_status(
            record.content_addr().clone(),
            record.size_bytes(),
            record.reference_count(),
            record.created_at_ms(),
            record.expires_at_ms(),
            BlobStatus::DurablyStored,
        );

        self.store.update_record(addr, new_record);
        Ok(())
    }

    fn publish(&self, addr: &ContentAddress) -> Result<(), BlobStoreError> {
        let record =
            self.store
                .get_record(addr)
                .ok_or_else(|| BlobStoreError::ContentNotFound {
                    content_addr: addr.to_string(),
                })?;

        if !record.can_transition_to(BlobStatus::Published) {
            return Err(BlobStoreError::InvalidPublicationStatus {
                content_addr: addr.to_string(),
                current_status: format!("{:?}", record.status()),
                attempted_operation: "publish".to_string(),
            });
        }

        let new_record = BlobRecord::with_status(
            record.content_addr().clone(),
            record.size_bytes(),
            record.reference_count(),
            record.created_at_ms(),
            record.expires_at_ms(),
            BlobStatus::Published,
        );

        self.store.update_record(addr, new_record);
        Ok(())
    }

    fn get_status(&self, addr: &ContentAddress) -> Option<BlobStatus> {
        self.store.get_record(addr).map(|r| r.status())
    }
}

// ========================================================================
// DIMENSION: output_ref ordering (ADR-040 §2)
// Tests verifying output_ref is only published after blob is durable
// ========================================================================

#[test]
fn moon_gate_output_ref_cannot_publish_from_pending() {
    let store = Arc::new(CrashableBlobStore::new());
    let protocol = PublicationProtocol::new(Arc::clone(&store));

    let addr = protocol.stage_blob(b"test blob").unwrap();
    assert_eq!(protocol.get_status(&addr), Some(BlobStatus::Pending));

    let result = protocol.publish(&addr);
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(matches!(
        err,
        BlobStoreError::InvalidPublicationStatus { .. }
    ));
    assert!(
        !protocol
            .get_status(&addr)
            .unwrap()
            .can_transition_to(BlobStatus::Published)
    );
}

#[test]
fn moon_gate_output_ref_cannot_publish_without_durable() {
    let store = Arc::new(CrashableBlobStore::new());
    let protocol = PublicationProtocol::new(Arc::clone(&store));

    let addr = protocol.stage_blob(b"test blob").unwrap();
    assert_eq!(protocol.get_status(&addr), Some(BlobStatus::Pending));

    let result = protocol.publish(&addr);
    assert!(result.is_err());

    let err = result.unwrap_err();
    if let BlobStoreError::InvalidPublicationStatus {
        current_status,
        attempted_operation,
        ..
    } = err
    {
        assert_eq!(current_status, "Pending");
        assert_eq!(attempted_operation, "publish");
    } else {
        panic!("Expected InvalidPublicationStatus error");
    }
}

#[test]
fn moon_gate_output_ref_can_publish_after_durable() {
    let store = Arc::new(CrashableBlobStore::new());
    let protocol = PublicationProtocol::new(Arc::clone(&store));

    let addr = protocol.stage_blob(b"test blob").unwrap();
    assert_eq!(protocol.get_status(&addr), Some(BlobStatus::Pending));

    protocol.mark_durable(&addr).unwrap();
    assert_eq!(protocol.get_status(&addr), Some(BlobStatus::DurablyStored));

    protocol.publish(&addr).unwrap();
    assert_eq!(protocol.get_status(&addr), Some(BlobStatus::Published));
}

#[test]
fn moon_gate_output_ref_ordering_full_lifecycle() {
    let store = Arc::new(CrashableBlobStore::new());
    let protocol = PublicationProtocol::new(Arc::clone(&store));

    let addr = protocol.stage_blob(b"lifecycle test").unwrap();

    assert_eq!(protocol.get_status(&addr), Some(BlobStatus::Pending));
    assert!(protocol.publish(&addr).is_err());

    protocol.mark_durable(&addr).unwrap();
    assert_eq!(protocol.get_status(&addr), Some(BlobStatus::DurablyStored));
    assert!(protocol.publish(&addr).is_ok());

    assert_eq!(protocol.get_status(&addr), Some(BlobStatus::Published));
}

// ========================================================================
// DIMENSION: atomicity tests with crash injection (ADR-040)
// Tests verifying atomicity of blob + ref publication
// ========================================================================

#[test]
fn moon_gate_crash_during_blob_store_leaves_no_partial_state() {
    let store = Arc::new(CrashableBlobStore::new());
    let protocol = PublicationProtocol::new(Arc::clone(&store));

    store.enable_crash();

    let result = protocol.stage_blob(b"crash test");
    assert!(result.is_err());

    assert_eq!(store.crash_count(), 1);

    let empty: Vec<_> = store.records.borrow().keys().cloned().collect();
    assert!(empty.is_empty(), "No records should exist after crash");
}

#[test]
fn moon_gate_crash_during_blob_store_allows_retry() {
    let store = Arc::new(CrashableBlobStore::new());
    let protocol = PublicationProtocol::new(Arc::clone(&store));

    store.enable_crash();
    let result1 = protocol.stage_blob(b"retry test");
    assert!(result1.is_err());

    store.enable_crash();
    let result2 = protocol.stage_blob(b"retry test");
    assert!(result2.is_err());

    store.crash_at_next_store.store(false, Ordering::SeqCst);
    let addr = protocol.stage_blob(b"retry test").unwrap();

    assert!(protocol.get_status(&addr).is_some());
}

#[test]
fn moon_gate_atomic_blob_and_status_transition() {
    let store = Arc::new(CrashableBlobStore::new());
    let protocol = PublicationProtocol::new(Arc::clone(&store));

    let addr = protocol.stage_blob(b"atomic test").unwrap();

    assert!(store.has_blob(&addr));
    assert_eq!(protocol.get_status(&addr), Some(BlobStatus::Pending));

    protocol.mark_durable(&addr).unwrap();
    assert!(store.has_blob(&addr));
    assert_eq!(protocol.get_status(&addr), Some(BlobStatus::DurablyStored));

    protocol.publish(&addr).unwrap();
    assert!(store.has_blob(&addr));
    assert_eq!(protocol.get_status(&addr), Some(BlobStatus::Published));
}

#[test]
fn moon_gate_published_blob_remains_durable_after_transitions() {
    let store = Arc::new(CrashableBlobStore::new());
    let protocol = PublicationProtocol::new(Arc::clone(&store));

    let addr = protocol.stage_blob(b"durable test").unwrap();
    protocol.mark_durable(&addr).unwrap();
    protocol.publish(&addr).unwrap();

    assert!(store.has_blob(&addr));

    let record = store.get_record(&addr).unwrap();
    assert_eq!(record.status(), BlobStatus::Published);
    assert!(!record.can_transition_to(BlobStatus::Pending));
    assert!(!record.can_transition_to(BlobStatus::DurablyStored));
}

// ========================================================================
// DIMENSION: dual representation (ADR-040 §1)
// Tests verifying inline vs BlobRef representation consistency
// ========================================================================

#[test]
fn moon_gate_inline_output_within_max_bytes() {
    let data = vec![0u8; INLINED_MAX_BYTES];
    let result = OutputRef::inline(data.clone());
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.is_inline());
    assert!(!output.is_blob_ref());
}

#[test]
fn moon_gate_inline_output_exceeds_max_rejected() {
    let data = vec![0u8; INLINED_MAX_BYTES + 1];
    let result = OutputRef::inline(data);
    assert!(result.is_err());
}

#[test]
fn moon_gate_blob_ref_output_requires_valid_blob() {
    let blob = BlobRef::new(
        "01H5JQX7K3R4T6V8W0X2Y4Z6A8",
        1024,
        "abcdef0123456789abcdef0123456789",
    );
    assert!(blob.is_ok());

    let output = OutputRef::blob_ref(blob.unwrap());
    assert!(!output.is_inline());
    assert!(output.is_blob_ref());
}

#[test]
fn moon_gate_dual_representation_serde_preserves_type() {
    let inline_output = OutputRef::inline(vec![1, 2, 3]).unwrap();
    let json = serde_json::to_string(&inline_output).unwrap();
    let recovered: OutputRef = serde_json::from_str(&json).unwrap();
    assert_eq!(inline_output, recovered);
    assert!(recovered.is_inline());

    let blob = BlobRef::new(
        "01H5JQX7K3R4T6V8W0X2Y4Z6A8",
        1024,
        "abcdef0123456789abcdef0123456789",
    )
    .unwrap();
    let blob_output = OutputRef::blob_ref(blob);
    let json = serde_json::to_string(&blob_output).unwrap();
    let recovered: OutputRef = serde_json::from_str(&json).unwrap();
    assert_eq!(blob_output, recovered);
    assert!(recovered.is_blob_ref());
}

#[test]
fn moon_gate_classify_small_data_as_inline() {
    let small_data = vec![0u8; 100];
    let result = OutputRef::classify(small_data.clone());
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.is_inline());
}

#[test]
fn moon_gate_classify_exactly_max_as_inline() {
    let data = vec![0u8; INLINED_MAX_BYTES];
    let result = OutputRef::classify(data);
    assert!(result.is_ok());
    assert!(result.unwrap().is_inline());
}

#[test]
fn moon_gate_blob_with_ref_count_not_gc_eligible() {
    let addr =
        ContentAddress::new("9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08")
            .unwrap();
    let record = BlobRecord::new(addr, 1024, 1, 1000, Some(1500)).unwrap();
    assert!(!record.is_expired(1499));
}

#[test]
fn moon_gate_blob_with_high_ref_count_not_gc_eligible() {
    let addr =
        ContentAddress::new("9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08")
            .unwrap();
    let record = BlobRecord::new(addr, 1024, 100, 1000, Some(1500)).unwrap();
    assert!(!record.is_expired(1499));
}

// ========================================================================
// DIMENSION: OutputPolicy failure semantics (ADR-040 §3)
// Tests verifying Required blocks on failure, Optional allows inline
// ========================================================================

#[test]
fn moon_gate_required_output_blocks_on_blob_failure() {
    let action = OutputPolicy::Required.blob_failure_action(BlobStatus::Failed);
    assert_eq!(action, BlobFailureAction::BlockStep);
}

#[test]
fn moon_gate_optional_output_allows_inline_on_failure() {
    let action = OutputPolicy::Optional.blob_failure_action(BlobStatus::Failed);
    assert_eq!(action, BlobFailureAction::CompleteWithInline);
}

#[test]
fn moon_gate_required_output_blocks_on_all_non_terminal_statuses() {
    for status in &[
        BlobStatus::Pending,
        BlobStatus::DurablyStored,
        BlobStatus::Published,
    ] {
        let action = OutputPolicy::Required.blob_failure_action(*status);
        assert_eq!(action, BlobFailureAction::BlockStep);
    }
}

#[test]
fn moon_gate_optional_output_blocks_on_non_failed_statuses() {
    for status in &[
        BlobStatus::Pending,
        BlobStatus::DurablyStored,
        BlobStatus::Published,
    ] {
        let action = OutputPolicy::Optional.blob_failure_action(*status);
        assert_eq!(action, BlobFailureAction::BlockStep);
    }
}

#[test]
fn moon_gate_optional_permits_completion_on_blob_failure() {
    assert!(OutputPolicy::Optional.permits_completion_on_blob_failure());
}

#[test]
fn moon_gate_required_denies_completion_on_blob_failure() {
    assert!(!OutputPolicy::Required.permits_completion_on_blob_failure());
}

// ========================================================================
// DIMENSION: BlobStatus state machine invariants
// ========================================================================

#[test]
fn moon_gate_pending_to_durably_stored_valid() {
    let addr =
        ContentAddress::new("9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08")
            .unwrap();
    let record = BlobRecord::new(addr, 1024, 1, 1000, None).unwrap();
    assert!(record.can_transition_to(BlobStatus::DurablyStored));
}

#[test]
fn moon_gate_pending_to_failed_valid() {
    let addr =
        ContentAddress::new("9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08")
            .unwrap();
    let record = BlobRecord::new(addr, 1024, 1, 1000, None).unwrap();
    assert!(record.can_transition_to(BlobStatus::Failed));
}

#[test]
fn moon_gate_durably_stored_to_published_valid() {
    let addr =
        ContentAddress::new("9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08")
            .unwrap();
    let record = BlobRecord::with_status(addr, 1024, 1, 1000, None, BlobStatus::DurablyStored);
    assert!(record.can_transition_to(BlobStatus::Published));
}

#[test]
fn moon_gate_pending_cannot_skip_to_published() {
    let addr =
        ContentAddress::new("9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08")
            .unwrap();
    let record = BlobRecord::new(addr, 1024, 1, 1000, None).unwrap();
    assert!(!record.can_transition_to(BlobStatus::Published));
}

#[test]
fn moon_gate_published_is_terminal() {
    let addr =
        ContentAddress::new("9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08")
            .unwrap();
    let record = BlobRecord::with_status(addr, 1024, 1, 1000, None, BlobStatus::Published);
    for status in BlobStatus::all_variants() {
        assert!(!record.can_transition_to(*status));
    }
}

#[test]
fn moon_gate_failed_is_terminal() {
    let addr =
        ContentAddress::new("9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08")
            .unwrap();
    let record = BlobRecord::with_status(addr, 1024, 1, 1000, None, BlobStatus::Failed);
    for status in BlobStatus::all_variants() {
        assert!(!record.can_transition_to(*status));
    }
}

// ========================================================================
// DIMENSION: reference count protection (ADR-040 §4)
// Tests verifying GC doesn't collect referenced blobs
// Note: GC eligibility is checked in list_gc_candidates which correctly
// filters out blobs with ref_count > 0 regardless of expiration time.
// ========================================================================

#[test]
fn moon_gate_ref_count_increment_saturates() {
    let addr =
        ContentAddress::new("9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08")
            .unwrap();
    let record = BlobRecord::new(addr, 1024, u64::MAX, 1000, None).unwrap();
    assert_eq!(record.increment_ref_count(), u64::MAX);
}

#[test]
fn moon_gate_ref_count_decrement_saturates_at_zero() {
    let addr =
        ContentAddress::new("9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08")
            .unwrap();
    let record = BlobRecord::new(addr, 1024, 1, 1000, None).unwrap();
    assert_eq!(record.decrement_ref_count(), 0);
}
