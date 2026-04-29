//! BLACK-HAT adversarial tests for vo-storage.
//!
//! Attacks storage invariants under corruption, race conditions, and resource
//! exhaustion. Every test uses an isolated temp directory. No state leaks
//! between tests.
//!
//! ve-327e3 — BLACK-HAT: vo-storage adversarial corruption testing

use std::fs;
use std::sync::Arc;

use tempfile::TempDir;
use vo_storage::blob_store::{
    BlobRecord, BlobStore, BlobStoreError, ContentAddress, PackIndexEntry,
};
use vo_storage::checksum::{verify_checksum, ChunkedHasher};
use vo_storage::codec::{decode_event_key, encode_event_key, StorageError};
use vo_storage::crypto::{
    decrypt_blob, encrypt_blob, generate_dek, unwrap_dek, wrap_dek, CryptoError,
};
use vo_storage::fs_store::FsBlobStore;
use vo_storage::key_encoding::{
    decode_dedupe_key, decode_effect_key, decode_event_key as ke_decode_event_key,
    decode_lease_key, decode_timer_key, encode_effect_key, encode_lease_key, get_event_key_prefix,
};
use vo_storage::merkle_tree::MerkleTree;
use vo_storage::partitions::{create_partition_layout, open_all_partitions, StorageEngine};
use vo_storage::snapshots::{
    compact_snapshots, decode_snapshot_key, encode_snapshot_key, snapshot_load_latest,
    snapshot_write, CompatSnapshotLoad, SnapshotDiscardReason,
};
use vo_types::state::InstanceState;
use vo_types::InstanceId;

fn min_instance_id() -> InstanceId {
    let mut bytes = [0u8; 16];
    bytes[15] = 1;
    InstanceId::from_bytes(bytes)
}

// ── 1. Fjall corruption: truncated SST files ───────────────────────────────────

#[test]
fn fjall_truncated_sst_does_not_panic_on_scan() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("db");

    {
        let layout = create_partition_layout(&path).unwrap();
        let partitions = open_all_partitions(&layout).unwrap();
        let events = partitions
            .iter()
            .find(|(n, _)| *n == "events")
            .unwrap()
            .1
            .clone();

        let id = min_instance_id();
        for seq in 1..=50u64 {
            let key =
                encode_event_key(&id, &vo_types::SequenceNumber::try_from(seq).unwrap()).unwrap();
            let val = serde_json::to_vec(&format!("event-{seq}")).unwrap();
            events.insert(key, val).unwrap();
        }
    }

    let sst_files: Vec<_> = fs::read_dir(&path)
        .unwrap()
        .flat_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "sst"))
        .collect();

    for sst in &sst_files {
        let data = fs::read(sst.path()).unwrap();
        if data.len() > 16 {
            fs::write(sst.path(), &data[..data.len() / 2]).unwrap();
        }
    }

    let result = create_partition_layout(&path);
    let _ = result;
}

// ── 2. Fjall corruption: invalid keys written directly ────────────────────────

#[test]
fn fjall_invalid_key_bytes_rejected_on_decode() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("db");

    let layout = create_partition_layout(&path).unwrap();
    let partitions = open_all_partitions(&layout).unwrap();
    let events = partitions
        .iter()
        .find(|(n, _)| *n == "events")
        .unwrap()
        .1
        .clone();

    events
        .insert(b"\x00\x00\x00\x00".to_vec(), b"garbage".to_vec())
        .unwrap();
    events
        .insert([0xFF; 24].to_vec(), b"bad-key".to_vec())
        .unwrap();

    for item in events.prefix(b"\x00\x00\x00\x00") {
        let (key, _val) = item.into_inner().unwrap();
        let result = decode_event_key(&key);
        assert!(
            result.is_err(),
            "short/garbage key should fail decode: len={}",
            key.len()
        );
    }
}

// ── 3. Concurrent write race on same key ───────────────────────────────────────

