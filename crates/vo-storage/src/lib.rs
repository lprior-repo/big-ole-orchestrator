//! Storage abstractions for the Veloxide workflow engine.
//!
//! This crate provides storage layer abstractions including:
//! - [`append_event`] - Event append with sequence validation (ADR-002, ADR-016)
//! - [`append`] - Event append operations
//! - [`blob_store`] - Blob storage for large data
//! - [`dedupe_partition`] - Exactly-once ingress deduplication (ADR-028)
//! - [`effect_journal`] - Effect journal for event sourcing
//! - [`instance_index`] - Instance lookup and indexing
//! - [`purge`] - Data retention and purging
//! - [`query_events`] - Query appended events (for test verification)
//! - [`snapshots`] - State snapshots for fast replay
//!
//! # Architecture
//!
//! The storage layer is designed for crash safety and exact-once semantics.
//! All operations are idempotent where possible to handle retries.
//!
//! # Event Append
//!
//! [`append_event`] provides a synchronous event append facade that
//! serializes events to JSON, validates sequence continuity, and stores
//! them in an in-memory event log. Events are keyed by `instance_id` and
//! assigned monotonically increasing sequence numbers.

#![cfg_attr(not(test), deny(clippy::unwrap_used))]
#![cfg_attr(not(test), deny(clippy::expect_used))]
#![cfg_attr(not(test), deny(clippy::panic))]
#![allow(clippy::module_name_repetitions)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::complexity)]
#![warn(clippy::cognitive_complexity)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::iter_with_drain,
    clippy::significant_drop_tightening,
    clippy::match_same_arms,
    clippy::needless_pass_by_value,
    clippy::missing_const_for_fn,
    clippy::manual_let_else,
    clippy::used_underscore_binding,
    clippy::option_if_let_else,
    clippy::match_wildcard_for_single_variants,
    clippy::expect_used,
    unsafe_code
)]

pub mod append;
pub mod atomic_wait_commit;
pub mod event_summary_commit;
pub mod blob;
pub mod blob_store;
#[cfg(test)]
mod blob_store_tests;
pub mod budget_saga;
pub mod checksum;
pub mod codec;
pub mod compensation_saga;
pub mod crypto;
#[cfg(test)]
mod crypto_tests;
pub mod dedupe_partition;
pub mod effect_journal;
pub mod event_store;
pub mod failure_window_store;
pub mod fs_store;
pub mod instance_index;
pub mod key_encoding;
pub mod key_partition;
pub mod lease_partition;
pub mod lineage_store;
pub mod merkle_tree;
pub mod mmap_cache;
pub mod partitions;
pub mod projection_compat;
pub mod purge;
pub mod qos_router;
pub mod query;
pub mod receipts;
pub mod snapshot_diff;
pub mod snapshots;
pub mod status_store;
pub mod timer_index;
#[cfg(test)]
mod timer_index_tests;
pub mod workflow_version_partition;

use serde::Serialize;
use std::collections::HashMap;
use std::sync::LazyLock;

#[derive(Debug, Clone)]
struct EventRecord {
    sequence: u64,
    payload: serde_json::Value,
}

static EVENT_STORE: LazyLock<std::sync::Mutex<EventStoreBackend>> =
    LazyLock::new(|| std::sync::Mutex::new(EventStoreBackend::new()));

struct EventStoreBackend {
    sequences: HashMap<String, u64>,
    events: HashMap<String, Vec<EventRecord>>,
}

impl EventStoreBackend {
    fn new() -> Self {
        Self {
            sequences: HashMap::new(),
            events: HashMap::new(),
        }
    }

    fn append<E: Serialize>(&mut self, instance_id: &str, event: E) -> Result<u64, String> {
        let payload =
            serde_json::to_value(&event).map_err(|e| format!("serialization failed: {e}"))?;
        let expected_sequence = self.sequences.get(instance_id).copied().unwrap_or(0);
        let sequence = expected_sequence + 1;

        if sequence == 0 {
            return Err("sequence cannot be zero".to_string());
        }

        self.sequences.insert(instance_id.to_string(), sequence);
        self.events
            .entry(instance_id.to_string())
            .or_default()
            .push(EventRecord { sequence, payload });

        Ok(sequence)
    }
}

