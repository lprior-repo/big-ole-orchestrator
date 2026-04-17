//! Fjall-backed persistent implementation of `EffectJournal` for production use.

use std::sync::Arc;

use vo_types::EffectIntent;
use vo_types::{EffectRecord, InstanceId};

use super::{EffectId, EffectJournal, EffectJournalError, EFFECTS_PARTITION};

pub struct FjallEffectJournal {
    partition: Arc<fjall::Keyspace>,
}

impl std::fmt::Debug for FjallEffectJournal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FjallEffectJournal").finish()
    }
}

impl std::fmt::Debug for FjallEffectJournal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FjallEffectJournal").finish()
    }
}

impl FjallEffectJournal {
    /// Opens a new effect journal backed by the given keyspace.
    ///
    /// # Errors
    ///
    /// Returns `EffectJournalError::Storage` if the effects partition cannot be opened.
    pub fn open(db: &fjall::Database) -> Result<Self, EffectJournalError> {
        let partition = db
            .keyspace(EFFECTS_PARTITION, || {
                fjall::KeyspaceCreateOptions::default()
            })
            .map_err(|e| EffectJournalError::Storage {
                reason: format!("failed to open effects partition: {e}"),
            })?;
        Ok(Self {
            partition: Arc::new(partition),
        })
    }
}

impl EffectJournal for FjallEffectJournal {
    fn prepare(
        &self,
        instance_id: &InstanceId,
        record: EffectRecord,
    ) -> Result<EffectId, EffectJournalError> {
        let intent_id = record.intent_id().to_string();
        let effect_id = EffectId::new(instance_id, intent_id.as_str())?;
        let key = super::encode_effect_key(&effect_id);

        if let Ok(Some(_)) = self.partition.get(&key) {
            return Ok(effect_id);
        }

        let value = super::encode_effect_record(&record)?;
        self.partition
            .insert(&key, &value)
            .map_err(|e| EffectJournalError::Storage {
                reason: e.to_string(),
            })?;
        Ok(effect_id)
    }

    fn commit(&self, effect_id: &EffectId) -> Result<(), EffectJournalError> {
        let key = super::encode_effect_key(effect_id);
        let record = self
            .get_impl(&key)?
            .ok_or_else(|| EffectJournalError::NotFound {
                effect_id: effect_id.as_str().to_string(),
            })?;

        if record.status().is_terminal() {
            return Err(EffectJournalError::AlreadyTerminal {
                effect_id: effect_id.as_str().to_string(),
                current_status: format!("{:?}", record.status()),
            });
        }

        let ts =
            Some(
                vo_types::TimestampMs::parse("100").map_err(|e| EffectJournalError::Storage {
                    reason: format!("failed to parse timestamp: {e}"),
                })?,
            );

        let next_record = EffectRecord::new(
            record.intent_id().to_string(),
            record.kind(),
            record.params_json().clone(),
            EffectIntent::Committed,
            ts,
        )
        .ok_or_else(|| EffectJournalError::Storage {
            reason: "failed to create record".to_string(),
        })?;

        let value = super::encode_effect_record(&next_record)?;
        self.partition
            .insert(&key, &value)
            .map_err(|e| EffectJournalError::Storage {
                reason: e.to_string(),
            })
    }

    fn rollback(&self, effect_id: &EffectId) -> Result<(), EffectJournalError> {
        let key = super::encode_effect_key(effect_id);
        let record = self
            .get_impl(&key)?
            .ok_or_else(|| EffectJournalError::NotFound {
                effect_id: effect_id.as_str().to_string(),
            })?;

        if record.status().is_terminal() {
            return Err(EffectJournalError::AlreadyTerminal {
                effect_id: effect_id.as_str().to_string(),
                current_status: format!("{:?}", record.status()),
            });
        }

        let next_record = EffectRecord::new(
            record.intent_id().to_string(),
            record.kind(),
            record.params_json().clone(),
            EffectIntent::RolledBack,
            None,
        )
        .ok_or_else(|| EffectJournalError::Storage {
            reason: "failed to create record".to_string(),
        })?;

        let value = super::encode_effect_record(&next_record)?;
        self.partition
            .insert(&key, &value)
            .map_err(|e| EffectJournalError::Storage {
                reason: e.to_string(),
            })
    }

