use vo_storage::blob_store::BlobStoreError;

#[test]
fn red_queen_blob_store_error_display_all_variants() {
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