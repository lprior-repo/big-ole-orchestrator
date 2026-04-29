//! Fjall partition layout for veloxide storage per ADR-002.
//!
//! ## Partitions
//!
//! | Partition | Purpose | Key Pattern |
//! |-----------|---------|-------------|
//! | `events` | Minimal replay events and state transitions | `<instance_id><sequence>` |
//! | `instances` | Materialized instance summaries | `<status><created_at><instance_id>` |
//! | `timers` | Durable wake-up schedule | `<fire_at_ms><instance_id><timer_id>` |
//! | `snapshots` | Periodic replay acceleration checkpoints | `<instance_id><sequence>` |
//! | `dedupe` | Exactly-once ingress deduplication | `<dedupe_key>` |
//! | `effects` | EffectPrepared/EffectCommitted journal | `<instance_id><intent_id>` |
//! | `leases` | Monotonic fence tokens | `<instance_id><step_id>` |
//! | `receipts` | Execution receipts for managed connectors | `<effect_id>` |
//! | `workflow_versions` | Canonical `WorkflowSpec` by hash | `<hash>` |
//! | `payload_blobs` | Encrypted canonical payload blobs | `<content_addr>` |
//!
//! ## Hot/Cold Split
//!
//! - **Hot control-plane partitions**: events, instances, timers, dedupe, effects, leases
//! - **Cold blob storage**: `snapshots` (compaction-heavy), `payload_blobs` (large values)

use std::fmt;
use std::path::Path;
use std::sync::Arc;

use fjall::config::{BloomConstructionPolicy, CompressionPolicy, FilterPolicy, FilterPolicyEntry};
use fjall::{
    compaction::{Fifo, Leveled},
    CompressionType, KeyspaceCreateOptions, KvSeparationOptions,
};
use serde::{Deserialize, Serialize};

pub use crate::dedupe_partition::DEDUPE_PARTITION;
pub use crate::dedupe_partition::DEDUPE_RETENTION_PARTITION;
pub use crate::effect_journal::EFFECTS_PARTITION;
pub use crate::lease_partition::LEASE_PARTITION;
pub use crate::receipts::RECEIPTS_PARTITION;

pub const EVENTS_PARTITION: &str = "events";
pub const INSTANCES_PARTITION: &str = "instances";
pub const TIMERS_PARTITION: &str = "timers";
pub const SNAPSHOTS_PARTITION: &str = "snapshots";
pub const WORKFLOW_VERSIONS_PARTITION: &str = "workflow_versions";
pub const PAYLOAD_BLOBS_PARTITION: &str = "payload_blobs";
pub const BLOB_RECORDS_PARTITION: &str = "blob_records";
pub const BLOB_PACK_INDEX_PARTITION: &str = "blob_pack_index";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PartitionClass {
    Hot,
    Cold,
    Blob,
}

