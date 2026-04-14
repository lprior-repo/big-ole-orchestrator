//! In-memory implementation of `EffectJournal` for testing and development.

use std::collections::HashMap;
use std::sync::Mutex;

use vo_types::EffectIntent;
use vo_types::{EffectRecord, InstanceId};

use super::{EffectId, EffectJournal, EffectJournalError};

/// In-memory implementation of `EffectJournal` for testing and development.
#[derive(Debug, Default)]
pub struct InMemoryEffectJournal {
    records: Mutex<HashMap<String, EffectRecord>>,
}

impl InMemoryEffectJournal {
    /// Creates a new empty `InMemoryEffectJournal`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            records: Mutex::new(HashMap::new()),
        }
    }

    /// Ensure the record is not already Committed or `RolledBack`.
    fn ensure_not_terminal(record: &EffectRecord, key: &str) -> Result<(), EffectJournalError> {
        match record.status() {
            EffectIntent::Committed | EffectIntent::RolledBack => {
                Err(EffectJournalError::AlreadyTerminal {
                    effect_id: key.to_string(),
                    current_status: format!("{:?}", record.status()),
                })
            }
            _ => Ok(()),
        }
    }

    /// Constructs the next `EffectRecord` for the target intent.
    fn construct_next_record(
        record: &EffectRecord,
        target: EffectIntent,
    ) -> Result<EffectRecord, EffectJournalError> {
        let ts = match target {
            EffectIntent::Committed => Some(vo_types::TimestampMs::parse("100").map_err(|e| {
                EffectJournalError::Storage {
                    reason: format!("failed to parse timestamp: {e}"),
                }
            })?),
            EffectIntent::RolledBack => None,
            _ => unreachable!(),
        };
        EffectRecord::new(
            record.intent_id().to_string(),
            record.kind(),
            record.params_json().clone(),
            target,
            ts,
        )
        .ok_or_else(|| EffectJournalError::Storage {
            reason: "failed to create record".to_string(),
        })
    }

    /// Validate record exists and is in Prepared state, then apply transition.
    fn validate_and_transition(
        record: &EffectRecord,
        key: &str,
        target: EffectIntent,
    ) -> Result<EffectRecord, EffectJournalError> {
        Self::ensure_not_terminal(record, key)?;
        Self::construct_next_record(record, target)
    }
}

/// Suppress `significant_drop_tightening` in lock guard scopes.
#[allow(clippy::significant_drop_tightening)]
impl EffectJournal for InMemoryEffectJournal {
    fn prepare(
        &self,
        instance_id: &InstanceId,
        record: EffectRecord,
    ) -> Result<EffectId, EffectJournalError> {
        let intent_id = record.intent_id().to_string();
        let effect_id = EffectId::new(instance_id, intent_id.as_str())?;
        let key = effect_id.as_str().to_string();

        let mut records = self
            .records
            .lock()
            .map_err(|e| EffectJournalError::Storage {
                reason: e.to_string(),
            })?;

        if records.contains_key(&key) {
            return Ok(effect_id);
        }

        records.insert(key, record);
        Ok(effect_id)
    }

    fn commit(&self, effect_id: &EffectId) -> Result<(), EffectJournalError> {
        let key = effect_id.as_str().to_string();
        let mut records = self
            .records
            .lock()
            .map_err(|e| EffectJournalError::Storage {
                reason: e.to_string(),
            })?;

        let record = records
            .get_mut(&key)
            .ok_or_else(|| EffectJournalError::NotFound {
                effect_id: key.clone(),
            })?;

        let next = Self::validate_and_transition(record, &key, EffectIntent::Committed)?;
        *record = next;
        Ok(())
    }

    fn rollback(&self, effect_id: &EffectId) -> Result<(), EffectJournalError> {
        let key = effect_id.as_str().to_string();
        let mut records = self
            .records
            .lock()
            .map_err(|e| EffectJournalError::Storage {
                reason: e.to_string(),
            })?;

        let record = records
            .get_mut(&key)
            .ok_or_else(|| EffectJournalError::NotFound {
                effect_id: key.clone(),
            })?;

        let next = Self::validate_and_transition(record, &key, EffectIntent::RolledBack)?;
        *record = next;
        Ok(())
    }

    fn list_pending(
        &self,
        instance_id: &InstanceId,
    ) -> Result<Vec<EffectRecord>, EffectJournalError> {
        let records = self
            .records
            .lock()
            .map_err(|e| EffectJournalError::Storage {
                reason: e.to_string(),
            })?;

        let prefix = format!("{instance_id}::");
        Ok(records
            .iter()
            .filter(|(k, v)| k.starts_with(&prefix) && v.status() == EffectIntent::Prepared)
            .map(|(_, v)| v.clone())
            .collect())
    }

    fn compact(&self, older_than: vo_types::TimestampMs) -> Result<usize, EffectJournalError> {
        let mut records = self
            .records
            .lock()
            .map_err(|e| EffectJournalError::Storage {
                reason: e.to_string(),
            })?;

        let keys_to_remove: Vec<String> = records
            .iter()
            .filter(|(_, v)| {
                if v.status().is_terminal() {
                    if let Some(committed_at) = v.committed_at() {
                        return *committed_at < older_than;
                    }
                }
                false
            })
            .map(|(k, _)| k.clone())
            .collect();

        let count = keys_to_remove.len();
        for key in keys_to_remove {
            records.remove(&key);
        }

        Ok(count)
    }
}