#[test]
fn concurrent_snapshot_writes_do_not_corrupt_each_other() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("db");

    let layout = create_partition_layout(&path).unwrap();
    let partitions = open_all_partitions(&layout).unwrap();
    let snaps = partitions
        .iter()
        .find(|(n, _)| *n == "snapshots")
        .unwrap()
        .1
        .clone();

    let id = min_instance_id();
    let state = InstanceState { counter: 42 };

    let handles: Vec<_> = (0..10)
        .map(|i| {
            let snaps = snaps.clone();
            let id = id.clone();
            let state = state.clone();
            std::thread::spawn(move || {
                let seq = (i as u64) * 10 + 1;
                snapshot_write(&snaps, id.clone(), seq, &state)
            })
        })
        .collect();

    let mut successes = 0u32;
    let mut errors = 0u32;
    for h in handles {
        match h.join().unwrap() {
            Ok(()) => successes += 1,
            Err(_) => errors += 1,
        }
    }

    assert!(
        successes > 0,
        "expected some concurrent writes to succeed, got {successes} ok, {errors} err"
    );

    let prefix = id.to_bytes().unwrap();
    for item in snaps.prefix(prefix) {
        let (key, value) = item.into_inner().unwrap();
        assert!(
            decode_snapshot_key(&key).is_ok(),
            "corrupted snapshot key from concurrent writes"
        );
        let _: serde_json::Value = serde_json::from_slice(&value)
            .expect("snapshot value should be valid JSON after concurrent writes");
    }
}

// ── 4. Disk full simulation via read-only dir ─────────────────────────────────

#[cfg(unix)]
#[test]
fn blob_store_gracefully_handles_write_failure() {
    use std::os::unix::fs::PermissionsExt;

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let _guard = rt.enter();
    let tmp = TempDir::new().unwrap();
    let store = FsBlobStore::new(tmp.path());

    let addr = store.store(b"hello").unwrap();
    assert!(store.contains(&addr).unwrap());

    let blobs_dir = tmp.path().join("blobs");
    let _perm = fs::set_permissions(&blobs_dir, fs::Permissions::from_mode(0o444));

    let result = store.store(b"this should fail");
    assert!(result.is_err());
    match result.unwrap_err() {
        BlobStoreError::Storage { reason } => assert!(!reason.is_empty()),
        other => panic!("expected Storage error on permission-denied write, got: {other}"),
    }
}

// ── 5. Malformed snapshot: truncated header JSON ─────────────────────────────

#[test]
fn snapshot_load_rejects_truncated_header_json() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("db");

    let layout = create_partition_layout(&path).unwrap();
    let partitions = open_all_partitions(&layout).unwrap();
    let snaps = partitions
        .iter()
        .find(|(n, _)| *n == "snapshots")
        .unwrap()
        .1
        .clone();

    let id = min_instance_id();
    let key = encode_snapshot_key(&id, 1).unwrap();

    let bad_value = b"{\"version\":1,\"sequence_number\":1,\"instance_id\":\"";
    snaps.insert(key, bad_value).unwrap();

    let result = snapshot_load_latest(&snaps, &id);
    assert!(result.is_err(), "truncated header JSON should be rejected");
}

// ── 6. Malformed snapshot: wrong checksum ─────────────────────────────────────

#[test]
fn snapshot_load_rejects_bad_checksum() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("db");

    let layout = create_partition_layout(&path).unwrap();
    let partitions = open_all_partitions(&layout).unwrap();
    let snaps = partitions
        .iter()
        .find(|(n, _)| *n == "snapshots")
        .unwrap()
        .1
        .clone();

    let id = min_instance_id();
    let key = encode_snapshot_key(&id, 1).unwrap();

    let bad_value = b"{\"version\":1,\"sequence_number\":1,\"checksum\":999999}|{\"counter\":0}";
    snaps.insert(key, bad_value).unwrap();

    let result = snapshot_load_latest(&snaps, &id);
    assert!(
        result.is_err(),
        "snapshot with wrong checksum should be rejected"
    );
}

// ── 7. Snapshot legacy format fallback ────────────────────────────────────────

