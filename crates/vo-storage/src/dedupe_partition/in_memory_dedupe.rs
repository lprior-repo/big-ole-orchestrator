//! In-memory implementation of `DedupeStore` for testing and development.

use std::collections::HashMap;
use std::sync::Mutex;

use vo_types::{DedupeKey, InstanceId};

use super::{AdmissionResult, DedupeEntry, DedupeStore, DedupeStoreError};

/// In-memory implementation of `DedupeStore` for testing and development.
#[derive(Debug, Default)]
pub struct InMemoryDedupeStore {
    entries: Mutex<HashMap<String, DedupeEntry>>,
}

impl InMemoryDedupeStore {
    /// Creates a new empty `InMemoryDedupeStore`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }
}

impl DedupeStore for InMemoryDedupeStore {
    fn check_and_insert(
        &self,
        key: &DedupeKey,
        instance_id: &InstanceId,
        ttl_ms: u64,
    ) -> Result<AdmissionResult, DedupeStoreError> {
        if ttl_ms == 0 {
            return Err(DedupeStoreError::InvalidArgument);
        }

        #[allow(clippy::cast_possible_truncation, clippy::expect_used)]
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time is after UNIX epoch")
            .as_millis() as u64;
        let expires_at = now_ms.saturating_add(ttl_ms);

        let key_str = key.as_str().to_string();

        let mut entries = self.entries.lock().map_err(|e| DedupeStoreError::Storage {
            reason: e.to_string(),
        })?;

        if let Some(entry) = entries.get(&key_str) {
            if !entry.is_expired(now_ms) {
                return Ok(AdmissionResult::Duplicate {
                    instance_id: entry.instance_id().to_string(),
                });
            }
        }

        let entry = DedupeEntry::new(key_str.clone(), instance_id.to_string(), expires_at)?;
        entries.insert(key_str, entry);
        drop(entries);

        Ok(AdmissionResult::Admitted)
    }

    fn purge_expired(&self, now_ms: u64) -> Result<u64, DedupeStoreError> {
        let mut entries = self.entries.lock().map_err(|e| DedupeStoreError::Storage {
            reason: e.to_string(),
        })?;

        let keys_to_remove: Vec<String> = entries
            .iter()
            .filter(|(_, entry)| entry.is_expired(now_ms))
            .map(|(k, _)| k.clone())
            .collect();

        let count = keys_to_remove.len() as u64;
        for key in keys_to_remove {
            entries.remove(&key);
        }
        drop(entries);

        Ok(count)
    }

    fn contains(&self, key: &DedupeKey) -> Result<bool, DedupeStoreError> {
        #[allow(clippy::cast_possible_truncation, clippy::expect_used)]
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time is after UNIX epoch")
            .as_millis() as u64;

        let entries = self.entries.lock().map_err(|e| DedupeStoreError::Storage {
            reason: e.to_string(),
        })?;

        let key_str = key.as_str().to_string();
        let result = entries
            .get(&key_str)
            .is_some_and(|e| !e.is_expired(now_ms));
        drop(entries);
        Ok(result)
    }
}
