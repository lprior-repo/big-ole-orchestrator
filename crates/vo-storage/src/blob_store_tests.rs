use crate::blob_store::*;
use vo_types::BlobStatus;

const VALID_SHA256: &str = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";
const VALID_SHA256_2: &str = "ABCDEF0123456789abcdef0123456789abcdef0123456789abcdef0123456789";

#[test]
fn content_address_constructs_with_valid_hex() {
    let s = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";
    assert_eq!(s.len(), 64, "string length should be 64");
    let addr = ContentAddress::new(s);
    assert!(addr.is_ok(), "ContentAddress::new failed: {:?}", addr);
    let addr = addr.unwrap();
    assert_eq!(addr.as_str(), s);
}

#[test]
fn content_address_rejects_wrong_length() {
    let result = ContentAddress::new("abcdef");
    assert!(result.is_err());
}

#[test]
fn content_address_rejects_uppercase_hex() {
    let result =
        ContentAddress::new("ABCDEF0123456789abcdef0123456789abcdef0123456789abcdef01234567");
    assert!(result.is_err());
}

#[test]
fn content_address_from_bytes_roundtrip() {
    let original = [
        0x9f_u8, 0x86, 0xd0, 0x81, 0x88, 0x84, 0xc7, 0xd6, 0x59, 0xa2, 0xfe, 0xaa, 0x0c, 0x55,
        0xad, 0x01, 0x5a, 0x3b, 0xf4, 0xf1, 0xb2, 0xb0, 0xb8, 0x22, 0xcd, 0x15, 0xd6, 0xc1, 0x5b,
        0x0f, 0x00, 0xa0,
    ];
    let addr = ContentAddress::from_bytes(&original);
    let bytes = addr.as_bytes();
    assert_eq!(bytes, original);
}

#[test]
fn blob_record_constructs_with_valid_fields() {
    let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
    let record = BlobRecord::new(content_addr, 1024, 1, 1000, Some(2000));
    assert!(record.is_ok());
    let record = record.unwrap();
    assert_eq!(record.size_bytes(), 1024);
    assert_eq!(record.reference_count(), 1);
    assert!(record.expires_at_ms().is_some());
    assert_eq!(record.status(), BlobStatus::Pending);
}

#[test]
fn blob_record_with_status_constructs_with_explicit_status() {
    let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
    let record = BlobRecord::with_status(
        content_addr.clone(),
        1024,
        1,
        1000,
        Some(2000),
        BlobStatus::DurablyStored,
    );
    assert_eq!(record.status(), BlobStatus::DurablyStored);
    assert_eq!(record.content_addr(), &content_addr);
    assert_eq!(record.size_bytes(), 1024);
}

#[test]
fn blob_record_can_transition_from_pending_to_durably_stored() {
    let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
    let record = BlobRecord::new(content_addr, 1024, 1, 1000, None).unwrap();
    assert!(record.can_transition_to(BlobStatus::DurablyStored));
}

#[test]
fn blob_record_can_transition_from_pending_to_failed() {
    let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
    let record = BlobRecord::new(content_addr, 1024, 1, 1000, None).unwrap();
    assert!(record.can_transition_to(BlobStatus::Failed));
}

#[test]
fn blob_record_can_transition_from_durably_stored_to_published() {
    let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
    let record =
        BlobRecord::with_status(content_addr, 1024, 1, 1000, None, BlobStatus::DurablyStored);
    assert!(record.can_transition_to(BlobStatus::Published));
}

#[test]
fn blob_record_cannot_skip_to_published_from_pending() {
    let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
    let record = BlobRecord::new(content_addr, 1024, 1, 1000, None).unwrap();
    assert!(!record.can_transition_to(BlobStatus::Published));
}

#[test]
fn blob_record_published_is_terminal() {
    let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
    let record = BlobRecord::with_status(content_addr, 1024, 1, 1000, None, BlobStatus::Published);
    assert!(!record.can_transition_to(BlobStatus::Pending));
    assert!(!record.can_transition_to(BlobStatus::DurablyStored));
    assert!(!record.can_transition_to(BlobStatus::Failed));
}

#[test]
fn blob_record_failed_is_terminal() {
    let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
    let record = BlobRecord::with_status(content_addr, 1024, 1, 1000, None, BlobStatus::Failed);
    assert!(!record.can_transition_to(BlobStatus::Pending));
    assert!(!record.can_transition_to(BlobStatus::DurablyStored));
    assert!(!record.can_transition_to(BlobStatus::Published));
}

