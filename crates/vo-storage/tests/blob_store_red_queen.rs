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

use sha2::Digest;
use vo_storage::blob_store::{BlobRecord, ContentAddress};
use vo_types::{
    BlobFailureAction, BlobRef, BlobStatus, OutputPolicy, OutputRef, INLINED_MAX_BYTES,
};

// ========================================================================
// Test helpers
// ========================================================================

const VALID_SHA256: &str = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";
const VALID_SHA256_2: &str = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";

fn make_content_addr() -> ContentAddress {
    ContentAddress::new(VALID_SHA256).unwrap()
}

fn make_blob_ref() -> BlobRef {
    BlobRef::new(
        "01H5JQX7K3R4T6V8W0X2Y4Z6A8",
        1024,
        "abcdef0123456789abcdef0123456789",
    )
    .unwrap()
}

fn make_blob_record(ref_count: u64) -> BlobRecord {
    let content_addr = make_content_addr();
    BlobRecord::new(content_addr, 1024, ref_count, 1000, Some(2000)).unwrap()
}

// ========================================================================
// DIMENSION: output_ref durability before blob
// ADR-040 §2: Pending → DurablyStored → Published state machine
// ========================================================================

#[test]
fn red_queen_blob_status_pending_is_initial() {
    let all_statuses = BlobStatus::all_variants();
    assert!(
        all_statuses.contains(&BlobStatus::Pending),
        "Pending must be a valid status"
    );
}

#[test]
fn red_queen_blob_status_transitions_are_valid() {
    assert!(
        BlobStatus::Pending.can_transition_to(BlobStatus::DurablyStored),
        "Pending → DurablyStored must be valid"
    );
    assert!(
        BlobStatus::Pending.can_transition_to(BlobStatus::Failed),
        "Pending → Failed must be valid"
    );
    assert!(
        BlobStatus::DurablyStored.can_transition_to(BlobStatus::Published),
        "DurablyStored → Published must be valid"
    );
}

#[test]
fn red_queen_blob_status_invalid_transitions_rejected() {
    assert!(
        !BlobStatus::Pending.can_transition_to(BlobStatus::Published),
        "Pending cannot skip to Published"
    );
    assert!(
        !BlobStatus::Published.can_transition_to(BlobStatus::Pending),
        "Published cannot revert to Pending"
    );
    assert!(
        !BlobStatus::Published.can_transition_to(BlobStatus::Failed),
        "Published cannot transition to Failed"
    );
    assert!(
        !BlobStatus::Published.can_transition_to(BlobStatus::DurablyStored),
        "Published cannot transition to DurablyStored"
    );
    assert!(
        !BlobStatus::Failed.can_transition_to(BlobStatus::Pending),
        "Failed cannot revert to Pending"
    );
    assert!(
        !BlobStatus::Failed.can_transition_to(BlobStatus::DurablyStored),
        "Failed cannot transition to DurablyStored"
    );
    assert!(
        !BlobStatus::Failed.can_transition_to(BlobStatus::Published),
        "Failed cannot transition to Published"
    );
    assert!(
        !BlobStatus::DurablyStored.can_transition_to(BlobStatus::Pending),
        "DurablyStored cannot revert to Pending"
    );
    assert!(
        !BlobStatus::DurablyStored.can_transition_to(BlobStatus::Failed),
        "DurablyStored cannot transition to Failed"
    );
}

#[test]
fn red_queen_blob_status_terminal_states_are_truly_terminal() {
    for status in BlobStatus::all_variants() {
        assert!(
            !BlobStatus::Published.can_transition_to(*status),
            "Published must be terminal"
        );
        assert!(
            !BlobStatus::Failed.can_transition_to(*status),
            "Failed must be terminal"
        );
    }
}

#[test]
fn red_queen_blob_status_all_variants_count_is_four() {
    let variants = BlobStatus::all_variants();
    assert_eq!(variants.len(), 4, "Must have exactly 4 status variants");
}

// ========================================================================
// DIMENSION: dual representation consistency
// ADR-040 §2: OutputRef is either Inline(Vec<u8>) or BlobRef(BlobRef)
// ========================================================================

#[test]
fn red_queen_outputref_inline_within_max_bytes() {
    let data = vec![0u8; INLINED_MAX_BYTES];
    let result = OutputRef::inline(data.clone());
    assert!(result.is_ok(), "Must accept exactly INLINED_MAX_BYTES");
    let output = result.unwrap();
    assert!(output.is_inline());
    assert!(!output.is_blob_ref());
    assert_eq!(output.as_inline(), Some(data.as_slice()));
}