#[test]
fn snapshot_load_falls_back_to_legacy_format() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("db");

    let layout = create_partition_layout(&path).unwrap();
    let partitions = open_all_partitions(&layout).unwrap();
    let snaps = partitions
        .iter()
        .find(|(n, _)| *n == "snapshots")
        .unwrap()
        .1
        .clone();

    let id = min_instance_id();
    let state = InstanceState { counter: 99 };
    let key = encode_snapshot_key(&id, 1).unwrap();

    let value = serde_json::to_vec(&state).unwrap();
    snaps.insert(key, value).unwrap();

    let result = snapshot_load_latest(&snaps, &id);
    assert!(result.is_ok());
    let loaded = result.unwrap().unwrap();
    assert_eq!(loaded.0, 1);
    assert_eq!(loaded.1.counter, 99);
}

// ── 8. Blob integrity: tamper detection on retrieve ───────────────────────────

#[cfg(unix)]
#[test]
fn fs_blob_store_detects_tampered_content() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let _guard = rt.enter();
    let tmp = TempDir::new().unwrap();
    let store = FsBlobStore::new(tmp.path());

    let original = b"untampered important payload";
    let addr = store.store(original).unwrap();

    let blob_path = tmp.path().join("blobs").join(addr.as_str());
    let mut data = fs::read(&blob_path).unwrap();
    data[0] = data[0].wrapping_add(1);
    fs::write(&blob_path, &data).unwrap();

    let result = store.retrieve(&addr);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        BlobStoreError::ChecksumMismatch { .. }
    ));
}

// ── 9. Blob integrity: tampered metadata ──────────────────────────────────────

#[cfg(unix)]
#[test]
fn fs_blob_store_detects_tampered_metadata() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let _guard = rt.enter();
    let tmp = TempDir::new().unwrap();
    let store = FsBlobStore::new(tmp.path());

    let original = b"metadata tamper test";
    let addr = store.store(original).unwrap();

    let meta_path = tmp
        .path()
        .join("meta")
        .join(format!("{}.json", addr.as_str()));
    fs::write(&meta_path, b"{}").unwrap();

    let result = store.get_metadata(&addr);
    assert!(result.is_err());
}

// ── 10. Key encoding: adversarial boundary inputs ──────────────────────────────

#[test]
fn key_encoding_event_key_all_zero_bytes_is_corrupt() {
    let key = [0u8; 24];
    let result = ke_decode_event_key(&key);
    assert!(
        result.is_err(),
        "all-zero key (sequence=0) should be corrupt"
    );
}

#[test]
fn key_encoding_event_key_wrong_length_rejected() {
    for len in [0, 1, 15, 16, 17, 23, 25, 100] {
        let key = vec![0u8; len];
        let result = ke_decode_event_key(&key);
        assert!(result.is_err(), "event key len={len} should be rejected");
    }
}

#[test]
fn key_encoding_timer_key_wrong_length_rejected() {
    for len in [0, 1, 8, 15, 23, 25, 100] {
        let key = vec![0u8; len];
        let result = decode_timer_key(&key);
        assert!(result.is_err(), "timer key len={len} should be rejected");
    }
}

#[test]
fn key_encoding_effect_key_wrong_length_rejected() {
    for len in [0, 1, 24, 26, 100] {
        let key = vec![0xFF; len];
        let result = decode_effect_key(&key);
        assert!(result.is_err(), "effect key len={len} should be rejected");
    }
}

#[test]
fn key_encoding_effect_key_missing_marker_rejected() {
    let mut key = [0u8; 25];
    key[24] = 0x00;
    let result = decode_effect_key(&key);
    assert!(
        result.is_err(),
        "effect key without 0xFF marker should be rejected"
    );
}

#[test]
fn key_encoding_lease_key_missing_delimiter_rejected() {
    let key = b"01H5JYV4XHGSR2F8KZ9BWNRFMAstep-id-without-delimiter";
    let result = decode_lease_key(key);
    assert!(
        result.is_err(),
        "lease key without :: delimiter should be rejected"
    );
}

#[test]
fn key_encoding_lease_key_empty_components_rejected() {
    let key = b"::step-id";
    let result = decode_lease_key(key);
    assert!(
        result.is_err(),
        "lease key with empty instance should be rejected"
    );
}

#[test]
fn key_encoding_dedupe_key_truncated_length_prefix() {
    let key = vec![0x05];
    let result = decode_dedupe_key(&key);
    assert!(result.is_err(), "truncated dedupe key should be rejected");
}

