use serde::{Deserialize, Serialize};

use crate::replay::projection::ProjectionError;
use crate::snapshot_compat::{check_snapshot_compat, SnapshotCompat};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub projection_id: String,
    pub schema_version: u8,
    pub state_bytes: Vec<u8>,
    pub sequence: u64,
    pub created_at: u64,
    pub checksum: u64,
}

impl Snapshot {
    #[must_use]
    pub fn new(
        projection_id: String,
        schema_version: u8,
        state_bytes: Vec<u8>,
        sequence: u64,
        created_at: u64,
        checksum: u64,
    ) -> Self {
        Self {
            projection_id,
            schema_version,
            state_bytes,
            sequence,
            created_at,
            checksum,
        }
    }

    pub fn is_compatible_with(&self, engine_version: u8) -> bool {
        matches!(
            check_snapshot_compat(self.schema_version as u16, engine_version as u16),
            SnapshotCompat::Compatible
        )
    }
}

pub trait SnapshotStore: Send + Sync {
    fn save_snapshot(&self, snapshot: &Snapshot) -> Result<(), ProjectionError>;
    fn load_latest_snapshot(
        &self,
        projection_id: &str,
    ) -> Result<Option<Snapshot>, ProjectionError>;
    fn delete_snapshot(&self, projection_id: &str, sequence: u64) -> Result<(), ProjectionError>;
}

pub trait EventStore: Send + Sync {
    fn fetch_events(
        &self,
        projection_id: &str,
        from_sequence: u64,
        to_sequence: u64,
    ) -> Result<Vec<vo_types::events::EventEnvelope>, ProjectionError>;
}

#[derive(Debug)]
pub struct InMemorySnapshotStore {
    snapshots: std::sync::Mutex<std::collections::HashMap<String, Snapshot>>,
}

impl InMemorySnapshotStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            snapshots: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }
}

impl Default for InMemorySnapshotStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SnapshotStore for InMemorySnapshotStore {
    fn save_snapshot(&self, snapshot: &Snapshot) -> Result<(), ProjectionError> {
        self.snapshots
            .lock()
            .map_err(|_| ProjectionError::Storage("lock poisoned".to_string()))?
            .insert(snapshot.projection_id.clone(), snapshot.clone());
        Ok(())
    }

    fn load_latest_snapshot(
        &self,
        projection_id: &str,
    ) -> Result<Option<Snapshot>, ProjectionError> {
        Ok(self
            .snapshots
            .lock()
            .map_err(|_| ProjectionError::Storage("lock poisoned".to_string()))?
            .get(projection_id)
            .cloned())
    }

    fn delete_snapshot(&self, projection_id: &str, _sequence: u64) -> Result<(), ProjectionError> {
        self.snapshots
            .lock()
            .map_err(|_| ProjectionError::Storage("lock poisoned".to_string()))?
            .remove(projection_id);
        Ok(())
    }
}