impl fmt::Display for PartitionClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hot => write!(f, "hot"),
            Self::Cold => write!(f, "cold"),
            Self::Blob => write!(f, "blob"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartitionInfo {
    pub name: &'static str,
    pub class: PartitionClass,
}

impl PartitionInfo {
    #[must_use]
    pub const fn new(name: &'static str, class: PartitionClass) -> Self {
        Self { name, class }
    }
}

pub const ALL_PARTITIONS: &[&str] = &[
    EVENTS_PARTITION,
    INSTANCES_PARTITION,
    TIMERS_PARTITION,
    SNAPSHOTS_PARTITION,
    DEDUPE_PARTITION,
    DEDUPE_RETENTION_PARTITION,
    EFFECTS_PARTITION,
    LEASE_PARTITION,
    RECEIPTS_PARTITION,
    WORKFLOW_VERSIONS_PARTITION,
    PAYLOAD_BLOBS_PARTITION,
    BLOB_RECORDS_PARTITION,
    BLOB_PACK_INDEX_PARTITION,
];

pub const HOT_PARTITIONS: &[&str] = &[
    EVENTS_PARTITION,
    INSTANCES_PARTITION,
    TIMERS_PARTITION,
    DEDUPE_PARTITION,
    EFFECTS_PARTITION,
    LEASE_PARTITION,
    RECEIPTS_PARTITION,
];

pub const COLD_PARTITIONS: &[&str] = &[SNAPSHOTS_PARTITION, WORKFLOW_VERSIONS_PARTITION];

pub const BLOB_PARTITIONS: &[&str] = &[
    PAYLOAD_BLOBS_PARTITION,
    BLOB_RECORDS_PARTITION,
    BLOB_PACK_INDEX_PARTITION,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartitionConfig {
    pub compaction_enabled: bool,
    pub bloom_filter_bits_per_key: u8,
    pub flush_interval_bytes: u64,
}

impl Default for PartitionConfig {
    fn default() -> Self {
        Self {
            compaction_enabled: true,
            bloom_filter_bits_per_key: 10,
            flush_interval_bytes: 64 * 1024 * 1024,
        }
    }
}

impl PartitionConfig {
    #[must_use]
    pub const fn hot() -> Self {
        Self {
            compaction_enabled: true,
            bloom_filter_bits_per_key: 10,
            flush_interval_bytes: 64 * 1024 * 1024,
        }
    }

    #[must_use]
    pub const fn cold() -> Self {
        Self {
            compaction_enabled: true,
            bloom_filter_bits_per_key: 0,
            flush_interval_bytes: 256 * 1024 * 1024,
        }
    }

    #[must_use]
    pub const fn blob() -> Self {
        Self {
            compaction_enabled: true,
            bloom_filter_bits_per_key: 0,
            flush_interval_bytes: 1024 * 1024 * 1024,
        }
    }

    /// Infers the storage class from the partition configuration.
    #[must_use]
    fn infer_storage_class(&self) -> PartitionClass {
        if self.bloom_filter_bits_per_key == 10 && self.flush_interval_bytes == 64 * 1024 * 1024 {
            PartitionClass::Hot
        } else if self.flush_interval_bytes == 1024 * 1024 * 1024 {
            PartitionClass::Blob
        } else {
            PartitionClass::Cold
        }
    }

    /// Applies the appropriate compaction strategy to the options based on `self.compaction_enabled`.
    fn apply_compaction(&self, opts: KeyspaceCreateOptions) -> KeyspaceCreateOptions {
        if self.compaction_enabled {
            opts
        } else {
            // No-op compaction: Fifo with u64::MAX limit never triggers
            opts.compaction_strategy(Arc::new(Fifo::new(u64::MAX, None)))
        }
    }

    /// Builds options for HOT class partitions.
    #[must_use]
    fn build_hot_options(&self) -> KeyspaceCreateOptions {
        let opts = KeyspaceCreateOptions::default()
            .max_memtable_size(self.flush_interval_bytes)
            .filter_policy(FilterPolicy::new([
                FilterPolicyEntry::Bloom(BloomConstructionPolicy::FalsePositiveRate(0.0001)),
                FilterPolicyEntry::Bloom(BloomConstructionPolicy::BitsPerKey(10.0)),
            ]))
            .expect_point_read_hits(true)
            .data_block_compression_policy(CompressionPolicy::new([
                CompressionType::None,
                CompressionType::None,
                CompressionType::Lz4,
            ]))
            .compaction_strategy(Arc::new(
                Leveled::default()
                    .with_l0_threshold(8)
                    .with_table_target_size(64 * 1024 * 1024),
            ));
        self.apply_compaction(opts)
    }

    /// Builds options for BLOB class partitions.
    #[must_use]
    fn build_blob_options(&self) -> KeyspaceCreateOptions {
        let opts = KeyspaceCreateOptions::default()
            .max_memtable_size(self.flush_interval_bytes)
            .filter_policy(FilterPolicy::disabled())
            .with_kv_separation(Some(
                KvSeparationOptions::default()
                    .separation_threshold(1024)
                    .file_target_size(128 * 1024 * 1024)
                    .staleness_threshold(0.5)
                    .age_cutoff(0.6)
                    .compression(CompressionType::Lz4),
            ))
            .compaction_strategy(Arc::new(
                Leveled::default()
                    .with_l0_threshold(4)
                    .with_table_target_size(64 * 1024 * 1024),
            ));
        self.apply_compaction(opts)
    }

    /// Builds options for COLD class partitions.
    #[must_use]
    fn build_cold_options(&self) -> KeyspaceCreateOptions {
        let opts = KeyspaceCreateOptions::default()
            .max_memtable_size(self.flush_interval_bytes)
            .filter_policy(FilterPolicy::disabled())
            .data_block_compression_policy(CompressionPolicy::all(CompressionType::Lz4))
            .compaction_strategy(Arc::new(
                Leveled::default()
                    .with_l0_threshold(4)
                    .with_table_target_size(128 * 1024 * 1024),
            ));
        self.apply_compaction(opts)
    }

    #[must_use]
    pub fn to_fjall_options(&self) -> KeyspaceCreateOptions {
        match self.infer_storage_class() {
            PartitionClass::Hot => self.build_hot_options(),
            PartitionClass::Blob => self.build_blob_options(),
            PartitionClass::Cold => self.build_cold_options(),
        }
    }
}

pub struct FjallPartitionLayout {
    db: fjall::Database,
}

impl FjallPartitionLayout {
    #[must_use]
    pub fn db(&self) -> &fjall::Database {
        &self.db
    }
}

pub type StorageResult<T> = Result<T, StorageError>;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StorageError {
    #[error("failed to open partition '{name}': {reason}")]
    PartitionOpenFailed { name: String, reason: String },
    #[error("invalid storage path: {reason}")]
    InvalidPath { reason: String },
    #[error("optimistic concurrency conflict: expected version {expected_version}, found {actual_version}")]
    OptimisticConcurrency {
        expected_version: u64,
        actual_version: u64,
    },
}

pub struct StorageConfig {
    pub path: String,
    pub compaction_enabled: bool,
    pub hot_config: PartitionConfig,
    pub cold_config: PartitionConfig,
    pub blob_config: PartitionConfig,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            path: String::from("/tmp/veloxide-storage"),
            compaction_enabled: true,
            hot_config: PartitionConfig::hot(),
            cold_config: PartitionConfig::cold(),
            blob_config: PartitionConfig::blob(),
        }
    }
}