#[test]
fn red_queen_outputref_inline_exactly_at_boundary() {
    let data = vec![1u8; INLINED_MAX_BYTES];
    let result = OutputRef::inline(data);
    assert!(
        result.is_ok(),
        "Must accept exactly INLINED_MAX_BYTES bytes"
    );
}

#[test]
fn red_queen_outputref_inline_exceeds_max_rejected() {
    let data = vec![2u8; INLINED_MAX_BYTES + 1];
    let result = OutputRef::inline(data);
    assert!(
        result.is_err(),
        "Must reject data exceeding INLINED_MAX_BYTES"
    );
}

#[test]
fn red_queen_outputref_blob_ref_construction() {
    let blob = make_blob_ref();
    let output = OutputRef::blob_ref(blob.clone());
    assert!(!output.is_inline());
    assert!(output.is_blob_ref());
    assert_eq!(output.as_blob_ref(), Some(&blob));
    assert_eq!(output.as_inline(), None);
}

#[test]
fn red_queen_outputref_inline_and_blob_ref_are_unequal() {
    let inline_output = OutputRef::inline(vec![1, 2, 3]).unwrap();
    let blob_output = OutputRef::blob_ref(make_blob_ref());
    assert_ne!(
        inline_output, blob_output,
        "Inline and BlobRef variants must be unequal"
    );
}

#[test]
fn red_queen_outputref_classify_small_data_as_inline() {
    let small_data = vec![0u8; 100];
    let result = OutputRef::classify(small_data.clone());
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(
        output.is_inline(),
        "Small data must be classified as inline"
    );
}

#[test]
fn red_queen_outputref_classify_exactly_max_as_inline() {
    let data = vec![0u8; INLINED_MAX_BYTES];
    let result = OutputRef::classify(data);
    assert!(result.is_ok(), "Exactly INLINED_MAX_BYTES must succeed");
}

#[test]
fn red_queen_outputref_classify_exceeds_max_rejected() {
    let data = vec![0u8; INLINED_MAX_BYTES + 1];
    let result = OutputRef::classify(data);
    assert!(result.is_err(), "Exceeding INLINED_MAX_BYTES must fail");
}

#[test]
fn red_queen_outputref_dual_representation_serde_preserves_variant() {
    let blob_ref_output = OutputRef::blob_ref(make_blob_ref());
    let json = serde_json::to_string(&blob_ref_output).unwrap();
    let recovered: OutputRef = serde_json::from_str(&json).unwrap();
    assert_eq!(blob_ref_output, recovered);
    assert!(recovered.is_blob_ref());

    let inline_output = OutputRef::inline(vec![5, 6, 7]).unwrap();
    let json = serde_json::to_string(&inline_output).unwrap();
    let recovered: OutputRef = serde_json::from_str(&json).unwrap();
    assert_eq!(inline_output, recovered);
    assert!(recovered.is_inline());
}

#[test]
fn red_queen_outputref_empty_inline_is_valid() {
    let result = OutputRef::inline(vec![]);
    assert!(result.is_ok(), "Empty inline data must be valid");
    let output = result.unwrap();
    assert!(output.is_inline());
    assert_eq!(output.as_inline(), Some(&[][..]));
}

// ========================================================================
// DIMENSION: GC of referenced blobs
// BlobRecord.ref_count prevents collection when > 0
// ========================================================================

#[test]
fn red_queen_blob_record_with_nonzero_ref_count_not_gc_eligible() {
    let record = make_blob_record(1);
    assert_eq!(record.reference_count(), 1, "ref_count must be 1");
    assert!(
        !record.is_gc_eligible(2000),
        "record with ref_count=1 must not be GC eligible regardless of expiry"
    );
}

#[test]
fn red_queen_blob_record_with_high_ref_count_not_gc_eligible() {
    let content_addr = make_content_addr();
    let record = BlobRecord::new(content_addr, 1024, 100, 1000, Some(1500)).unwrap();
    assert!(
        !record.is_gc_eligible(3000),
        "ref_count=100 must prevent GC even when expired"
    );
}

#[test]
fn red_queen_blob_record_zero_ref_count_expired_is_gc_eligible() {
    let content_addr = make_content_addr();
    let record = BlobRecord::new(content_addr, 1024, 0, 1000, Some(1500));
    assert!(
        record.is_err(),
        "ref_count=0 should not be allowed on construction"
    );
}