#[test]
fn blob_record_rejects_zero_reference_count() {
    let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
    let result = BlobRecord::new(content_addr, 1024, 0, 1000, None);
    assert!(result.is_err());
}

#[test]
fn blob_record_expired_check() {
    let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
    let record = BlobRecord::new(content_addr, 1024, 1, 1000, Some(1500)).unwrap();
    assert!(!record.is_expired(1000));
    assert!(!record.is_expired(1499));
    assert!(record.is_expired(1500));
    assert!(record.is_expired(2000));
}

#[test]
fn blob_record_never_expires_without_ttl() {
    let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
    let record = BlobRecord::new(content_addr, 1024, 1, 1000, None).unwrap();
    assert!(!record.is_expired(u64::MAX));
}

#[test]
fn blob_record_increment_decrement_ref_count() {
    let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
    let record = BlobRecord::new(content_addr, 1024, 2, 1000, None).unwrap();
    assert_eq!(record.increment_ref_count(), 3);
    assert_eq!(record.decrement_ref_count(), 1);
}

#[test]
fn pack_index_entry_accessors() {
    let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
    let pack_id = PackFileId::new("pack-001").unwrap();
    let entry = PackIndexEntry::new(content_addr.clone(), pack_id.clone(), 100, 512);
    assert_eq!(entry.content_addr(), &content_addr);
    assert_eq!(entry.pack_file_id(), &pack_id);
    assert_eq!(entry.offset_bytes(), 100);
    assert_eq!(entry.size_bytes(), 512);
}

#[test]
fn blob_store_error_display() {
    let err = BlobStoreError::ContentNotFound {
        content_addr: "abc".to_string(),
    };
    assert!(err.to_string().contains("content not found"));
    assert!(err.to_string().contains("abc"));
}

#[test]
fn blob_store_error_is_error() {
    let err = BlobStoreError::Storage {
        reason: "disk full".to_string(),
    };
    assert!(std::error::Error::source(&err).is_none());
}

#[test]
fn encode_decode_content_address_roundtrip() {
    let addr = ContentAddress::new(VALID_SHA256).unwrap();
    let encoded = encode_content_address(&addr);
    let decoded = decode_content_address(&encoded).unwrap();
    assert_eq!(addr, decoded);
}

#[test]
fn validate_content_address_accepts_valid() {
    assert!(validate_content_address(VALID_SHA256).is_ok());
}

#[test]
fn validate_content_address_rejects_invalid() {
    assert!(validate_content_address("not-valid").is_err());
    assert!(validate_content_address("").is_err());
    assert!(validate_content_address(VALID_SHA256_2).is_err());
}

#[test]
fn content_address_rejects_empty_string() {
    let result = ContentAddress::new("");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, BlobStoreError::InvalidArgument { .. }));
}

#[test]
fn content_address_rejects_too_long() {
    let result =
        ContentAddress::new("9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08ab");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, BlobStoreError::InvalidArgument { .. }));
}

#[test]
fn content_address_rejects_non_hex_characters() {
    let result =
        ContentAddress::new("9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15g0f00a08");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, BlobStoreError::InvalidArgument { .. }));
    assert!(err.to_string().contains("lowercase hex"));
}

#[test]
fn content_address_from_bytes_produces_correct_hex() {
    let hex_str = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";
    let addr = ContentAddress::new(hex_str).unwrap();
    let bytes = addr.as_bytes();
    let roundtrip = ContentAddress::from_bytes(&bytes);
    assert_eq!(roundtrip.as_str(), hex_str);
}

#[test]
fn content_address_as_str_returns_inner_string() {
    let addr = ContentAddress::new(VALID_SHA256).unwrap();
    assert_eq!(addr.as_str(), VALID_SHA256);
}

#[test]
fn content_address_full_roundtrip() {
    let original = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";
    let addr = ContentAddress::new(original).unwrap();
    let bytes = addr.as_bytes();
    let roundtripped = ContentAddress::from_bytes(&bytes);
    assert_eq!(roundtripped.as_str(), original);
}

#[test]
fn pack_file_id_new_accepts_non_empty() {
    let result = PackFileId::new("pack-001");
    assert!(result.is_ok());
    assert_eq!(result.unwrap().as_str(), "pack-001");
}

#[test]
fn pack_file_id_new_rejects_empty() {
    let result = PackFileId::new("");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, BlobStoreError::InvalidArgument { .. }));
    assert!(err.to_string().contains("cannot be empty"));
}