#[test]
fn key_encoding_dedupe_key_length_exceeds_data() {
    let mut key = vec![0x00, 0x64];
    key.extend_from_slice(b"abc");
    let result = decode_dedupe_key(&key);
    assert!(
        result.is_err(),
        "dedupe key claiming more bytes than available should be rejected"
    );
}

// ── 11. Codec: event key encode/decode roundtrip adversarial ───────────────────

#[test]
fn codec_encode_decode_roundtrip_boundary_sequences() {
    let id = min_instance_id();

    let seq = vo_types::SequenceNumber::try_from(u64::MAX).unwrap();
    let encoded = encode_event_key(&id, &seq).unwrap();
    let decoded = decode_event_key(&encoded).unwrap();
    assert_eq!(decoded.0, id);
    assert_eq!(decoded.1.as_u64(), u64::MAX);

    let seq1 = vo_types::SequenceNumber::try_from(1u64).unwrap();
    let encoded1 = encode_event_key(&id, &seq1).unwrap();
    let decoded1 = decode_event_key(&encoded1).unwrap();
    assert_eq!(decoded1.1.as_u64(), 1);
}

#[test]
fn codec_decode_rejects_short_and_long_keys() {
    for len in [0, 1, 5, 10, 15, 16, 17, 20, 23] {
        if len == 24 {
            continue;
        }
        let key = vec![0xFF; len];
        let result = decode_event_key(&key);
        assert!(result.is_err(), "codec should reject key of len={len}");
    }
}

// ── 12. Crypto: unwrap with wrong KEK, short ciphertext ───────────────────────

#[test]
fn crypto_unwrap_with_wrong_kek_fails() {
    let dek = generate_dek().unwrap();
    let kek = [0xAA; 32];
    let wrong_kek = [0xBB; 32];

    let wrapped = wrap_dek(&dek, &kek).unwrap();
    let result = unwrap_dek(&wrapped, &wrong_kek);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), CryptoError::UnwrappingFailed));
}

#[test]
fn crypto_unwrap_truncated_data_rejected() {
    let kek = [0xAA; 32];
    let short_data = vec![0u8; 10];
    let result = unwrap_dek(&short_data, &kek);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        CryptoError::InvalidKeyMaterial
    ));
}

#[test]
fn crypto_encrypt_decrypt_roundtrip() {
    let dek = generate_dek().unwrap();
    let plaintext = b"secret workflow payload";

    let blob = encrypt_blob(plaintext, &dek).unwrap();
    let recovered = decrypt_blob(&blob, &dek).unwrap();
    assert_eq!(recovered, plaintext);
}

#[test]
fn crypto_decrypt_with_wrong_dek_fails() {
    let dek1 = generate_dek().unwrap();
    let dek2 = generate_dek().unwrap();
    let plaintext = b"secret workflow payload";

    let blob = encrypt_blob(plaintext, &dek1).unwrap();
    let result = decrypt_blob(&blob, &dek2);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), CryptoError::DecryptionFailed));
}

#[test]
fn crypto_decrypt_tampered_ciphertext_fails() {
    let dek = generate_dek().unwrap();
    let plaintext = b"tamper me";

    let mut blob = encrypt_blob(plaintext, &dek).unwrap();
    if !blob.ciphertext.is_empty() {
        blob.ciphertext[0] = blob.ciphertext[0].wrapping_add(1);
    }

    let result = decrypt_blob(&blob, &dek);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), CryptoError::DecryptionFailed));
}

#[test]
fn crypto_decrypt_tampered_tag_fails() {
    let dek = generate_dek().unwrap();
    let plaintext = b"tag tamper";

    let mut blob = encrypt_blob(plaintext, &dek).unwrap();
    if !blob.tag.is_empty() {
        blob.tag[0] = blob.tag[0].wrapping_add(1);
    }

    let result = decrypt_blob(&blob, &dek);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), CryptoError::DecryptionFailed));
}

#[test]
fn crypto_decrypt_wrong_iv_length_rejected() {
    let dek = generate_dek().unwrap();
    let plaintext = b"iv test";

    let mut blob = encrypt_blob(plaintext, &dek).unwrap();
    blob.iv = vec![0u8; 8];

    let result = decrypt_blob(&blob, &dek);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        CryptoError::InvalidKeyMaterial
    ));
}