#[test]
fn red_queen_blob_record_increment_ref_count_saturates() {
    let record = make_blob_record(u64::MAX);
    let new_count = record.increment_ref_count();
    assert_eq!(new_count, u64::MAX, "increment must saturate at MAX");
}

#[test]
fn red_queen_blob_record_decrement_ref_count_saturates_at_zero() {
    let record = make_blob_record(1);
    let new_count = record.decrement_ref_count();
    assert_eq!(new_count, 0, "decrement must saturate at 0");
}

#[test]
fn red_queen_blob_record_decrement_from_zero_saturates() {
    let content_addr = make_content_addr();
    let record = BlobRecord::new(content_addr, 1024, 0, 1000, None);
    assert!(record.is_err(), "ref_count=0 is invalid on construction");
}

#[test]
fn red_queen_blob_record_expires_at_none_never_expires() {
    let content_addr = make_content_addr();
    let record = BlobRecord::new(content_addr, 1024, 1, 1000, None).unwrap();
    assert!(
        !record.is_expired(u64::MAX),
        "expires_at=None must mean never expires"
    );
}

#[test]
fn red_queen_blob_record_expires_at_boundary() {
    let content_addr = make_content_addr();
    let record = BlobRecord::new(content_addr, 1024, 1, 1000, Some(1500)).unwrap();
    assert!(!record.is_expired(1499), "1499 < 1500 → not expired");
    assert!(record.is_expired(1500), "1500 >= 1500 → expired");
}

// ========================================================================
// DIMENSION: concurrent publish race
// Multiple concurrent publications of the same content
// ========================================================================

#[test]
fn red_queen_concurrent_publish_same_content_deduplication() {
    let store = std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::<
        ContentAddress,
        Vec<u8>,
    >::new()));

    let data = b"identical content for deduplication".to_vec();
    let num_threads = 16;
    let barrier = std::sync::Barrier::new(num_threads);
    let data_clone = data.clone();

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let store = std::sync::Arc::clone(&store);
            let barrier = std::sync::Barrier::new(num_threads);
            std::thread::spawn({
                let value = data_clone.clone();
                move || {
                    barrier.wait();
                    let mut guard = store.lock().unwrap();
                    let content_addr =
                        ContentAddress::from_bytes(&sha2::Sha256::digest(&value).into());
                    if !guard.contains_key(&content_addr) {
                        guard.insert(content_addr.clone(), value.clone());
                    }
                    guard.len()
                }
            })
        })
        .collect();

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    let unique_inserts = results.iter().min().unwrap();
    let max_results = results.iter().max().unwrap();
    assert_eq!(
        *unique_inserts, 1,
        "BUG: Only one thread should have inserted the content"
    );
    assert_eq!(
        *max_results, 1,
        "BUG: Final count must be exactly 1 (dedup)"
    );
}

#[test]
fn red_queen_concurrent_publish_different_content_no_dedup() {
    let store = std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::<
        ContentAddress,
        Vec<u8>,
    >::new()));

    let num_threads = 16;
    let barrier = std::sync::Barrier::new(num_threads);

    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let store = std::sync::Arc::clone(&store);
            let barrier = std::sync::Barrier::new(num_threads);
            std::thread::spawn(move || {
                barrier.wait();
                let data = format!("unique content {}", i).into_bytes();
                let content_addr = ContentAddress::from_bytes(&sha2::Sha256::digest(&data).into());
                let mut guard = store.lock().unwrap();
                guard.insert(content_addr, data);
                guard.len()
            })
        })
        .collect();

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    let final_count = *results.iter().max().unwrap();
    assert_eq!(
        final_count, num_threads,
        "BUG: Each unique content must be stored separately"
    );
}