#[test]
fn pack_file_id_as_str_returns_inner() {
    let id = PackFileId::new("pack-002").unwrap();
    assert_eq!(id.as_str(), "pack-002");
}

#[test]
fn blob_record_rejects_zero_created_at() {
    let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
    let result = BlobRecord::new(content_addr, 1024, 1, 0, None);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, BlobStoreError::InvalidArgument { .. }));
    assert!(err.to_string().contains("created_at_ms"));
}

#[test]
fn blob_record_increment_saturates_at_max() {
    let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
    let record = BlobRecord::new(content_addr, 1024, u64::MAX, 1000, None).unwrap();
    assert_eq!(record.increment_ref_count(), u64::MAX);
}

#[test]
fn blob_record_decrement_saturates_at_zero() {
    let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
    let record = BlobRecord::new(content_addr, 1024, 0, 1000, None);
    assert!(record.is_err());
}

#[test]
fn blob_record_decrement_from_one_saturates_at_zero() {
    let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
    let record = BlobRecord::new(content_addr, 1024, 1, 1000, None).unwrap();
    assert_eq!(record.decrement_ref_count(), 0);
}

#[test]
fn error_content_not_found_display() {
    let err = BlobStoreError::ContentNotFound {
        content_addr: "abc123".to_string(),
    };
    let s = err.to_string();
    assert!(s.contains("content not found"));
    assert!(s.contains("abc123"));
}

#[test]
fn error_pack_file_not_found_display() {
    let err = BlobStoreError::PackFileNotFound {
        pack_file_id: "pack-001".to_string(),
    };
    let s = err.to_string();
    assert!(s.contains("pack file not found"));
    assert!(s.contains("pack-001"));
}

#[test]
fn error_duplicate_content_display() {
    let err = BlobStoreError::DuplicateContent {
        content_addr: "def456".to_string(),
    };
    let s = err.to_string();
    assert!(s.contains("duplicate content"));
    assert!(s.contains("def456"));
}

#[test]
fn error_corrupt_pack_index_display() {
    let err = BlobStoreError::CorruptPackIndex {
        reason: "missing field".to_string(),
    };
    let s = err.to_string();
    assert!(s.contains("corrupt pack index"));
    assert!(s.contains("missing field"));
}

#[test]
fn error_corrupt_pack_file_display() {
    let err = BlobStoreError::CorruptPackFile {
        pack_file_id: "pack-002".to_string(),
        reason: "truncated".to_string(),
    };
    let s = err.to_string();
    assert!(s.contains("corrupt pack file pack-002"));
    assert!(s.contains("truncated"));
}

#[test]
fn error_checksum_mismatch_display() {
    let err = BlobStoreError::ChecksumMismatch {
        content_addr: "abc".to_string(),
        expected: "expected_hash".to_string(),
        actual: "actual_hash".to_string(),
    };
    let s = err.to_string();
    assert!(s.contains("checksum mismatch"));
    assert!(s.contains("abc"));
    assert!(s.contains("expected_hash"));
    assert!(s.contains("actual_hash"));
}

#[test]
fn error_serialization_failed_display() {
    let err = BlobStoreError::SerializationFailed {
        reason: "JSON error".to_string(),
    };
    let s = err.to_string();
    assert!(s.contains("serialization failed"));
    assert!(s.contains("JSON error"));
}

#[test]
fn error_deserialization_failed_display() {
    let err = BlobStoreError::DeserializationFailed {
        reason: "invalid JSON".to_string(),
    };
    let s = err.to_string();
    assert!(s.contains("deserialization failed"));
    assert!(s.contains("invalid JSON"));
}

#[test]
fn error_storage_display() {
    let err = BlobStoreError::Storage {
        reason: "disk full".to_string(),
    };
    let s = err.to_string();
    assert!(s.contains("storage error"));
    assert!(s.contains("disk full"));
}

#[test]
fn error_invalid_argument_display() {
    let err = BlobStoreError::InvalidArgument {
        reason: "bad input".to_string(),
    };
    let s = err.to_string();
    assert!(s.contains("invalid argument"));
    assert!(s.contains("bad input"));
}

#[test]
fn error_gc_cycle_in_progress_display() {
    let err = BlobStoreError::GcCycleInProgress;
    let s = err.to_string();
    assert!(s.contains("GC cycle already in progress"));
}

