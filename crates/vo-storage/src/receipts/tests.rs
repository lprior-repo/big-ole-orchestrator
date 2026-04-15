use super::in_memory_receipt_store::InMemoryReceiptStore;
use super::*;
use vo_types::EffectKind;

fn sample_instance_id() -> InstanceId {
    InstanceId::from_bytes([1u8; 16])
}

fn make_receipt(effect_id: &str, instance_id: &InstanceId) -> ExecutionReceipt {
    let effect_id_with_prefix = format!("{instance_id}::{effect_id}");
    ExecutionReceipt::new(
        effect_id_with_prefix,
        instance_id.to_string(),
        EffectKind::HttpCall,
        1000,
        "Success".to_string(),
    )
    .unwrap()
}

// ========================================================================
// ExecutionReceipt construction
// ========================================================================

#[test]
fn execution_receipt_constructs_with_valid_fields() {
    let receipt = ExecutionReceipt::new(
        "fx-1".to_string(),
        "inst-1".to_string(),
        EffectKind::HttpCall,
        1000,
        "Success".to_string(),
    );
    assert!(receipt.is_ok());
    let r = receipt.unwrap();
    assert_eq!(r.effect_id(), "fx-1");
    assert_eq!(r.instance_id(), "inst-1");
    assert_eq!(r.kind(), EffectKind::HttpCall);
    assert_eq!(r.committed_at_ms(), 1000);
    assert_eq!(r.connector_result(), "Success");
}

#[test]
fn execution_receipt_rejects_empty_effect_id() {
    let result = ExecutionReceipt::new(
        "".to_string(),
        "inst-1".to_string(),
        EffectKind::HttpCall,
        1000,
        "Success".to_string(),
    );
    assert_eq!(result, Err(ReceiptStoreError::InvalidArgument));
}

#[test]
fn execution_receipt_rejects_empty_instance_id() {
    let result = ExecutionReceipt::new(
        "fx-1".to_string(),
        "".to_string(),
        EffectKind::HttpCall,
        1000,
        "Success".to_string(),
    );
    assert_eq!(result, Err(ReceiptStoreError::InvalidArgument));
}

#[test]
fn execution_receipt_serde_roundtrip() {
    let receipt = make_receipt("fx-42", &sample_instance_id());
    let json = serde_json::to_vec(&receipt).unwrap();
    let recovered: ExecutionReceipt = serde_json::from_slice(&json).unwrap();
    assert_eq!(recovered, receipt);
}

// ========================================================================
// Calc layer — key encoding/decoding
// ========================================================================

#[test]
fn encode_decode_receipt_key_roundtrip() {
    let key = "instance-abc::intent-xyz";
    let encoded = encode_receipt_key(key);
    let decoded = decode_receipt_key(&encoded).unwrap();
    assert_eq!(decoded, key);
}

#[test]
fn decode_receipt_key_rejects_empty() {
    let result = decode_receipt_key(&[]);
    assert!(result.is_err());
}

#[test]
fn decode_receipt_key_rejects_invalid_utf8() {
    let bytes = vec![0xFF, 0xFE];
    let result = decode_receipt_key(&bytes);
    assert!(result.is_err());
}

// ========================================================================
// Calc layer — receipt encoding/decoding
// ========================================================================

#[test]
fn encode_decode_receipt_roundtrip() {
    let receipt = make_receipt("fx-roundtrip", &sample_instance_id());
    let encoded = encode_receipt(&receipt).unwrap();
    let decoded = decode_receipt(&encoded).unwrap();
    assert_eq!(decoded, receipt);
}

// ========================================================================
// InMemoryReceiptStore — store and retrieve
// ========================================================================

#[test]
fn in_memory_store_receipt_and_retrieve() {
    let store = InMemoryReceiptStore::new();
    let id = sample_instance_id();
    let receipt = make_receipt("fx-1", &id);

    store.store_receipt(receipt.clone()).unwrap();
    let found = store.get_receipt(receipt.effect_id()).unwrap();

    assert!(found.is_some());
    assert_eq!(found.unwrap(), receipt);
}

#[test]
fn in_memory_store_receipt_is_idempotent() {
    let store = InMemoryReceiptStore::new();
    let id = sample_instance_id();
    let receipt = make_receipt("fx-2", &id);

    let first = store.store_receipt(receipt.clone()).unwrap();
    let second = store.store_receipt(receipt).unwrap();

    assert_eq!(first, ());
    assert_eq!(second, ());

    let all = store.list_by_instance(&id).unwrap();
    assert_eq!(all.len(), 1);
}

#[test]
fn in_memory_get_nonexistent_receipt_returns_none() {
    let store = InMemoryReceiptStore::new();
    let result = store.get_receipt("nonexistent");
    assert_eq!(result, Ok(None));
}

#[test]
fn in_memory_has_receipt_returns_true_for_existing() {
    let store = InMemoryReceiptStore::new();
    let id = sample_instance_id();
    store.store_receipt(make_receipt("fx-3", &id)).unwrap();

    assert_eq!(store.has_receipt(&format!("{id}::fx-3")), Ok(true));
}

#[test]
fn in_memory_has_receipt_returns_false_for_missing() {
    let store = InMemoryReceiptStore::new();
    assert_eq!(store.has_receipt("missing"), Ok(false));
}

#[test]
fn in_memory_list_by_instance_returns_only_matching_receipts() {
    let store = InMemoryReceiptStore::new();
    let id = sample_instance_id();
    let other_id = InstanceId::from_bytes([2u8; 16]);

    store.store_receipt(make_receipt("fx-a", &id)).unwrap();
    store.store_receipt(make_receipt("fx-b", &id)).unwrap();
    store
        .store_receipt(make_receipt("fx-c", &other_id))
        .unwrap();

    let results = store.list_by_instance(&id).unwrap();
    assert_eq!(results.len(), 2);
}

#[test]
fn in_memory_store_rejects_empty_effect_id() {
    let store = InMemoryReceiptStore::new();
    let receipt = ExecutionReceipt::new(
        "".to_string(),
        "inst-1".to_string(),
        EffectKind::HttpCall,
        1000,
        "Success".to_string(),
    );
    assert!(receipt.is_err());
}

#[test]
fn in_memory_get_rejects_empty_effect_id() {
    let store = InMemoryReceiptStore::new();
    let result = store.get_receipt("");
    assert_eq!(result, Err(ReceiptStoreError::InvalidArgument));
}

// ========================================================================
// ReceiptStoreError
// ========================================================================

#[test]
fn receipt_store_error_storage_displays_reason() {
    let err = ReceiptStoreError::Storage {
        reason: "disk full".to_string(),
    };
    assert!(err.to_string().contains("disk full"));
}

#[test]
fn receipt_store_error_codec_displays_reason() {
    let err = ReceiptStoreError::Codec {
        reason: "bad json".to_string(),
    };
    assert!(err.to_string().contains("bad json"));
}

#[test]
fn receipt_store_error_invalid_argument_displays() {
    let err = ReceiptStoreError::InvalidArgument;
    assert!(err.to_string().contains("invalid"));
}