#[test]
fn red_queen_concurrent_ref_count_increment_thread_safety() {
    let counter = std::sync::Arc::new(std::sync::Mutex::new(0u64));
    let num_threads = 16;
    let iterations = 100;

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let counter = std::sync::Arc::clone(&counter);
            std::thread::spawn(move || {
                for _ in 0..iterations {
                    let mut guard = counter.lock().unwrap();
                    *guard += 1;
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let final_count = *counter.lock().unwrap();
    assert_eq!(
        final_count,
        (num_threads * iterations) as u64,
        "BUG: All increments must be accounted for"
    );
}

// ========================================================================
// DIMENSION: ContentAddress invariants
// ========================================================================

#[test]
fn red_queen_content_address_valid_sha256_roundtrip() {
    let addr = make_content_addr();
    let bytes = addr.as_bytes();
    let roundtrip = ContentAddress::from_bytes(&bytes);
    assert_eq!(roundtrip.as_str(), VALID_SHA256);
}

#[test]
fn red_queen_content_address_rejects_uppercase() {
    let upper = "ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789";
    let result = ContentAddress::new(upper);
    assert!(result.is_err(), "ContentAddress must reject uppercase hex");
}

#[test]
fn red_queen_content_address_rejects_wrong_length() {
    let short = "abc123";
    let result = ContentAddress::new(short);
    assert!(
        result.is_err(),
        "ContentAddress must reject non-64-char strings"
    );
}

#[test]
fn red_queen_content_address_rejects_non_hex() {
    let non_hex = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15g0f00a08";
    let result = ContentAddress::new(non_hex);
    assert!(
        result.is_err(),
        "ContentAddress must reject non-hex characters"
    );
}

#[test]
fn red_queen_content_address_empty_rejected() {
    let result = ContentAddress::new("");
    assert!(result.is_err(), "ContentAddress must reject empty string");
}

// ========================================================================
// DIMENSION: OutputPolicy failure semantics (ADR-040 §3)
// ========================================================================

#[test]
fn red_queen_output_policy_required_blocks_on_blob_failure() {
    let action = OutputPolicy::Required.blob_failure_action(BlobStatus::Failed);
    assert_eq!(
        action,
        BlobFailureAction::BlockStep,
        "Required output must block step on blob failure"
    );
}

#[test]
fn red_queen_output_policy_optional_allows_inline_on_blob_failure() {
    let action = OutputPolicy::Optional.blob_failure_action(BlobStatus::Failed);
    assert_eq!(
        action,
        BlobFailureAction::CompleteWithInline,
        "Optional output must allow inline completion on blob failure"
    );
}

#[test]
fn red_queen_output_policy_non_failed_status_blocks_regardless() {
    let statuses = [
        BlobStatus::Pending,
        BlobStatus::DurablyStored,
        BlobStatus::Published,
    ];
    for status in statuses {
        let required_action = OutputPolicy::Required.blob_failure_action(status);
        assert_eq!(
            required_action,
            BlobFailureAction::BlockStep,
            "Required policy must block for non-failed status {:?}",
            status
        );

        let optional_action = OutputPolicy::Optional.blob_failure_action(status);
        assert_eq!(
            optional_action,
            BlobFailureAction::BlockStep,
            "Optional policy must block for non-failed status {:?}",
            status
        );
    }
}

#[test]
fn red_queen_output_policy_required_is_required_for_replay() {
    assert!(
        OutputPolicy::Required.is_required_for_replay(),
        "Required must be required for replay"
    );
}

#[test]
fn red_queen_output_policy_optional_not_required_for_replay() {
    assert!(
        !OutputPolicy::Optional.is_required_for_replay(),
        "Optional must NOT be required for replay"
    );
}

#[test]
fn red_queen_output_policy_optional_permits_completion() {
    assert!(
        OutputPolicy::Optional.permits_completion_on_blob_failure(),
        "Optional must permit completion on blob failure"
    );
}

#[test]
fn red_queen_output_policy_required_denies_completion() {
    assert!(
        !OutputPolicy::Required.permits_completion_on_blob_failure(),
        "Required must deny completion on blob failure"
    );
}

// ========================================================================
// DIMENSION: BlobRef invariants
// ========================================================================

#[test]
fn red_queen_blobref_valid_construction() {
    let blob = make_blob_ref();
    assert_eq!(blob.blob_id(), "01H5JQX7K3R4T6V8W0X2Y4Z6A8");
    assert_eq!(blob.size_bytes(), 1024);
    assert_eq!(blob.content_hash(), "abcdef0123456789abcdef0123456789");
}

#[test]
fn red_queen_blobref_rejects_empty_blob_id() {
    let result = BlobRef::new("", 1024, "abcdef0123456789abcdef0123456789");
    assert!(result.is_err(), "blob_id cannot be empty");
}

#[test]
fn red_queen_blobref_rejects_invalid_ulid() {
    let result = BlobRef::new("not-a-ulid", 1024, "abcdef0123456789abcdef0123456789");
    assert!(result.is_err(), "blob_id must be valid ULID");
}

#[test]
fn red_queen_blobref_rejects_wrong_length_blob_id() {
    let result = BlobRef::new("01H5JQX7K3", 1024, "abcdef0123456789abcdef0123456789");
    assert!(result.is_err(), "blob_id must be exactly 26 chars");
}

#[test]
fn red_queen_blobref_rejects_zero_size() {
    let result = BlobRef::new(
        "01H5JQX7K3R4T6V8W0X2Y4Z6A8",
        0,
        "abcdef0123456789abcdef0123456789",
    );
    assert!(result.is_err(), "size_bytes cannot be zero");
}

#[test]
fn red_queen_blobref_rejects_empty_content_hash() {
    let result = BlobRef::new("01H5JQX7K3R4T6V8W0X2Y4Z6A8", 1024, "");
    assert!(result.is_err(), "content_hash cannot be empty");
}

#[test]
fn red_queen_blobref_rejects_non_hex_content_hash() {
    let result = BlobRef::new("01H5JQX7K3R4T6V8W0X2Y4Z6A8", 1024, "ghijklmnopqrstuv");
    assert!(result.is_err(), "content_hash must be lowercase hex");
}

#[test]
fn red_queen_blobref_rejects_odd_length_content_hash() {
    let result = BlobRef::new("01H5JQX7K3R4T6V8W0X2Y4Z6A8", 1024, "abcde");
    assert!(result.is_err(), "content_hash must have even length");
}

#[test]
fn red_queen_blobref_rejects_short_content_hash() {
    let result = BlobRef::new("01H5JQX7K3R4T6V8W0X2Y4Z6A8", 1024, "ab");
    assert!(result.is_err(), "content_hash must be at least 8 chars");
}

// ========================================================================
// DIMENSION: encoding roundtrips
// ========================================================================

#[test]
fn red_queen_content_address_encode_decode_roundtrip() {
    use vo_storage::blob_store::{decode_content_address, encode_content_address};
    let addr = make_content_addr();
    let encoded = encode_content_address(&addr);
    let decoded = decode_content_address(&encoded).unwrap();
    assert_eq!(addr, decoded);
}

#[test]
fn red_queen_blob_record_encode_decode_roundtrip() {
    use vo_storage::blob_store::{decode_blob_record, encode_blob_record};
    let record = make_blob_record(5);
    let encoded = encode_blob_record(&record).unwrap();
    let decoded = decode_blob_record(&encoded).unwrap();
    assert_eq!(record.content_addr(), decoded.content_addr());
    assert_eq!(record.size_bytes(), decoded.size_bytes());
    assert_eq!(record.reference_count(), decoded.reference_count());
}

// ========================================================================
// DIMENSION: error display format
// ========================================================================

#[test]
fn red_queen_blob_store_error_display_all_variants() {
    use vo_storage::blob_store::BlobStoreError;

    let errors = [
        BlobStoreError::ContentNotFound {
            content_addr: "abc".to_string(),
        },
        BlobStoreError::PackFileNotFound {
            pack_file_id: "pack-1".to_string(),
        },
        BlobStoreError::DuplicateContent {
            content_addr: "def".to_string(),
        },
        BlobStoreError::CorruptPackIndex {
            reason: "bad index".to_string(),
        },
        BlobStoreError::CorruptPackFile {
            pack_file_id: "pack-2".to_string(),
            reason: "truncated".to_string(),
        },
        BlobStoreError::ChecksumMismatch {
            content_addr: "xyz".to_string(),
            expected: "exp".to_string(),
            actual: "act".to_string(),
        },
        BlobStoreError::SerializationFailed {
            reason: "json error".to_string(),
        },
        BlobStoreError::DeserializationFailed {
            reason: "parse error".to_string(),
        },
        BlobStoreError::Storage {
            reason: "disk full".to_string(),
        },
        BlobStoreError::InvalidArgument {
            reason: "bad input".to_string(),
        },
        BlobStoreError::GcCycleInProgress,
        BlobStoreError::PackFileFull {
            pack_file_id: "pack-3".to_string(),
            max_size_bytes: 1000,
        },
    ];

    for err in errors {
        let display = err.to_string();
        assert!(
            !display.is_empty(),
            "Error {:?} must have non-empty Display",
            err
        );
    }
}

// ========================================================================
// DIMENSION: INLINED_MAX_BYTES constant correctness
// ========================================================================

#[test]
fn red_queen_inlined_max_bytes_is_4096() {
    assert_eq!(INLINED_MAX_BYTES, 4096, "INLINED_MAX_BYTES must be 4096");
}