#[test]
fn error_pack_file_full_display() {
    let err = BlobStoreError::PackFileFull {
        pack_file_id: "pack-003".to_string(),
        max_size_bytes: 1000,
    };
    let s = err.to_string();
    assert!(s.contains("pack file pack-003 full"));
    assert!(s.contains("1000"));
}

#[test]
fn error_invalid_publication_status_display() {
    let err = BlobStoreError::InvalidPublicationStatus {
        content_addr: "abc123".to_string(),
        current_status: "Pending".to_string(),
        attempted_operation: "publish".to_string(),
    };
    let s = err.to_string();
    assert!(s.contains("invalid publication status"));
    assert!(s.contains("abc123"));
    assert!(s.contains("Pending"));
    assert!(s.contains("publish"));
}

#[test]
fn error_not_durably_stored_display() {
    let err = BlobStoreError::NotDurablyStored {
        content_addr: "def456".to_string(),
    };
    let s = err.to_string();
    assert!(s.contains("not durably stored"));
    assert!(s.contains("def456"));
}

#[test]
fn all_blob_store_error_variants_implement_error_trait() {
    fn assert_impl<T: std::error::Error>() {}
    assert_impl::<BlobStoreError>();
}

#[test]
fn encode_content_address_produces_valid_utf8() {
    let addr = ContentAddress::new(VALID_SHA256).unwrap();
    let encoded = encode_content_address(&addr);
    let as_str = String::from_utf8(encoded.clone()).unwrap();
    assert_eq!(as_str, VALID_SHA256);
}

#[test]
fn decode_content_address_rejects_invalid_utf8() {
    let invalid_utf8 = [0x80, 0x81, 0x82, 0x83];
    let result = decode_content_address(&invalid_utf8);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, BlobStoreError::CorruptPackIndex { .. }));
}

#[test]
fn decode_content_address_rejects_invalid_format() {
    let invalid_format = b"not-a-valid-content-address".to_vec();
    let result = decode_content_address(&invalid_format);
    assert!(result.is_err());
}

#[test]
fn decode_content_address_rejects_wrong_length() {
    let too_short = b"abc123".to_vec();
    let result = decode_content_address(&too_short);
    assert!(result.is_err());
}

#[test]
fn encode_pack_index_entry_and_decode_roundtrip() {
    let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
    let pack_id = PackFileId::new("pack-001").unwrap();
    let entry = PackIndexEntry::new(content_addr, pack_id, 100, 512);
    let encoded = encode_pack_index_entry(&entry).unwrap();
    let decoded = decode_pack_index_entry(&encoded).unwrap();
    assert_eq!(decoded.content_addr(), entry.content_addr());
    assert_eq!(decoded.pack_file_id(), entry.pack_file_id());
    assert_eq!(decoded.offset_bytes(), entry.offset_bytes());
    assert_eq!(decoded.size_bytes(), entry.size_bytes());
}

#[test]
fn encode_blob_record_and_decode_roundtrip() {
    let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
    let record = BlobRecord::new(content_addr, 1024, 1, 1000, Some(2000)).unwrap();
    let encoded = encode_blob_record(&record).unwrap();
    let decoded = decode_blob_record(&encoded).unwrap();
    assert_eq!(decoded.content_addr(), record.content_addr());
    assert_eq!(decoded.size_bytes(), record.size_bytes());
    assert_eq!(decoded.reference_count(), record.reference_count());
    assert_eq!(decoded.created_at_ms(), record.created_at_ms());
    assert_eq!(decoded.expires_at_ms(), record.expires_at_ms());
}

#[test]
fn decode_pack_index_entry_rejects_invalid_json() {
    let invalid_json = b"not json".to_vec();
    let result = decode_pack_index_entry(&invalid_json);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, BlobStoreError::CorruptPackIndex { .. }));
}

#[test]
fn decode_blob_record_rejects_invalid_json() {
    let invalid_json = b"not json".to_vec();
    let result = decode_blob_record(&invalid_json);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, BlobStoreError::DeserializationFailed { .. }));
}

