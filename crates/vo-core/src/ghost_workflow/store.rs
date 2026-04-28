//! Fjall-backed persistent store for workflow registrations.
//!
//! Persists `WorkflowRegistration` records to Fjall keyspace so that
//! GhostLifecycle state survives restarts.

use std::sync::Arc;

use vo_types::{WorkflowName, TimestampMs};

use super::{GhostWorkflowError, WorkflowRegistration};
use vo_types::RegistrationStatus;

const WORKFLOW_REGISTRATIONS_PARTITION: &str = "workflow_registrations";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GhostStoreError {
    #[error("registration not found: {workflow}")]
    NotFound { workflow: String },
    #[error("storage error: {reason}")]
    Storage { reason: String },
}

impl From<GhostStoreError> for GhostWorkflowError {
    fn from(err: GhostStoreError) -> Self {
        match err {
            GhostStoreError::NotFound { workflow } => {
                GhostWorkflowError::InvalidTransition {
                    workflow,
                    from: RegistrationStatus::Deleted,
                    to: RegistrationStatus::Deactivated,
                }
            }
            GhostStoreError::Storage { reason } => {
                GhostWorkflowError::ReaperNotDeactivated {
                    workflow: reason.clone(),
                    status: RegistrationStatus::Active,
                }
            }
        }
    }
}

pub struct GhostLifecycleStore {
    partition: Arc<fjall::Keyspace>,
}

impl GhostLifecycleStore {
    pub fn open(db: &fjall::Database) -> Result<Self, GhostStoreError> {
        let partition = db
            .keyspace(
                WORKFLOW_REGISTRATIONS_PARTITION,
                fjall::KeyspaceCreateOptions::default(),
            )
            .map_err(|e| GhostStoreError::Storage {
                reason: format!("failed to open workflow_registrations partition: {e}"),
            })?;
        Ok(Self {
            partition: Arc::new(partition),
        })
    }

    pub fn get(&self, name: &WorkflowName) -> Result<WorkflowRegistration, GhostStoreError> {
        let key = name.as_str();
        match self.partition.get(key) {
            Ok(Some(value_bytes)) => {
                serde_json::from_slice(&value_bytes).map_err(|e| GhostStoreError::Storage {
                    reason: format!("failed to deserialize registration: {e}"),
                })
            }
            Ok(None) => Err(GhostStoreError::NotFound {
                workflow: name.as_str().to_string(),
            }),
            Err(e) => Err(GhostStoreError::Storage {
                reason: e.to_string(),
            }),
        }
    }

    pub fn put(&self, reg: &WorkflowRegistration) -> Result<(), GhostStoreError> {
        let key = reg.name().as_str();
        let value_bytes = serde_json::to_vec(reg).map_err(|e| GhostStoreError::Storage {
            reason: format!("failed to serialize registration: {e}"),
        })?;
        self.partition
            .insert(key, &value_bytes)
            .map_err(|e| GhostStoreError::Storage {
                reason: e.to_string(),
            })?;
        Ok(())
    }

    pub fn delete(&self, name: &WorkflowName) -> Result<(), GhostStoreError> {
        let key = name.as_str();
        match self.partition.get(key) {
            Ok(Some(_)) => {
                self.partition
                    .remove(key)
                    .map_err(|e| GhostStoreError::Storage {
                        reason: e.to_string(),
                    })?;
                Ok(())
            }
            Ok(None) => Err(GhostStoreError::NotFound {
                workflow: name.as_str().to_string(),
            }),
            Err(e) => Err(GhostStoreError::Storage {
                reason: e.to_string(),
            }),
        }
    }

    pub fn list_all(&self) -> Result<Vec<WorkflowRegistration>, GhostStoreError> {
        let mut registrations = Vec::new();
        for item in self.partition.iter() {
            let (_key_bytes, value_bytes) = item.into_inner().map_err(|e| GhostStoreError::Storage {
                reason: e.to_string(),
            })?;
            let reg: WorkflowRegistration = serde_json::from_slice(&value_bytes)
                .map_err(|e| GhostStoreError::Storage {
                    reason: format!("failed to deserialize registration: {e}"),
                })?;
            registrations.push(reg);
        }
        Ok(registrations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vo_types::{BinaryHash, TimestampMs, WorkflowName};

    fn make_hash() -> BinaryHash {
        BinaryHash::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap()
    }

    fn make_name(s: &str) -> WorkflowName {
        WorkflowName::parse(s).unwrap()
    }

    fn make_ts(ms: u64) -> TimestampMs {
        TimestampMs::try_from(ms).unwrap()
    }

    fn make_registration(name: &str) -> WorkflowRegistration {
        WorkflowRegistration::new(make_name(name), make_hash(), make_ts(1000))
    }

    #[test]
    fn ghost_lifecycle_store_put_and_get() {
        let dir = tempfile::tempdir().unwrap();
        let db = fjall::Database::builder(dir.path()).open().unwrap();
        let store = GhostLifecycleStore::open(&db).unwrap();

        let reg = make_registration("test-wf");
        store.put(&reg).unwrap();

        let retrieved = store.get(make_name("test-wf")).unwrap();
        assert_eq!(retrieved.name(), reg.name());
        assert_eq!(retrieved.status(), reg.status());
    }

    #[test]
    fn ghost_lifecycle_store_get_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let db = fjall::Database::builder(dir.path()).open().unwrap();
        let store = GhostLifecycleStore::open(&db).unwrap();

        let result = store.get(make_name("nonexistent"));
        assert!(matches!(result, Err(GhostStoreError::NotFound { .. })));
    }

    #[test]
    fn ghost_lifecycle_store_delete() {
        let dir = tempfile::tempdir().unwrap();
        let db = fjall::Database::builder(dir.path()).open().unwrap();
        let store = GhostLifecycleStore::open(&db).unwrap();

        let reg = make_registration("delete-me");
        store.put(&reg).unwrap();
        store.delete(make_name("delete-me")).unwrap();

        let result = store.get(make_name("delete-me"));
        assert!(matches!(result, Err(GhostStoreError::NotFound { .. })));
    }

    #[test]
    fn ghost_lifecycle_store_list_all() {
        let dir = tempfile::tempdir().unwrap();
        let db = fjall::Database::builder(dir.path()).open().unwrap();
        let store = GhostLifecycleStore::open(&db).unwrap();

        store.put(&make_registration("wf-a")).unwrap();
        store.put(&make_registration("wf-b")).unwrap();

        let all = store.list_all().unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn ghost_lifecycle_store_persists_across_restart() {
        let dir = tempfile::tempdir().unwrap();
        let db = fjall::Database::builder(dir.path()).open().unwrap();
        let store = GhostLifecycleStore::open(&db).unwrap();

        let reg = make_registration("persist-test");
        store.put(&reg).unwrap();
        drop(store);
        drop(db);

        let db2 = fjall::Database::builder(dir.path()).open().unwrap();
        let store2 = GhostLifecycleStore::open(&db2).unwrap();

        let retrieved = store2.get(make_name("persist-test")).unwrap();
        assert_eq!(retrieved.name(), make_name("persist-test"));
    }
}