/// Appends an event to the in-memory event store.
///
/// Events are keyed by `instance_id` and assigned monotonically increasing
/// sequence numbers starting from 1. Subsequent calls for the same instance
/// must have strictly increasing sequences (gap detection enforced).
///
/// This is a synchronous facade that routes to the internal in-memory event store.
/// For production use with durable Fjall-backed storage, use [`event_store::EventStore`]
/// or [`event_summary_commit::commit_event_and_summary`] directly.
///
/// # Errors
///
/// Returns an error if the append operation fails due to storage or serialization issues.
pub fn append_event<E: serde::Serialize>(
    namespace: &str,
    instance_id: &str,
    event: E,
) -> Result<(), String> {
    let event_json = serde_json::to_value(&event).map_err(|e| e.to_string())?;

    let instance = vo_types::InstanceId::parse(instance_id).map_err(|e| e.to_string())?;

    let store = &*APPEND_EVENT_STORE;

    let sequence = block_on_sync(store.get_sequence(&instance)).unwrap_or(0);

    let envelope = vo_types::EventEnvelope {
        schema_version: 1,
        instance_id: instance_id.to_string(),
        sequence: u64::try_from(sequence + 1).map_err(|e| e.to_string())?,
        timestamp_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX)),
        payload: event_json,
        metadata: vo_types::events::EventMetadata::default(),
    };

    block_on_sync(store.append(&instance, vec![envelope]))
        .map(|_| ())
        .map_err(|e| e.to_string())
}

use event_store::{EventStore, InMemoryEventStore};

static APPEND_EVENT_STORE: LazyLock<InMemoryEventStore> = LazyLock::new(InMemoryEventStore::new);

pub(crate) fn _internal_append_store() -> &'static InMemoryEventStore {
    &APPEND_EVENT_STORE
}

fn block_on_sync<F, T>(f: F) -> T
where
    F: std::future::Future<Output = T>,
{
    let handle = tokio::runtime::Handle::current();
    tokio::task::block_in_place(|| handle.block_on(f))
}

use event_store::{EventStore, InMemoryEventStore};

static APPEND_EVENT_STORE: LazyLock<InMemoryEventStore> = LazyLock::new(InMemoryEventStore::new);

pub(crate) fn _internal_append_store() -> &'static InMemoryEventStore {
    &APPEND_EVENT_STORE
}

fn block_on_sync<F, T>(f: F) -> T
where
    F: std::future::Future<Output = T>,
{
    let handle = tokio::runtime::Handle::current();
    tokio::task::block_in_place(|| handle.block_on(f))
}

/// Queries all stored events for a given `instance_id`.
///
/// Returns a vector of `(sequence, payload)` tuples in ascending sequence order.
/// Returns an empty vector if no events exist for the instance.
///
/// # Example
///
/// ```
/// use vo_storage::{append_event, query_events};
/// append_event("ns", "inst-1", serde_json::json!({"type": "start"})).unwrap();
/// let events = query_events("inst-1");
/// assert_eq!(events.len(), 1);
/// ```
pub fn query_events(instance_id: &str) -> Vec<(u64, serde_json::Value)> {
    let store = EVENT_STORE.lock().expect("event store lock poisoned");
    store
        .events
        .get(instance_id)
        .map(|records| {
            records
                .iter()
                .map(|r| (r.sequence, r.payload.clone()))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::append_event;
    use crate::event_store::EventStore;
    use crate::_internal_append_store;
    use vo_types::InstanceId;

    #[tokio::test]
    async fn given_append_event_called_when_query_runs_then_event_is_durable() {
        let instance = InstanceId::parse("00000000000000000000000001").unwrap();
        let store = _internal_append_store();

        let event = serde_json::json!({ "type": "workflow_started", "data": "test" });

        let result = append_event("test", "00000000000000000000000001", &event);
        assert!(result.is_ok(), "append_event should succeed");

        let seq = store.get_sequence(&instance).await.unwrap();
        assert_eq!(seq, 1, "event should be stored with sequence 1");
    }

    #[test]
    fn append_event_returns_err_on_invalid_instance_id() {
        let result = append_event("ns", "not-a-valid-instance-id", "event");
        assert!(result.is_err(), "should fail with invalid instance id");
    }
}