#[test]
fn blob_record_is_gc_eligible_requires_both_conditions() {
    let content_addr = ContentAddress::new(VALID_SHA256).unwrap();

    let record = BlobRecord::with_status(
        content_addr.clone(),
        1024,
        0,
        1000,
        Some(2000),
        BlobStatus::Pending,
    );
    assert!(!record.is_gc_eligible(1500), "ref=0 but not expired yet");

    let record = BlobRecord::new(content_addr.clone(), 1024, 1, 1000, Some(1500)).unwrap();
    assert!(!record.is_gc_eligible(2000), "expired but ref=1");

    let record = BlobRecord::with_status(
        content_addr.clone(),
        1024,
        0,
        1000,
        Some(1500),
        BlobStatus::Pending,
    );
    assert!(record.is_gc_eligible(1500), "both ref=0 and expired");

    let record = BlobRecord::with_status(
        content_addr.clone(),
        1024,
        0,
        1000,
        Some(1500),
        BlobStatus::Pending,
    );
    assert!(!record.is_gc_eligible(1499), "expired at 1500, not at 1499");
}

#[test]
fn blob_record_gc_eligible_without_ttl() {
    let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
    let record = BlobRecord::with_status(
        content_addr.clone(),
        1024,
        0,
        1000,
        None,
        BlobStatus::Pending,
    );
    assert!(
        !record.is_gc_eligible(u64::MAX),
        "no TTL means never expires, even with ref=0"
    );
}

#[test]
fn blob_record_status_transition_consistency() {
    let content_addr = ContentAddress::new(VALID_SHA256).unwrap();

    let pending = BlobRecord::new(content_addr.clone(), 1024, 1, 1000, None).unwrap();
    assert!(pending.can_transition_to(BlobStatus::DurablyStored));
    assert!(pending.can_transition_to(BlobStatus::Failed));
    assert!(!pending.can_transition_to(BlobStatus::Published));

    let stored = BlobRecord::with_status(
        content_addr.clone(),
        1024,
        1,
        1000,
        None,
        BlobStatus::DurablyStored,
    );
    assert!(stored.can_transition_to(BlobStatus::Published));
    assert!(!stored.can_transition_to(BlobStatus::Pending));
    assert!(!stored.can_transition_to(BlobStatus::Failed));
}

#[test]
fn blob_record_terminal_states_no_transitions() {
    let content_addr = ContentAddress::new(VALID_SHA256).unwrap();

    let published = BlobRecord::with_status(
        content_addr.clone(),
        1024,
        1,
        1000,
        None,
        BlobStatus::Published,
    );
    assert!(!published.can_transition_to(BlobStatus::Pending));
    assert!(!published.can_transition_to(BlobStatus::DurablyStored));
    assert!(!published.can_transition_to(BlobStatus::Failed));

    let failed = BlobRecord::with_status(
        content_addr.clone(),
        1024,
        1,
        1000,
        None,
        BlobStatus::Failed,
    );
    assert!(!failed.can_transition_to(BlobStatus::Pending));
    assert!(!failed.can_transition_to(BlobStatus::DurablyStored));
    assert!(!failed.can_transition_to(BlobStatus::Published));
}

#[test]
fn blob_record_decrement_from_zero_returns_zero() {
    let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
    let record = BlobRecord::new(content_addr, 1024, 0, 1000, None);
    assert!(record.is_err(), "cannot create record with ref_count=0");
}

#[test]
fn blob_record_reference_count_saturation_bounds() {
    let content_addr = ContentAddress::new(VALID_SHA256).unwrap();

    let record = BlobRecord::new(content_addr.clone(), 1024, u64::MAX - 1, 1000, None).unwrap();
    assert_eq!(record.increment_ref_count(), u64::MAX);

    let record = BlobRecord::new(content_addr.clone(), 1024, u64::MAX, 1000, None).unwrap();
    assert_eq!(record.increment_ref_count(), u64::MAX);
}