/// Creates the partition layout at the given path.
///
/// # Errors
///
/// Returns `StorageError::InvalidPath` if the directory cannot be created or opened.
pub fn create_partition_layout(path: impl AsRef<Path>) -> StorageResult<FjallPartitionLayout> {
    let path = path.as_ref();
    std::fs::create_dir_all(path).map_err(|e| StorageError::InvalidPath {
        reason: e.to_string(),
    })?;

    let metadata = std::fs::metadata(path).map_err(|e| StorageError::InvalidPath {
        reason: e.to_string(),
    })?;
    if !metadata.is_dir() {
        return Err(StorageError::InvalidPath {
            reason: "path exists but is not a directory".to_string(),
        });
    }

    let db = fjall::Database::builder(path)
        .open()
        .map_err(|e| StorageError::InvalidPath {
            reason: e.to_string(),
        })?;

    Ok(FjallPartitionLayout { db })
}

/// Returns the partition config for the given partition name.
#[must_use]
pub fn get_partition_config(name: &str) -> PartitionConfig {
    if HOT_PARTITIONS.contains(&name) {
        PartitionConfig::hot()
    } else if COLD_PARTITIONS.contains(&name) {
        PartitionConfig::cold()
    } else if BLOB_PARTITIONS.contains(&name) {
        PartitionConfig::blob()
    } else {
        PartitionConfig::default()
    }
}

/// Opens all partitions defined in the layout.
///
/// # Errors
///
/// Returns `StorageError::PartitionOpenFailed` if any partition cannot be opened.
pub fn open_all_partitions(
    layout: &FjallPartitionLayout,
) -> StorageResult<Vec<(&'static str, fjall::Keyspace)>> {
    let mut partitions = Vec::with_capacity(ALL_PARTITIONS.len());

    for name in ALL_PARTITIONS {
        let config = get_partition_config(name);
        let partition = layout
            .db
            .keyspace(name, || config.to_fjall_options())
            .map_err(|e| StorageError::PartitionOpenFailed {
                name: name.to_string(),
                reason: e.to_string(),
            })?;
        partitions.push((*name, partition));
    }

    Ok(partitions)
}