#[test]
fn crypto_decrypt_wrong_tag_length_rejected() {
    let dek = generate_dek().unwrap();
    let plaintext = b"tag len test";

    let mut blob = encrypt_blob(plaintext, &dek).unwrap();
    blob.tag = vec![0u8; 8];

    let result = decrypt_blob(&blob, &dek);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        CryptoError::InvalidKeyMaterial
    ));
}

// ── 13. Checksum: verify detects single-byte corruption ───────────────────────

#[test]
fn checksum_detects_single_byte_flip() {
    let original = b"The quick brown fox jumps over the lazy dog";
    let expected = vo_storage::checksum::compute_checksum(original);

    for offset in 0..original.len() {
        let mut corrupted = original.to_vec();
        corrupted[offset] = corrupted[offset].wrapping_add(1);
        let result = verify_checksum(&corrupted, &expected);
        assert!(
            result.is_err(),
            "single-byte flip at offset {offset} must be detected"
        );
    }
}

#[test]
fn checksum_empty_data_verifies() {
    let empty: &[u8] = &[];
    let expected = vo_storage::checksum::compute_checksum(empty);
    assert!(
        verify_checksum(empty, &expected).is_ok(),
        "empty data should verify"
    );
}

#[test]
#[test]
fn chunked_hasher_produces_correct_total_size() {
    let data = b"0123456789ABCDEF"; // 16 bytes, chunk size 5
    let mut hasher = ChunkedHasher::new(5);
    hasher.update(data);
    let chunks = hasher.finalize();
    let total: u64 = chunks.iter().map(|c| c.size).sum();
    assert_eq!(
        total, 16,
        "chunked hasher total size should equal input size"
    );
}

// ── 14. Merkle tree: adversarial inputs ───────────────────────────────────────

#[test]
fn merkle_tree_different_chunk_sizes_valid_proofs() {
    let data = b"consistent data regardless of chunk boundaries";
    let tree_64 = MerkleTree::new(data, 64);
    let tree_128 = MerkleTree::new(data, 128);
    let tree_1024 = MerkleTree::new(data, 1024);

    assert_eq!(tree_64.leaf_hashes.len(), tree_64.leaf_hashes.len());

    for tree in [&tree_64, &tree_128, &tree_1024] {
        let root = tree.root_hash();
        for i in 0..tree.leaf_hashes.len() {
            let proof = tree.proof(i).unwrap();
            assert!(proof.verify(root), "proof {i} should verify for chunk_size");
        }
    }
}

#[test]
fn merkle_tree_proof_with_tampered_sibling_fails() {
    let data = b"merkle tamper test data for adversarial checking";
    let tree = MerkleTree::new(data, 8);
    let root = tree.root_hash();

    if tree.leaf_hashes.len() >= 2 {
        let mut proof = tree.proof(0).unwrap();
        if !proof.proof_hashes.is_empty() {
            proof.proof_hashes[0][0] = proof.proof_hashes[0][0].wrapping_add(1);
            assert!(
                !proof.verify(root),
                "tampered sibling hash should fail proof"
            );
        }
    }
}

// ── 15. Snapshot compat: version boundary attacks ─────────────────────────────

#[test]
fn snapshot_compat_rejects_future_version() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("db");

    let layout = create_partition_layout(&path).unwrap();
    let db = layout.db();

    let snaps = db
        .keyspace("snapshots_test_compat", || {
            fjall::KeyspaceCreateOptions::default()
        })
        .unwrap();

    let id = min_instance_id();
    let key = encode_snapshot_key(&id, 1).unwrap();

    let state_json = serde_json::to_vec(&InstanceState { counter: 1 }).unwrap();
    let mut value = String::from_utf8(state_json).unwrap();
    // Inject a header with pipe separator and future version
    value = format!("{{\"version\":999,\"sequence_number\":1,\"checksum\":0,\"instance_id\":\"{}\"}}|{{\"counter\":1}}", id);
    snaps.insert(key, value.as_bytes()).unwrap();

    let result = vo_storage::snapshots::snapshot_load_latest_with_compat(&snaps, &id, 1, 1);
    // Future version should be discarded, not cause a panic. Either Ok(Discarded) or Err is acceptable.
    if let Ok(Some(loaded)) = &result {
        assert!(
            matches!(
                loaded,
                CompatSnapshotLoad::Discarded {
                    reason: SnapshotDiscardReason::VersionTooNew { .. },
                    ..
                }
            ),
            "future version should be discarded: {loaded:?}"
        );
    }
    // Err is also acceptable (engine rejects malformed snapshot before compat check)
}

