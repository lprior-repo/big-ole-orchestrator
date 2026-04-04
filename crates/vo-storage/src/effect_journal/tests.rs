#![allow(clippy::unwrap_used)]
use super::*;
use rstest::rstest;
use serde_json::json;
use std::collections::HashMap;

// Helper: valid InstanceId for tests
fn test_instance_id() -> InstanceId {
    InstanceId::from_bytes([1u8; 16])
}

// ========================================================================
// EffectId Construction
// ========================================================================

#[test]
fn effectid_constructs_when_instance_id_and_intent_id_valid() {
    let id = InstanceId::from_bytes([1u8; 16]);
    let result = EffectId::new(&id, "fx-123");
    assert!(result.is_ok());
    let eid = result.unwrap();
    assert!(eid.as_str().contains("fx-123"));
}

#[test]
fn effectid_rejects_when_intent_id_empty() {
    let id = InstanceId::from_bytes([1u8; 16]);
    let result = EffectId::new(&id, "");
    assert_eq!(result, Err(EffectJournalError::InvalidArgument));
}

#[test]
fn effectid_equality_and_hashing() {
    let id = InstanceId::from_bytes([1u8; 16]);
    let a = EffectId::new(&id, "fx-1").unwrap();
    let b = EffectId::new(&id, "fx-1").unwrap();
    assert_eq!(a, b);

    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h1 = DefaultHasher::new();
    a.hash(&mut h1);
    let mut h2 = DefaultHasher::new();
    b.hash(&mut h2);
    assert_eq!(h1.finish(), h2.finish());
}

#[test]
fn effectid_serde_roundtrip() {
    let id = InstanceId::from_bytes([1u8; 16]);
    let eid = EffectId::new(&id, "fx-456").unwrap();
    let json_str = serde_json::to_string(&eid).unwrap();
    let recovered: EffectId = serde_json::from_str(&json_str).unwrap();
    assert_eq!(recovered, eid);
}

// ========================================================================
// Error Display
// ========================================================================

#[test]
fn error_already_terminal_displays_effect_id_and_status() {
    let err = EffectJournalError::AlreadyTerminal {
        effect_id: "fx-1".to_string(),
        current_status: "Committed".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("fx-1"), "should contain effect_id");
    assert!(msg.contains("Committed"), "should contain status");
}

#[test]
fn error_not_found_displays_effect_id() {
    let err = EffectJournalError::NotFound {
        effect_id: "fx-999".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("fx-999"));
}

#[test]
fn error_storage_displays_reason() {
    let err = EffectJournalError::Storage {
        reason: "disk full".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("disk full"));
}

#[test]
fn error_codec_displays_reason() {
    let err = EffectJournalError::Codec {
        reason: "invalid JSON".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("invalid JSON"));
}

// ========================================================================
// Calc Layer — Key Encode/Decode
// ========================================================================

#[test]
fn encode_effect_key_produces_utf8_bytes() {
    let id = InstanceId::from_bytes([1u8; 16]);
    let eid = EffectId::new(&id, "instance::fx-123").unwrap();
    let bytes = encode_effect_key(&eid);
    assert_eq!(bytes, eid.as_str().as_bytes());
}

#[test]
fn decode_effect_key_recovers_effect_id() {
    let id = InstanceId::from_bytes([1u8; 16]);
    let eid = EffectId::new(&id, "test-key").unwrap();
    let bytes = encode_effect_key(&eid);
    let recovered = decode_effect_key(&bytes).unwrap();
    assert_eq!(recovered, eid);
}

#[test]
fn decode_effect_key_returns_error_for_invalid_utf8() {
    let bad_bytes: &[u8] = &[0xFF, 0xFE];
    let result = decode_effect_key(bad_bytes);
    assert!(matches!(result, Err(EffectJournalError::Codec { .. })));
}

#[test]
fn decode_effect_key_returns_error_for_empty_bytes() {
    let result = decode_effect_key(&[]);
    assert!(matches!(result, Err(EffectJournalError::Codec { .. })));
}

// ========================================================================
// Calc Layer — Record Encode/Decode
// ========================================================================

