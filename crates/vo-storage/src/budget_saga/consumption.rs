use crate::append::WriteClass;

use std::sync::{Arc, Mutex};

use crate::append::{BudgetQueues, ClassifiedWrite, WriteClass};

use super::allocation::{BudgetManifest, SagaEntry, SagaError, SagaStatus};

/// Fjall-backed persistent store for saga entries.
pub struct SagaStore {
    partition: fjall::Keyspace,
}

    /// Open a saga store backed by the given keyspace.
    ///
    /// # Errors
    ///
    /// Returns [`SagaError::Storage`] if the `saga_manifest` partition cannot be opened.
    pub fn open(db: &fjall::Database) -> Result<Self, SagaError> {
        let partition = db
            .keyspace("saga_manifest", fjall::KeyspaceCreateOptions::default)
            .map_err(|e| SagaError::Storage {
                reason: format!("failed to open saga_manifest partition: {e}"),
            })?;
        Ok(Self { partition })
    }

    /// Stage a new entry in the persistent saga store.
    ///
    /// # Errors
    ///
    /// Returns [`SagaError::AlreadyExists`] if an entry with the same key already exists.
    /// Returns [`SagaError::Storage`] if serialization or disk write fails.
    pub fn stage_entry(
        &self,
        write_key: &str,
        class: WriteClass,
        size_bytes: u64,
    ) -> Result<(), SagaError> {
        if self.read_entry(write_key)?.is_some() {
            return Err(SagaError::AlreadyExists(write_key.to_string()));
        }
        let entry = SagaEntry {
            write_key: write_key.to_string(),
            class,
            size_bytes,
            status: SagaStatus::Staged,
        };
        let key = format!("entry:{write_key}").into_bytes();
        let value = serde_json::to_vec(&entry).map_err(|e| SagaError::Storage {
            reason: format!("failed to serialize saga entry: {e}"),
        })?;
        self.partition
            .insert(&key, &value)
            .map_err(|e| SagaError::Storage {
                reason: e.to_string(),
            })
    }

    /// Read a saga entry by write key.
    ///
    /// # Errors
    ///
    /// Returns [`SagaError::CorruptEntry`] if the stored data cannot be deserialized.
    /// Returns [`SagaError::Storage`] if the partition read fails.
    pub fn read_entry(&self, write_key: &str) -> Result<Option<SagaEntry>, SagaError> {
        let key = format!("entry:{write_key}").into_bytes();
        match self.partition.get(&key) {
            Ok(Some(bytes)) => {
                let entry: SagaEntry =
                    serde_json::from_slice(&bytes).map_err(|e| SagaError::CorruptEntry {
                        key: write_key.to_string(),
                        reason: e.to_string(),
                    })?;
                Ok(Some(entry))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(SagaError::Storage {
                reason: e.to_string(),
            }),
        }
    }

    /// Commit a staged entry, transitioning it to `Committed`.
    ///
    /// # Errors
    ///
    /// Returns [`SagaError::NotFound`] if no entry exists for the given key.
    /// Returns [`SagaError::InvalidState`] if the entry is not in the `Staged` state.
    /// Returns [`SagaError::Storage`] if serialization or disk write fails.
    pub fn commit_entry(&self, write_key: &str) -> Result<(), SagaError> {
        let mut entry = self
            .read_entry(write_key)?
            .ok_or_else(|| SagaError::NotFound(write_key.to_string()))?;
        if entry.status != SagaStatus::Staged {
            return Err(SagaError::InvalidState {
                key: write_key.to_string(),
                expected: SagaStatus::Staged,
                actual: entry.status,
            });
        }
        entry.status = SagaStatus::Committed;
        let key = format!("entry:{write_key}").into_bytes();
        let value = serde_json::to_vec(&entry).map_err(|e| SagaError::Storage {
            reason: format!("failed to serialize saga entry: {e}"),
        })?;
        self.partition
            .insert(&key, &value)
            .map_err(|e| SagaError::Storage {
                reason: e.to_string(),
            })
    }

    /// Roll back an entry, transitioning it to `RolledBack`.
    ///
    /// # Errors
    ///
    /// Returns [`SagaError::NotFound`] if no entry exists for the given key.
    /// Returns [`SagaError::AlreadyRolledBack`] if the entry is already rolled back.
    /// Returns [`SagaError::Storage`] if serialization or disk write fails.
    pub fn rollback_entry(&self, write_key: &str) -> Result<(), SagaError> {
        let mut entry = self
            .read_entry(write_key)?
            .ok_or_else(|| SagaError::NotFound(write_key.to_string()))?;
        if entry.status == SagaStatus::RolledBack {
            return Err(SagaError::AlreadyRolledBack(write_key.to_string()));
        }
        entry.status = SagaStatus::RolledBack;
        let key = format!("entry:{write_key}").into_bytes();
        let value = serde_json::to_vec(&entry).map_err(|e| SagaError::Storage {
            reason: format!("failed to serialize saga entry: {e}"),
        })?;
        self.partition
            .insert(&key, &value)
            .map_err(|e| SagaError::Storage {
                reason: e.to_string(),
            })
    }

    /// Recover from a crash by rolling back all staged entries.
    ///
    /// # Errors
    ///
    /// Returns [`SagaError::Storage`] if iterating or writing entries fails.
    /// Returns [`SagaError::CorruptEntry`] if a stored entry cannot be deserialized.
    pub fn recover(&self) -> Result<RecoveryOutcome, SagaError> {
        let mut count = 0usize;
        let iter = self.partition.iter();
        for item in iter {
            let (key_bytes, value_bytes) = item.into_inner().map_err(|e| SagaError::Storage {
                reason: e.to_string(),
            })?;
            let key_str = std::str::from_utf8(&key_bytes).unwrap_or("");
            let Some(write_key) = key_str.strip_prefix("entry:") else {
                continue;
            };
            let mut entry: SagaEntry =
                serde_json::from_slice(&value_bytes).map_err(|e| SagaError::CorruptEntry {
                    key: write_key.to_string(),
                    reason: e.to_string(),
                })?;
            if entry.status == SagaStatus::Staged {
                entry.status = SagaStatus::RolledBack;
                let value = serde_json::to_vec(&entry).map_err(|e| SagaError::Storage {
                    reason: format!("failed to serialize saga entry: {e}"),
                })?;
                self.partition
                    .insert(format!("entry:{write_key}").as_bytes(), &value)
                    .map_err(|e| SagaError::Storage {
                        reason: e.to_string(),
                    })?;
                count += 1;
            }
        }
        if count > 0 {
            Ok(RecoveryOutcome::RolledBack { count })
        } else {
            Ok(RecoveryOutcome::NothingToRecover)
        }
    }
}