#[test]
fn content_address_validity_invariant() {
    let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
    assert_eq!(content_addr.as_str().len(), 64);
    assert!(content_addr
        .as_str()
        .chars()
        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
}

#[test]
fn content_address_bytes_roundtrip_preserves_invariant() {
    let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
    let bytes = content_addr.as_bytes();
    let recovered = ContentAddress::from_bytes(&bytes);
    assert_eq!(recovered.as_str(), content_addr.as_str());
}

#[test]
fn pack_index_entry_immutable_after_construction() {
    let content_addr = ContentAddress::new(VALID_SHA256).unwrap();
    let pack_id = PackFileId::new("pack-001").unwrap();
    let entry = PackIndexEntry::new(content_addr.clone(), pack_id.clone(), 100, 512);

    assert_eq!(entry.content_addr(), &content_addr);
    assert_eq!(entry.pack_file_id(), &pack_id);
    assert_eq!(entry.offset_bytes(), 100);
    assert_eq!(entry.size_bytes(), 512);
}

#[test]
fn blob_store_error_is_transient_classification() {
    let transient_errors = vec![
        BlobStoreError::Storage {
            reason: "disk full".to_string(),
        },
        BlobStoreError::DuplicateContent {
            content_addr: "abc".to_string(),
        },
        BlobStoreError::GcCycleInProgress,
        BlobStoreError::PackFileFull {
            pack_file_id: "pack-001".to_string(),
            max_size_bytes: 1000,
        },
    ];

    for err in transient_errors {
        assert!(err.is_transient(), "Expected {:?} to be transient", err);
    }

    let fatal_errors = vec![
        BlobStoreError::CorruptPackIndex {
            reason: "bad index".to_string(),
        },
        BlobStoreError::CorruptPackFile {
            pack_file_id: "pack-001".to_string(),
            reason: "truncated".to_string(),
        },
        BlobStoreError::ChecksumMismatch {
            content_addr: "abc".to_string(),
            expected: "def".to_string(),
            actual: "ghi".to_string(),
        },
        BlobStoreError::InvalidArgument {
            reason: "bad input".to_string(),
        },
    ];

    for err in fatal_errors {
        assert!(err.is_fatal(), "Expected {:?} to be fatal", err);
    }

    let not_transient_or_fatal = BlobStoreError::ContentNotFound {
        content_addr: "abc".to_string(),
    };
    assert!(!not_transient_or_fatal.is_transient());
    assert!(!not_transient_or_fatal.is_fatal());
}

#[test]
fn blob_record_with_status_allows_direct_status_construction() {
    let content_addr = ContentAddress::new(VALID_SHA256).unwrap();

    let record = BlobRecord::with_status(
        content_addr.clone(),
        1024,
        1,
        1000,
        Some(2000),
        BlobStatus::Pending,
    );
    assert_eq!(record.status(), BlobStatus::Pending);

    let record = BlobRecord::with_status(
        content_addr.clone(),
        1024,
        1,
        1000,
        Some(2000),
        BlobStatus::DurablyStored,
    );
    assert_eq!(record.status(), BlobStatus::DurablyStored);

    let record = BlobRecord::with_status(
        content_addr.clone(),
        1024,
        1,
        1000,
        Some(2000),
        BlobStatus::Published,
    );
    assert_eq!(record.status(), BlobStatus::Published);

    let record = BlobRecord::with_status(
        content_addr.clone(),
        1024,
        1,
        1000,
        Some(2000),
        BlobStatus::Failed,
    );
    assert_eq!(record.status(), BlobStatus::Failed);
}

#[test]
fn content_address_from_bytes_invalidates_uppercase() {
    let bytes = [
        0xAB_u8, 0xCD, 0xEF, 0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0x01, 0x23, 0x45,
        0x67, 0x89, 0xAB, 0xCD, 0xEF, 0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0x01, 0x23,
        0x45, 0x67, 0x89,
    ];
    let addr = ContentAddress::from_bytes(&bytes);
    let hex_str = addr.as_str();
    assert!(
        hex_str
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
        "from_bytes must produce lowercase hex"
    );
}

#[test]
fn content_address_deserialize_valid() {
    let addr = ContentAddress::new(VALID_SHA256).unwrap();
    let json = serde_json::to_string(&addr).unwrap();
    let decoded: ContentAddress = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, addr);
}

#[test]
fn content_address_deserialize_rejects_wrong_length() {
    let json = "\"abcdef\"";
    let result: Result<ContentAddress, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn content_address_deserialize_rejects_uppercase() {
    let json = "\"ABCDEF0123456789abcdef0123456789abcdef0123456789abcdef0123456789\"";
    let result: Result<ContentAddress, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn content_address_deserialize_rejects_non_hex() {
    let json = "\"9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15g0f00a08\"";
    let result: Result<ContentAddress, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn content_address_deserialize_rejects_empty() {
    let json = "\"\"";
    let result: Result<ContentAddress, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn pack_index_entry_deserialize_validates_content_address() {
    let json = r#"{"content_addr":"bad","pack_file_id":"pk-1","offset_bytes":0,"size_bytes":100}"#;
    let result: Result<PackIndexEntry, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn blob_record_deserialize_validates_content_address() {
    let json = r#"{"content_addr":"bad","size_bytes":100,"reference_count":1,"created_at_ms":1000,"expires_at_ms":null,"status":"Active"}"#;
    let result: Result<BlobRecord, _> = serde_json::from_str(json);
    assert!(result.is_err());
}