#[test]
fn encode_decode_effect_record_roundtrip() {
    let record = EffectRecord::new(
        "fx-roundtrip".to_string(),
        EffectKind::HttpCall,
        json!({"url": "https://example.com"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let bytes = encode_effect_record(&record).unwrap();
    let recovered = decode_effect_record(&bytes).unwrap();
    assert_eq!(recovered, record);
}

#[test]
fn decode_effect_record_returns_error_for_invalid_json() {
    let result = decode_effect_record(b"not-json");
    assert!(matches!(result, Err(EffectJournalError::Codec { .. })));
}

#[rstest]
#[case(EffectIntent::Prepared)]
#[case(EffectIntent::Committed)]
#[case(EffectIntent::RolledBack)]
fn encode_decode_record_roundtrip_for_all_statuses(#[case] status: EffectIntent) {
    let ts = vo_types::TimestampMs::parse("42").unwrap();
    let record = EffectRecord::new(
        "fx-status-test".to_string(),
        EffectKind::SqlQuery,
        json!({"q": "SELECT 1"}),
        status,
        Some(ts),
    )
    .unwrap();
    let bytes = encode_effect_record(&record).unwrap();
    let recovered = decode_effect_record(&bytes).unwrap();
    assert_eq!(recovered, record);
}

// ========================================================================
// Trait Integration — via MockEffectJournal
// ========================================================================

/// In-memory mock implementation of EffectJournal for testing.
struct MockEffectJournal {
    records: std::cell::RefCell<HashMap<String, EffectRecord>>,
}

impl MockEffectJournal {
    fn new() -> Self {
        Self {
            records: std::cell::RefCell::new(HashMap::new()),
        }
    }
}

impl EffectJournal for MockEffectJournal {
    fn prepare(
        &self,
        _instance_id: &InstanceId,
        record: EffectRecord,
    ) -> Result<EffectId, EffectJournalError> {
        let intent_id = record.intent_id().to_string();
        let effect_id = EffectId::new(_instance_id, &intent_id)?;
        let key = effect_id.as_str().to_string();

        // Idempotent: return existing if same intent_id
        if self.records.borrow().contains_key(&key) {
            return Ok(effect_id);
        }

        self.records.borrow_mut().insert(key, record);
        Ok(effect_id)
    }

    fn commit(&self, effect_id: &EffectId) -> Result<(), EffectJournalError> {
        let key = effect_id.as_str().to_string();
        let mut records = self.records.borrow_mut();
        let record = records
            .get_mut(&key)
            .ok_or_else(|| EffectJournalError::NotFound {
                effect_id: key.clone(),
            })?;

        match record.status() {
            EffectIntent::Committed | EffectIntent::RolledBack => {
                return Err(EffectJournalError::AlreadyTerminal {
                    effect_id: key,
                    current_status: format!("{:?}", record.status()),
                });
            }
            EffectIntent::Prepared => {
                let committed = EffectRecord::new(
                    record.intent_id().to_string(),
                    record.kind(),
                    record.params_json().clone(),
                    EffectIntent::Committed,
                    Some(vo_types::TimestampMs::parse("100").unwrap()),
                );
                if let Some(c) = committed {
                    *record = c;
                }
                Ok(())
            }
        }
    }

    fn rollback(&self, effect_id: &EffectId) -> Result<(), EffectJournalError> {
        let key = effect_id.as_str().to_string();
        let mut records = self.records.borrow_mut();
        let record = records
            .get_mut(&key)
            .ok_or_else(|| EffectJournalError::NotFound {
                effect_id: key.clone(),
            })?;

        match record.status() {
            EffectIntent::Committed | EffectIntent::RolledBack => {
                Err(EffectJournalError::AlreadyTerminal {
                    effect_id: key,
                    current_status: format!("{:?}", record.status()),
                })
            }
            EffectIntent::Prepared => {
                let rolled_back = EffectRecord::new(
                    record.intent_id().to_string(),
                    record.kind(),
                    record.params_json().clone(),
                    EffectIntent::RolledBack,
                    None,
                );
                if let Some(rb) = rolled_back {
                    *record = rb;
                }
                Ok(())
            }
        }
    }

    fn list_pending(
        &self,
        instance_id: &InstanceId,
    ) -> Result<Vec<EffectRecord>, EffectJournalError> {
        let records = self.records.borrow();
        let prefix = format!("{instance_id}::");
        Ok(records
            .iter()
            .filter(|(k, v)| k.starts_with(&prefix) && v.status() == EffectIntent::Prepared)
            .map(|(_, v)| v.clone())
            .collect())
    }
}

#[test]
fn prepare_returns_effect_id_for_new_intent() {
    let journal = MockEffectJournal::new();
    let id = test_instance_id();
    let record = EffectRecord::new(
        "fx-1".to_string(),
        EffectKind::HttpCall,
        json!({"url": "https://api.stripe.com"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let result = journal.prepare(&id, record);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().as_str(), format!("{id}::fx-1"));
}

#[test]
fn prepare_is_idempotent_for_same_intent_id() {
    let journal = MockEffectJournal::new();
    let id = test_instance_id();
    let record = EffectRecord::new(
        "fx-1".to_string(),
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let first = journal.prepare(&id, record.clone()).unwrap();
    let second = journal.prepare(&id, record).unwrap();
    assert_eq!(first, second);
}

#[test]
fn commit_transitions_prepared_to_committed() {
    let journal = MockEffectJournal::new();
    let id = test_instance_id();
    let record = EffectRecord::new(
        "fx-commit".to_string(),
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let eid = journal.prepare(&id, record).unwrap();
    let result = journal.commit(&eid);
    assert!(result.is_ok());
}

#[test]
fn rollback_transitions_prepared_to_rolledback() {
    let journal = MockEffectJournal::new();
    let id = test_instance_id();
    let record = EffectRecord::new(
        "fx-rollback".to_string(),
        EffectKind::SqlQuery,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let eid = journal.prepare(&id, record).unwrap();
    let result = journal.rollback(&eid);
    assert!(result.is_ok());
}

#[test]
fn commit_returns_already_terminal_for_committed_effect() {
    let journal = MockEffectJournal::new();
    let id = test_instance_id();
    let record = EffectRecord::new(
        "fx-double".to_string(),
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let eid = journal.prepare(&id, record).unwrap();
    journal.commit(&eid).unwrap();
    let result = journal.commit(&eid);
    assert!(matches!(
        result,
        Err(EffectJournalError::AlreadyTerminal { .. })
    ));
}

#[test]
fn rollback_returns_already_terminal_for_rolledback_effect() {
    let journal = MockEffectJournal::new();
    let id = test_instance_id();
    let record = EffectRecord::new(
        "fx-rb-double".to_string(),
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let eid = journal.prepare(&id, record).unwrap();
    journal.rollback(&eid).unwrap();
    let result = journal.rollback(&eid);
    assert!(matches!(
        result,
        Err(EffectJournalError::AlreadyTerminal { .. })
    ));
}

#[test]
fn list_pending_returns_only_prepared_effects() {
    let journal = MockEffectJournal::new();
    let id = test_instance_id();

    let r1 = EffectRecord::new(
        "fx-pending".to_string(),
        EffectKind::HttpCall,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let r2 = EffectRecord::new(
        "fx-committed".to_string(),
        EffectKind::SqlQuery,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();
    let r3 = EffectRecord::new(
        "fx-rolledback".to_string(),
        EffectKind::BlobWrite,
        json!({}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap();

    let eid2 = journal.prepare(&id, r2).unwrap();
    let _ = journal.prepare(&id, r1).unwrap();
    let eid3 = journal.prepare(&id, r3).unwrap();

    journal.commit(&eid2).unwrap();
    journal.rollback(&eid3).unwrap();

    let pending = journal.list_pending(&id).unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].intent_id(), "fx-pending");
}

#[test]
fn commit_returns_not_found_for_unknown_effect() {
    let journal = MockEffectJournal::new();
    let eid = EffectId::new(&test_instance_id(), "nonexistent").unwrap();
    let result = journal.commit(&eid);
    assert!(matches!(result, Err(EffectJournalError::NotFound { .. })));
}

#[test]
fn rollback_returns_not_found_for_unknown_effect() {
    let journal = MockEffectJournal::new();
    let eid = EffectId::new(&test_instance_id(), "nonexistent").unwrap();
    let result = journal.rollback(&eid);
    assert!(matches!(result, Err(EffectJournalError::NotFound { .. })));
}