/// Fjall-backed recovery outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryOutcome {
    NothingToRecover,
    RolledBack { count: usize },
}

pub struct DurableBudgetSaga {
    store: Option<SagaStore>,
    manifest: Arc<Mutex<BudgetManifest>>,
    queues: BudgetQueues<StagedWrite>,
}

#[derive(Debug, Clone)]
pub struct StagedWrite {
    pub write_key: String,
    pub class: WriteClass,
    pub size_bytes: u64,
    pub staged_at: std::time::SystemTime,
}

impl StagedWrite {
    #[must_use]
    pub fn new(write_key: String, class: WriteClass, size_bytes: u64) -> Self {
        Self {
            write_key,
            class,
            size_bytes,
            staged_at: std::time::SystemTime::now(),
        }
    }
}

impl ClassifiedWrite for StagedWrite {
    fn write_class(&self) -> WriteClass {
        self.class
    }

    fn size_bytes(&self) -> u64 {
        self.size_bytes
    }
}

impl DurableBudgetSaga {
    /// Commit a previously staged write entry.
    ///
    /// # Panics
    ///
    /// Panics if the internal manifest mutex is poisoned.
    ///
    /// # Errors
    ///
    /// Returns [`SagaError::NotFound`] if no entry exists for the given key.
    /// Returns [`SagaError::InvalidState`] if the entry is not in the `Staged` state.
    pub fn commit(&self, write_key: &str) -> Result<(), SagaError> {
        self.store.as_ref().map_or_else(
            || {
                #[expect(clippy::unwrap_used)]
                let mut manifest = self.manifest.lock().unwrap();
                manifest.commit(write_key)
            },
            |store| store.commit_entry(write_key),
        )
    }

    /// Roll back a write entry and dequeue it from the budget queues.
    ///
    /// # Panics
    ///
    /// Panics if the internal manifest mutex is poisoned.
    ///
    /// # Errors
    ///
    /// Returns [`SagaError::NotFound`] if no entry exists for the given key.
    /// Returns [`SagaError::AlreadyRolledBack`] if the entry is already rolled back.
    pub fn rollback(&self, write_key: &str) -> Result<(), SagaError> {
        self.queues.dequeue(self.get_class_for_key(write_key)?);
        self.store.as_ref().map_or_else(
            || {
                #[expect(clippy::unwrap_used)]
                let mut manifest = self.manifest.lock().unwrap();
                manifest.rollback(write_key)
            },
            |store| store.rollback_entry(write_key),
        )
    }

    /// Recover from a crash by rolling back all staged entries.
    ///
    /// # Panics
    ///
    /// Panics if the internal manifest mutex is poisoned.
    pub fn recover_from_crash(&self) {
        #[expect(clippy::unwrap_used)]
        let mut manifest = self.manifest.lock().unwrap();
        manifest.recover_staged_as_rolled_back();
    }

    pub(crate) fn get_class_for_key(&self, write_key: &str) -> Result<WriteClass, SagaError> {
        #[expect(clippy::unwrap_used)]
        let manifest = self.manifest.lock().unwrap();
        manifest
            .get(write_key)
            .map(|e| e.class)
            .ok_or_else(|| SagaError::NotFound(write_key.to_string()))
    }
}