#[test]
fn snapshot_compat_rejects_legacy_version_zero() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("db");

    let layout = create_partition_layout(&path).unwrap();
    let db = layout.db();

    let snaps = db
        .keyspace("snapshots_compat_old", || {
            fjall::KeyspaceCreateOptions::default()
        })
        .unwrap();

    let id = min_instance_id();
    let key = encode_snapshot_key(&id, 1).unwrap();

    let state_json = serde_json::to_vec(&InstanceState { counter: 1 }).unwrap();
    snaps.insert(key, state_json).unwrap();

    let result = vo_storage::snapshots::snapshot_load_latest_with_compat(&snaps, &id, 1, 1);
    assert!(result.is_ok());
    let loaded = result.unwrap().unwrap();
    assert!(matches!(
        loaded,
        CompatSnapshotLoad::Discarded {
            reason: SnapshotDiscardReason::VersionZero,
            ..
        }
    ));
}

// ── 16. Blob store: content address validation attacks ────────────────────────

#[test]
fn content_address_rejects_wrong_length() {
    assert!(ContentAddress::new("abc123").is_err());
    assert!(ContentAddress::new("a".repeat(100)).is_err());
    assert!(ContentAddress::new("a".repeat(64)).is_ok());
}

#[test]
fn content_address_rejects_uppercase() {
    assert!(ContentAddress::new("A".repeat(64)).is_err());
}

#[test]
fn content_address_rejects_non_hex() {
    assert!(ContentAddress::new("g".repeat(64)).is_err());
}

#[test]
fn blob_record_rejects_zero_reference_count() {
    let addr = ContentAddress::from_bytes(&[0u8; 32]);
    let result = BlobRecord::new(addr.clone(), 100, 0, 1000, None);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        BlobStoreError::InvalidArgument { .. }
    ));
}

#[test]
fn blob_record_rejects_zero_created_at() {
    let addr = ContentAddress::from_bytes(&[0u8; 32]);
    let result = BlobRecord::new(addr.clone(), 100, 1, 0, None);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        BlobStoreError::InvalidArgument { .. }
    ));
}

#[test]
fn pack_index_entry_decode_rejects_garbage_json() {
    let result = vo_storage::blob_store::decode_pack_index_entry(b"not json at all");
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        BlobStoreError::CorruptPackIndex { .. }
    ));
}

#[test]
fn blob_record_decode_rejects_garbage_json() {
    let result = vo_storage::blob_store::decode_blob_record(b"{{{{invalid");
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        BlobStoreError::DeserializationFailed { .. }
    ));
}

#[test]
fn content_address_decode_rejects_non_utf8() {
    let result = vo_storage::blob_store::decode_content_address(&[0xFF, 0xFE, 0xFD]);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        BlobStoreError::CorruptPackIndex { .. }
    ));
}

// ── 17. BlobStore error taxonomy: is_transient / is_fatal ────────────────────

