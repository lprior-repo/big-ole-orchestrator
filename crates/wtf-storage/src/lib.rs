//! wtf-storage — NATS JetStream event log, NATS KV materialized view, sled snapshot cache.
//!
//! Source of truth: NATS JetStream (ADR-013).
//! Query side: NATS KV (ADR-014).
//! Snapshot cache: sled, single `snapshots` tree (ADR-019).

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

pub mod journal;
pub mod kv;
pub mod nats;
pub mod provision;
pub mod replay;
pub mod snapshots;

// Legacy sled modules — kept for reference, migrated to above modules.
pub mod db;
pub mod instances;
pub mod signals;
pub mod timers;

pub use journal::{append_event, build_subject};
pub use kv::{
    bucket_names, definition_key, delete_heartbeat, heartbeat_key, instance_key,
    provision_kv_buckets, write_heartbeat, KvStores,
};
pub use nats::{connect, NatsClient, NatsConfig};
pub use provision::{provision_streams, stream_names, subjects, verify_streams};
pub use replay::{
    create_replay_consumer, replay_start_seq, ReplayBatch, ReplayConfig, ReplayConsumer,
    ReplayedEvent,
};
pub use snapshots::{
    delete_snapshot, open_snapshot_db, read_snapshot, write_snapshot, SnapshotRecord,
};