    fn list_pending(
        &self,
        instance_id: &InstanceId,
    ) -> Result<Vec<EffectRecord>, EffectJournalError> {
        let prefix = format!("{instance_id}::");
        let prefix_bytes = prefix.as_bytes();
        let mut results = Vec::new();

        let iter = self.partition.iter();
        for item in iter {
            let (key_bytes, value_bytes) =
                item.into_inner().map_err(|e| EffectJournalError::Storage {
                    reason: e.to_string(),
                })?;

            if !key_bytes.starts_with(prefix_bytes) {
                continue;
            }

            let record = super::decode_effect_record(&value_bytes)?;
            if record.status() == EffectIntent::Prepared {
                results.push(record);
            }
        }

        Ok(results)
    }

    fn compact(&self, older_than: vo_types::TimestampMs) -> Result<usize, EffectJournalError> {
        let mut removed = 0;
        let keys_to_remove: Vec<Vec<u8>> = {
            let iter = self.partition.iter();
            let mut keys = Vec::new();
            for item in iter {
<<<<<<< HEAD
                let (key_bytes, value_bytes) =
                    item.into_inner().map_err(|e| EffectJournalError::Storage {
                        reason: e.to_string(),
                    })?;
=======
                let (key_bytes, value_bytes) = item.map_err(|e| EffectJournalError::Storage {
                    reason: e.to_string(),
                })?;
>>>>>>> origin/polecat/synth-mnw6kj8v

                let record = super::decode_effect_record(&value_bytes)?;

                if record.status().is_terminal() {
                    if let Some(committed_at) = record.committed_at() {
                        if *committed_at < older_than {
                            keys.push(key_bytes.to_vec());
                        }
                    }
                }
            }
            keys
        };

        for key in keys_to_remove {
            self.partition
                .remove(&key)
                .map_err(|e| EffectJournalError::Storage {
                    reason: format!("failed to remove key during compaction: {e}"),
                })?;
            removed += 1;
        }

        Ok(removed)
    }
}