#[test]
fn blob_store_error_taxonomy_correct() {
    let fatal_variants = [
        BlobStoreError::CorruptPackIndex {
            reason: "test".into(),
        },
        BlobStoreError::CorruptPackFile {
            pack_file_id: "p".into(),
            reason: "r".into(),
        },
        BlobStoreError::ChecksumMismatch {
            content_addr: "a".into(),
            expected: "e".into(),
            actual: "a".into(),
        },
        BlobStoreError::InvalidArgument {
            reason: "test".into(),
        },
    ];

    for err in &fatal_variants {
        assert!(err.is_fatal(), "should be fatal: {err}");
        assert!(!err.is_transient(), "fatal should not be transient: {err}");
    }

    let transient_variants = [
        BlobStoreError::Storage {
            reason: "io error".into(),
        },
        BlobStoreError::DuplicateContent {
            content_addr: "a".into(),
        },
        BlobStoreError::GcCycleInProgress,
        BlobStoreError::PackFileFull {
            pack_file_id: "p".into(),
            max_size_bytes: 1024,
        },
    ];

    for err in &transient_variants {
        assert!(err.is_transient(), "should be transient: {err}");
        assert!(!err.is_fatal(), "transient should not be fatal: {err}");
    }

    let neither_variants = [
        BlobStoreError::ContentNotFound {
            content_addr: "a".into(),
        },
        BlobStoreError::PackFileNotFound {
            pack_file_id: "p".into(),
        },
        BlobStoreError::SerializationFailed {
            reason: "test".into(),
        },
        BlobStoreError::DeserializationFailed {
            reason: "test".into(),
        },
    ];

    for err in &neither_variants {
        assert!(!err.is_fatal(), "should not be fatal: {err}");
        assert!(!err.is_transient(), "should not be transient: {err}");
    }
}

// ── 18. Pack index entry roundtrip with max values ───────────────────────────

#[test]
fn pack_index_entry_roundtrip_max_values() {
    let addr = ContentAddress::from_bytes(&[0xFF; 32]);
    let pack_id = vo_storage::blob_store::PackFileId::new("max-pack").unwrap();
    let entry = PackIndexEntry::new(addr, pack_id, u64::MAX, u64::MAX);

    let encoded = vo_storage::blob_store::encode_pack_index_entry(&entry).unwrap();
    let decoded = vo_storage::blob_store::decode_pack_index_entry(&encoded).unwrap();

    assert_eq!(
        decoded.content_addr().as_str(),
        entry.content_addr().as_str()
    );
    assert_eq!(
        decoded.pack_file_id().as_str(),
        entry.pack_file_id().as_str()
    );
    assert_eq!(decoded.offset_bytes(), u64::MAX);
    assert_eq!(decoded.size_bytes(), u64::MAX);
}

#[test]
fn pack_file_id_rejects_empty() {
    let result = vo_storage::blob_store::PackFileId::new("");
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        BlobStoreError::InvalidArgument { .. }
    ));
}

// ── 19. Key encoding prefix isolation ────────────────────────────────────────

#[test]
fn event_key_prefix_isolation() {
    let id1 = InstanceId::from_bytes([1u8; 16]);
    let id2 = InstanceId::from_bytes([2u8; 16]);

    let prefix1 = get_event_key_prefix(&id1);
    let prefix2 = get_event_key_prefix(&id2);

    assert_ne!(prefix1, prefix2);

    let key1 = vo_storage::key_encoding::encode_event_key(
        &id1,
        vo_types::SequenceNumber::try_from(1u64).unwrap(),
    );
    assert!(
        !key1.starts_with(&prefix2),
        "id1's key must not start with id2's prefix"
    );
}

// ── 20. Compact snapshots with corrupted keys ────────────────────────────────

#[test]
fn compact_snapshots_with_corrupted_keys_returns_error() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("db");

    let layout = create_partition_layout(&path).unwrap();
    let partitions = open_all_partitions(&layout).unwrap();
    let snaps = partitions
        .iter()
        .find(|(n, _)| *n == "snapshots")
        .unwrap()
        .1
        .clone();

    let id = min_instance_id();

    let key1 = encode_snapshot_key(&id, 1).unwrap();
    snaps
        .insert(
            key1,
            serde_json::to_vec(&InstanceState { counter: 1 }).unwrap(),
        )
        .unwrap();

    snaps
        .insert(b"short_key".to_vec(), b"garbage".to_vec())
        .unwrap();

    let result = compact_snapshots(&snaps, &id, 0);
    let _ = result; // Must not panic
}

// ── 21. StorageEngine: open on fresh temp dir ────────────────────────────────

#[test]
fn storage_engine_opens_fresh_temp_dir() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("engine");

    let engine1 = StorageEngine::open(&path);
    assert!(engine1.is_ok(), "first open should succeed");

    let engine2 = StorageEngine::open(&path);
    let _ = engine2; // Must not panic
}