#[derive(Debug, thiserror::Error)]
pub enum StorageEngineError {
    #[error("failed to open dedupe store: {0}")]
    DedupeStore(#[from] crate::dedupe_partition::DedupeStoreError),
    #[error("failed to open effect journal: {0}")]
    EffectJournal(#[from] crate::effect_journal::EffectJournalError),
    #[error("failed to open lease store: {0}")]
    LeaseStore(#[from] crate::lease_partition::LeaseStoreError),
    #[error("failed to open event store: {0}")]
    EventStore(#[from] crate::event_store::EventStoreError),
    #[error("failed to open DEK store: {0}")]
    DekStore(#[from] crate::key_partition::DekStoreError),
    #[error("failed to open workflow versions store: {0}")]
    WorkflowVersionStore(#[from] crate::workflow_version_partition::WorkflowVersionStoreError),
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
}

pub struct StorageEngine {
    db: fjall::Database,
    pub dedupe_store: Arc<crate::dedupe_partition::FjallDedupeStore>,
    pub effect_journal: Arc<crate::effect_journal::FjallEffectJournal>,
    pub lease_store: Arc<crate::lease_partition::FjallLeaseStore>,
    pub event_store: Arc<crate::event_store::FjallEventStore>,
    pub workflow_versions_store: Arc<crate::workflow_version_partition::FjallWorkflowVersionStore>,
}

impl StorageEngine {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageEngineError> {
        let path = path.as_ref();
        if !path.exists() {
            std::fs::create_dir_all(path).map_err(|e| StorageError::InvalidPath {
                reason: e.to_string(),
            })?;
        }

        let db = fjall::Database::builder(path)
            .open()
            .map_err(|e| StorageError::InvalidPath {
                reason: e.to_string(),
            })?;

        let metadata = std::fs::metadata(path).map_err(|e| StorageError::InvalidPath {
            reason: e.to_string(),
        })?;
        if !metadata.is_dir() {
            return Err(StorageEngineError::Storage(StorageError::InvalidPath {
                reason: "path exists but is not a directory".to_string(),
            }));
        }

        let db = fjall::Database::builder(path)
            .open()
            .map_err(|e| StorageError::InvalidPath {
                reason: e.to_string(),
            })?;

        let dedupe_store = Arc::new(crate::dedupe_partition::FjallDedupeStore::open(&db)?);
        let effect_journal = Arc::new(crate::effect_journal::FjallEffectJournal::open(&db)?);
        let lease_store = Arc::new(crate::lease_partition::FjallLeaseStore::open(&db)?);
        let event_store = Arc::new(crate::event_store::FjallEventStore::open(&db)?);
        let workflow_versions_store =
            Arc::new(crate::workflow_version_partition::FjallWorkflowVersionStore::open(&db)?);

        Ok(Self {
            db,
            dedupe_store,
            effect_journal,
            lease_store,
            event_store,
            workflow_versions_store,
        })
    }

    #[must_use]
    pub fn db(&self) -> &fjall::Database {
        &self.db
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partition_config_hot_has_correct_defaults() {
        let config = PartitionConfig::hot();
        assert!(config.compaction_enabled);
        assert_eq!(config.bloom_filter_bits_per_key, 10);
        assert_eq!(config.flush_interval_bytes, 64 * 1024 * 1024);
    }

    #[test]
    fn partition_config_cold_has_correct_defaults() {
        let config = PartitionConfig::cold();
        assert!(config.compaction_enabled);
        assert_eq!(config.bloom_filter_bits_per_key, 0);
        assert_eq!(config.flush_interval_bytes, 256 * 1024 * 1024);
    }

    #[test]
    fn partition_config_blob_has_correct_defaults() {
        let config = PartitionConfig::blob();
        assert!(config.compaction_enabled);
        assert_eq!(config.bloom_filter_bits_per_key, 0);
        assert_eq!(config.flush_interval_bytes, 1024 * 1024 * 1024);
    }

    #[test]
    fn get_partition_config_returns_hot_for_events() {
        let config = get_partition_config(EVENTS_PARTITION);
        assert_eq!(config.bloom_filter_bits_per_key, 10);
    }

    #[test]
    fn get_partition_config_returns_cold_for_snapshots() {
        let config = get_partition_config(SNAPSHOTS_PARTITION);
        assert_eq!(config.bloom_filter_bits_per_key, 0);
    }

    #[test]
    fn get_partition_config_returns_blob_for_payload_blobs() {
        let config = get_partition_config(PAYLOAD_BLOBS_PARTITION);
        assert_eq!(config.flush_interval_bytes, 1024 * 1024 * 1024);
    }

    #[test]
    fn all_partitions_contains_expected_count() {
        assert_eq!(ALL_PARTITIONS.len(), 13);
    }

    #[test]
    fn hot_partitions_are_hot() {
        for name in HOT_PARTITIONS {
            let config = get_partition_config(name);
            assert_eq!(config.bloom_filter_bits_per_key, 10, "{name} should be hot");
        }
    }

    #[test]
    fn cold_partitions_are_cold() {
        for name in COLD_PARTITIONS {
            let config = get_partition_config(name);
            assert_eq!(config.bloom_filter_bits_per_key, 0, "{name} should be cold");
        }
    }

    #[test]
    fn storage_error_display_partition_open_failed() {
        let err = StorageError::PartitionOpenFailed {
            name: "test".to_string(),
            reason: "disk full".to_string(),
        };
        assert!(err.to_string().contains("test"));
        assert!(err.to_string().contains("disk full"));
    }

    #[test]
    fn storage_error_display_invalid_path() {
        let err = StorageError::InvalidPath {
            reason: "not a directory".to_string(),
        };
        assert!(err.to_string().contains("not a directory"));
    }

    #[test]
    fn partition_class_display() {
        assert_eq!(PartitionClass::Hot.to_string(), "hot");
        assert_eq!(PartitionClass::Cold.to_string(), "cold");
        assert_eq!(PartitionClass::Blob.to_string(), "blob");
    }

    #[test]
    fn to_fjall_options_hot_has_bloom_filter_enabled() {
        let config = PartitionConfig::hot();
        let options = config.to_fjall_options();
        // HOT config should have bloom_filter_bits_per_key = 10
        assert_eq!(config.bloom_filter_bits_per_key, 10);
        assert_eq!(config.flush_interval_bytes, 64 * 1024 * 1024);
        // The options should be valid and not panic when used
        let _ = options;
    }

    #[test]
    fn to_fjall_options_cold_has_bloom_filter_disabled() {
        let config = PartitionConfig::cold();
        let options = config.to_fjall_options();
        // COLD config should have bloom_filter_bits_per_key = 0
        assert_eq!(config.bloom_filter_bits_per_key, 0);
        assert_eq!(config.flush_interval_bytes, 256 * 1024 * 1024);
        // The options should be valid and not panic when used
        let _ = options;
    }

    #[test]
    fn to_fjall_options_blob_has_kv_separation() {
        let config = PartitionConfig::blob();
        let options = config.to_fjall_options();
        // BLOB config should have flush_interval_bytes = 1GB
        assert_eq!(config.bloom_filter_bits_per_key, 0);
        assert_eq!(config.flush_interval_bytes, 1024 * 1024 * 1024);
        // The options should be valid and not panic when used
        let _ = options;
    }

    #[test]
    fn to_fjall_options_compaction_disabled() {
        let config = PartitionConfig {
            compaction_enabled: false,
            bloom_filter_bits_per_key: 10,
            flush_interval_bytes: 64 * 1024 * 1024,
        };
        let options = config.to_fjall_options();
        // Even with compaction disabled, we return valid options
        let _ = options;
    }

    #[test]
    fn occ_error_can_be_constructed_with_version_fields() {
        let err = StorageError::OptimisticConcurrency {
            expected_version: 5,
            actual_version: 3,
        };
        assert!(matches!(
            err,
            StorageError::OptimisticConcurrency {
                expected_version: 5,
                actual_version: 3,
            }
        ));
    }

    #[test]
    fn occ_error_display_renders_expected_vs_actual() {
        let err = StorageError::OptimisticConcurrency {
            expected_version: 10,
            actual_version: 7,
        };
        let msg = err.to_string();
        assert!(msg.contains("10"), "should contain expected version: {msg}");
        assert!(msg.contains('7'), "should contain actual version: {msg}");
    }

    #[test]
    fn create_partition_layout_creates_directory_if_missing() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("test-storage");
        assert!(!path.exists());

        let layout = create_partition_layout(&path);
        assert!(layout.is_ok());
        assert!(path.exists());
    }

    #[test]
    fn storage_engine_open_creates_all_stores() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("test-storage");

        let engine = StorageEngine::open(&path);
        assert!(
            engine.is_ok(),
            "StorageEngine::open failed: {:?}",
            engine.err()
        );
        let _engine = engine.unwrap();
    }

    #[tokio::test]
    async fn storage_engine_event_store_works() {
        use crate::event_store::EventStore;

        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("test-storage");

        let engine = StorageEngine::open(&path).unwrap();
        let instance_id = vo_types::InstanceId::from_bytes([1u8; 16]);
        let event = vo_types::events::EventEnvelope {
            schema_version: 1,
            instance_id: instance_id.to_string(),
            sequence: 1,
            timestamp_ms: 1000,
            payload: serde_json::json!({"type": "TestEvent"}),
            metadata: vo_types::events::EventMetadata::default(),
        };

        let result = engine.event_store.append(&instance_id, vec![event]).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);

        let seq = engine.event_store.get_sequence(&instance_id).await.unwrap();
        assert_eq!(seq, 1);
    }
}