impl FjallEffectJournal {
    fn get_impl(&self, key: &[u8]) -> Result<Option<EffectRecord>, EffectJournalError> {
        match self.partition.get(key) {
            Ok(Some(bytes)) => {
                let record = super::decode_effect_record(&bytes)?;
                Ok(Some(record))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(EffectJournalError::Storage {
                reason: e.to_string(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::{tempdir, TempDir};
    use vo_types::EffectKind;

    fn sample_instance_id() -> InstanceId {
        InstanceId::from_bytes([1u8; 16])
    }

    fn create_test_keyspace() -> (fjall::Database, TempDir) {
        let dir = tempdir().unwrap();
        let db = fjall::Database::builder(dir.path()).open().unwrap();
        (db, dir)
    }

    #[test]
    fn fjall_journal_prepare_returns_effect_id_for_new_intent() {
        let (keyspace, _dir) = create_test_keyspace();
        let journal = FjallEffectJournal::open(&keyspace).unwrap();
        let id = sample_instance_id();
        let record = EffectRecord::new(
            "fx-1".to_string(),
            EffectKind::HttpCall,
            serde_json::json!({"url": "https://api.stripe.com"}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        let result = journal.prepare(&id, record);
        let expected = EffectId::new(&id, "fx-1").unwrap();
        assert_eq!(result.unwrap().as_str(), expected.as_str());
    }

    #[test]
    fn fjall_journal_prepare_is_idempotent() {
        let (keyspace, _dir) = create_test_keyspace();
        let journal = FjallEffectJournal::open(&keyspace).unwrap();
        let id = sample_instance_id();
        let record = EffectRecord::new(
            "fx-1".to_string(),
            EffectKind::HttpCall,
            serde_json::json!({}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        let first = journal.prepare(&id, record.clone()).unwrap();
        let second = journal.prepare(&id, record).unwrap();
        assert_eq!(first, second);
        let pending = journal.list_pending(&id).unwrap();
        assert_eq!(pending.len(), 1);
    }

    #[test]
    fn fjall_journal_commit_transitions_prepared_to_committed() {
        let (keyspace, _dir) = create_test_keyspace();
        let journal = FjallEffectJournal::open(&keyspace).unwrap();
        let id = sample_instance_id();
        let record = EffectRecord::new(
            "fx-commit".to_string(),
            EffectKind::HttpCall,
            serde_json::json!({}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        let eid = journal.prepare(&id, record).unwrap();
        let result = journal.commit(&eid);
        assert_eq!(result, Ok(()));
        let pending = journal.list_pending(&id).unwrap();
        assert!(pending.is_empty());
    }

    #[test]
    fn fjall_journal_rollback_transitions_prepared_to_rolledback() {
        let (keyspace, _dir) = create_test_keyspace();
        let journal = FjallEffectJournal::open(&keyspace).unwrap();
        let id = sample_instance_id();
        let record = EffectRecord::new(
            "fx-rollback".to_string(),
            EffectKind::SqlQuery,
            serde_json::json!({}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        let eid = journal.prepare(&id, record).unwrap();
        let result = journal.rollback(&eid);
        assert_eq!(result, Ok(()));
        let pending = journal.list_pending(&id).unwrap();
        assert!(pending.is_empty());
    }

    #[test]
    fn fjall_journal_list_pending_returns_only_prepared_effects() {
        let (keyspace, _dir) = create_test_keyspace();
        let journal = FjallEffectJournal::open(&keyspace).unwrap();
        let id = sample_instance_id();

        let r1 = EffectRecord::new(
            "fx-pending".to_string(),
            EffectKind::HttpCall,
            serde_json::json!({}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        let r2 = EffectRecord::new(
            "fx-committed".to_string(),
            EffectKind::SqlQuery,
            serde_json::json!({}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        let r3 = EffectRecord::new(
            "fx-rolledback".to_string(),
            EffectKind::BlobWrite,
            serde_json::json!({}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();

        let eid2 = journal.prepare(&id, r2).unwrap();
        let eid1 = journal.prepare(&id, r1).unwrap();
        let eid3 = journal.prepare(&id, r3).unwrap();

        journal.commit(&eid2).unwrap();
        journal.rollback(&eid3).unwrap();

        let pending = journal.list_pending(&id).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].intent_id(), "fx-pending");
    }

    #[test]
    fn fjall_journal_commit_already_terminal_returns_error() {
        let (keyspace, _dir) = create_test_keyspace();
        let journal = FjallEffectJournal::open(&keyspace).unwrap();
        let id = sample_instance_id();
        let record = EffectRecord::new(
            "fx-twice".to_string(),
            EffectKind::HttpCall,
            serde_json::json!({}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        let eid = journal.prepare(&id, record).unwrap();
        journal.commit(&eid).unwrap();
        let result = journal.commit(&eid);
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(EffectJournalError::AlreadyTerminal { .. })
        ));
    }

    #[test]
    fn fjall_journal_rollback_already_terminal_returns_error() {
        let (keyspace, _dir) = create_test_keyspace();
        let journal = FjallEffectJournal::open(&keyspace).unwrap();
        let id = sample_instance_id();
        let record = EffectRecord::new(
            "fx-twice-rb".to_string(),
            EffectKind::HttpCall,
            serde_json::json!({}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        let eid = journal.prepare(&id, record).unwrap();
        journal.rollback(&eid).unwrap();
        let result = journal.rollback(&eid);
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(EffectJournalError::AlreadyTerminal { .. })
        ));
    }

    #[test]
    fn fjall_journal_commit_nonexistent_returns_not_found() {
        let (keyspace, _dir) = create_test_keyspace();
        let journal = FjallEffectJournal::open(&keyspace).unwrap();
        let id = sample_instance_id();
        let effect_id = EffectId::new(&id, "nonexistent").unwrap();
        let result = journal.commit(&effect_id);
        assert!(matches!(result, Err(EffectJournalError::NotFound { .. })));
    }

    #[test]
    fn fjall_journal_rollback_nonexistent_returns_not_found() {
        let (keyspace, _dir) = create_test_keyspace();
        let journal = FjallEffectJournal::open(&keyspace).unwrap();
        let id = sample_instance_id();
        let effect_id = EffectId::new(&id, "nonexistent").unwrap();
        let result = journal.rollback(&effect_id);
        assert!(matches!(result, Err(EffectJournalError::NotFound { .. })));
    }
    #[test]
    fn fjall_journal_compact_removes_old_terminal_effects() {
<<<<<<< HEAD
        let (keyspace, _dir) = create_test_keyspace();
=======
        let keyspace = create_test_keyspace();
>>>>>>> origin/polecat/synth-mnw6kj8v
        let journal = FjallEffectJournal::open(&keyspace).unwrap();
        let id = sample_instance_id();

        let old_record = EffectRecord::new(
            "fx-old".to_string(),
            EffectKind::HttpCall,
            serde_json::json!({}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        let old_eid = journal.prepare(&id, old_record).unwrap();
        journal.commit(&old_eid).unwrap();

        let new_record = EffectRecord::new(
            "fx-new".to_string(),
            EffectKind::SqlQuery,
            serde_json::json!({}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        let new_eid = journal.prepare(&id, new_record).unwrap();
        journal.commit(&new_eid).unwrap();

        let old_ts = vo_types::TimestampMs::parse("150").unwrap();
        let new_ts = vo_types::TimestampMs::parse("200").unwrap();

        let removed = journal.compact(old_ts).unwrap();
        assert_eq!(removed, 2);

        let removed_new = journal.compact(new_ts).unwrap();
        assert_eq!(removed_new, 0);
    }

    #[test]
    fn fjall_journal_compact_does_not_remove_prepared_effects() {
<<<<<<< HEAD
        let (keyspace, _dir) = create_test_keyspace();
=======
        let keyspace = create_test_keyspace();
>>>>>>> origin/polecat/synth-mnw6kj8v
        let journal = FjallEffectJournal::open(&keyspace).unwrap();
        let id = sample_instance_id();

        let prepared_record = EffectRecord::new(
            "fx-prepared".to_string(),
            EffectKind::BlobWrite,
            serde_json::json!({}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        journal.prepare(&id, prepared_record).unwrap();

        let committed_record = EffectRecord::new(
            "fx-committed".to_string(),
            EffectKind::HttpCall,
            serde_json::json!({}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        let committed_eid = journal.prepare(&id, committed_record).unwrap();
        journal.commit(&committed_eid).unwrap();

        let ts = vo_types::TimestampMs::parse("1000").unwrap();
        let removed = journal.compact(ts).unwrap();

        assert_eq!(removed, 1);

        let pending = journal.list_pending(&id).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].intent_id(), "fx-prepared");
    }

    #[test]
    fn fjall_journal_compact_does_not_remove_rolledback_effects_without_timestamp() {
<<<<<<< HEAD
        let (keyspace, _dir) = create_test_keyspace();
=======
        let keyspace = create_test_keyspace();
>>>>>>> origin/polecat/synth-mnw6kj8v
        let journal = FjallEffectJournal::open(&keyspace).unwrap();
        let id = sample_instance_id();

        let rolled_back_record = EffectRecord::new(
            "fx-rolledback".to_string(),
            EffectKind::SqlQuery,
            serde_json::json!({}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        let rb_eid = journal.prepare(&id, rolled_back_record).unwrap();
        journal.rollback(&rb_eid).unwrap();

        let ts = vo_types::TimestampMs::parse("1000").unwrap();
        let removed = journal.compact(ts).unwrap();

        assert_eq!(removed, 0);
    }
}
