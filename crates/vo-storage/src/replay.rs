//! Snapshot-aware replay fetch logic.
//!
//! Architecture: Data (`ReplayError`, `ReplayFetcher`) → Calc
//! (`fetch_snapshot_and_events`, `start_sequence_from_snapshot`) → Actions
//! (`ReplayFetcher` coordinating dual-reads).
//!
//! ## Problem
//!
//! When replaying events for an instance, we want to optimize by first loading
//! the most recent snapshot and then replaying only events after that point,
//! rather than replaying all events from version 0.
//!
//! ## Solution
//!
//! 1. **Snapshot-first**: Attempt to load the most recent snapshot.
//! 2. **Event replay from snapshot**: If snapshot exists at version N, replay
//!    events starting from N+1.
//! 3. **Fallback on corruption**: If snapshot load fails structurally (e.g.,
//!    checksum mismatch), fall back to full zero-version event replay.
//!
//! ## Usage
//!
//! ```ignore
//! let fetcher = ReplayFetcher::new(snapshot_store, event_store);
//! let result = fetcher.fetch_snapshot_and_events(&instance_id).await;
//! match result {
//!     Ok((snapshot, events)) => { /* use snapshot state and events */ }
//!     Err(ReplayError::NoSnapshot { events }) => { /* no snapshot, use events from v0 */ }
//!     Err(e) => { /* handle error */ }
//! }
//! ```

use vo_types::state::InstanceState;
use vo_types::InstanceId;

pub use crate::codec::StorageError;

#[derive(Debug, Clone, PartialEq)]
pub struct ReplayResult {
    pub snapshot_version: u64,
    pub state: InstanceState,
    pub events: Vec<FetchedEvent>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FetchedEvent {
    pub sequence: u64,
    pub envelope: vo_types::EventEnvelope,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ReplayError {
    #[error("snapshot load failed: {reason}")]
    SnapshotLoadFailed { reason: String },
    #[error("event fetch failed: {reason}")]
    EventFetchFailed { reason: String },
    #[error("no snapshot available, falling back to event replay")]
    NoSnapshot { events: Vec<FetchedEvent> },
}

impl From<StorageError> for ReplayError {
    fn from(e: StorageError) -> Self {
        match e {
            StorageError::DeserializationFailed => Self::SnapshotLoadFailed {
                reason: "deserialization failed (possible corruption)".to_string(),
            },
            StorageError::ChecksumMismatch => Self::SnapshotLoadFailed {
                reason: "checksum mismatch (possible corruption)".to_string(),
            },
            _ => Self::SnapshotLoadFailed {
                reason: format!("{e:?}"),
            },
        }
    }
}

pub trait SnapshotReader {
    /// Loads the latest snapshot for the given instance.
    ///
    /// # Errors
    ///
    /// Returns `StorageError` if the load operation fails.
    fn load_latest(
        &self,
        instance_id: &InstanceId,
    ) -> Result<Option<(u64, InstanceState)>, StorageError>;
}

pub trait EventStore {
    type EventIterator: Iterator<Item = Result<vo_types::EventEnvelope, StorageError>>;
    fn replay_events(&self, instance_id: &InstanceId, start_sequence: u64) -> Self::EventIterator;
}

pub struct ReplayFetcher<S, E>
where
    S: SnapshotReader,
    E: EventStore,
{
    snapshot_store: S,
    event_store: E,
}

impl<S, E> ReplayFetcher<S, E>
where
    S: SnapshotReader,
    E: EventStore,
{
    #[must_use]
    pub const fn new(snapshot_store: S, event_store: E) -> Self {
        Self {
            snapshot_store,
            event_store,
        }
    }

    /// Fetches the latest snapshot and subsequent events for an instance.
    ///
    /// # Errors
    ///
    /// Returns `ReplayError::SnapshotLoadFailed` if snapshot loading fails.
    /// Returns `ReplayError::NoSnapshot` if no snapshot exists (with events from v0).
    /// Returns `ReplayError::EventFetchFailed` if event fetching fails.
    pub fn fetch_snapshot_and_events(
        &self,
        instance_id: &InstanceId,
    ) -> Result<ReplayResult, ReplayError> {
        match self.snapshot_store.load_latest(instance_id) {
            Ok(Some((snapshot_version, state))) => {
                let start_seq = snapshot_version.saturating_add(1);
                let events = self.collect_events(instance_id, start_seq)?;
                Ok(ReplayResult {
                    snapshot_version,
                    state,
                    events,
                })
            }
            Ok(None) => {
                let events = self.collect_events(instance_id, 0)?;
                Err(ReplayError::NoSnapshot { events })
            }
            Err(e) => {
                let fallback_events = self.collect_events(instance_id, 0)?;
                if fallback_events.is_empty() {
                    return Err(ReplayError::SnapshotLoadFailed {
                        reason: format!("{e:?}"),
                    });
                }
                Err(ReplayError::NoSnapshot {
                    events: fallback_events,
                })
            }
        }
    }

    fn collect_events(
        &self,
        instance_id: &InstanceId,
        start_sequence: u64,
    ) -> Result<Vec<FetchedEvent>, ReplayError> {
        let iter = self.event_store.replay_events(instance_id, start_sequence);
        let events: Vec<FetchedEvent> = iter
            .map(|result| {
                result.map(|envelope| FetchedEvent {
                    sequence: envelope.sequence,
                    envelope,
                })
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| ReplayError::EventFetchFailed {
                reason: format!("{e:?}"),
            })?;
        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mock::{MockEventStore, MockSnapshotReader};

    fn make_instance_id() -> InstanceId {
        InstanceId::from_bytes([1u8; 16])
    }

    fn make_state(counter: u64) -> InstanceState {
        InstanceState { counter }
    }

    #[test]
    fn test_successfully_fetches_snapshot_at_v5_then_events_v6_10() {
        let instance_id = make_instance_id();
        let mock_snapshot = MockSnapshotReader::new().with_snapshot(5, make_state(100));
        let mock_events = MockEventStore::new().with_events(vec![6, 7, 8, 9, 10]);
        let fetcher = ReplayFetcher::new(mock_snapshot, mock_events);

        let result = fetcher.fetch_snapshot_and_events(&instance_id);

        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.snapshot_version, 5);
        assert_eq!(result.state.counter, 100);
        assert_eq!(result.events.len(), 5);
        assert_eq!(result.events[0].sequence, 6);
        assert_eq!(result.events[4].sequence, 10);
    }

    #[test]
    fn test_if_snapshot_returns_none_seamlessly_fetches_events_v0_10() {
        let instance_id = make_instance_id();
        let mock_snapshot = MockSnapshotReader::new();
        let mock_events = MockEventStore::new().with_events(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        let fetcher = ReplayFetcher::new(mock_snapshot, mock_events);

        let result = fetcher.fetch_snapshot_and_events(&instance_id);

        assert!(result.is_err());
        match result {
            Err(ReplayError::NoSnapshot { events }) => {
                assert_eq!(events.len(), 10);
                assert_eq!(events[0].sequence, 1);
                assert_eq!(events[9].sequence, 10);
            }
            _ => panic!("expected NoSnapshot error"),
        }
    }

    #[test]
    fn test_snapshot_corruption_falls_back_to_full_replay() {
        let instance_id = make_instance_id();
        let mock_snapshot =
            MockSnapshotReader::new().with_error(StorageError::DeserializationFailed);
        let mock_events = MockEventStore::new().with_events(vec![1, 2, 3]);
        let fetcher = ReplayFetcher::new(mock_snapshot, mock_events);

        let result = fetcher.fetch_snapshot_and_events(&instance_id);

        assert!(result.is_err());
        match result {
            Err(ReplayError::NoSnapshot { events }) => {
                assert_eq!(events.len(), 3);
            }
            _ => panic!("expected NoSnapshot error"),
        }
    }

    #[test]
    fn test_empty_event_store_after_snapshot() {
        let instance_id = make_instance_id();
        let mock_snapshot = MockSnapshotReader::new().with_snapshot(5, make_state(100));
        let mock_events = MockEventStore::new().with_events(vec![]);
        let fetcher = ReplayFetcher::new(mock_snapshot, mock_events);

        let result = fetcher.fetch_snapshot_and_events(&instance_id);

        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.snapshot_version, 5);
        assert_eq!(result.events.len(), 0);
    }

    #[test]
    fn test_replay_fetcher_new() {
        let mock_snapshot = MockSnapshotReader::new();
        let mock_events = MockEventStore::new();
        let fetcher = ReplayFetcher::new(mock_snapshot, mock_events);
        assert!(fetcher
            .snapshot_store
            .load_latest(&make_instance_id())
            .unwrap()
            .is_none());
    }
}

#[cfg(test)]
mod mock {
    use super::*;
    use std::cell::RefCell;

    pub struct MockSnapshotReader {
        snapshot: Option<(u64, InstanceState)>,
        error: Option<StorageError>,
    }

    impl MockSnapshotReader {
        pub fn new() -> Self {
            Self {
                snapshot: None,
                error: None,
            }
        }

        pub fn with_snapshot(mut self, version: u64, state: InstanceState) -> Self {
            self.snapshot = Some((version, state));
            self
        }

        pub fn with_error(mut self, error: StorageError) -> Self {
            self.error = Some(error);
            self
        }
    }

    impl Default for MockSnapshotReader {
        fn default() -> Self {
            Self::new()
        }
    }

    impl SnapshotReader for MockSnapshotReader {
        fn load_latest(
            &self,
            _instance_id: &InstanceId,
        ) -> Result<Option<(u64, InstanceState)>, StorageError> {
            if let Some(ref e) = self.error {
                return Err(e.clone());
            }
            Ok(self.snapshot.clone())
        }
    }

    pub struct MockEventStore {
        events: RefCell<Vec<vo_types::EventEnvelope>>,
    }

    impl MockEventStore {
        pub fn new() -> Self {
            Self {
                events: RefCell::new(vec![]),
            }
        }

        pub fn with_events(mut self, sequences: Vec<u64>) -> Self {
            let envelopes = sequences
                .into_iter()
                .map(|seq| vo_types::EventEnvelope {
                    schema_version: 1,
                    instance_id: "test-instance".to_string(),
                    sequence: seq,
                    timestamp_ms: 1000 + seq,
                    payload: serde_json::json!({"type": "TestEvent"}),
                    metadata: vo_types::events::metadata::EventMetadata::default(),
                })
                .collect();
            self.events = RefCell::new(envelopes);
            self
        }
    }

    impl Default for MockEventStore {
        fn default() -> Self {
            Self::new()
        }
    }

    impl EventStore for MockEventStore {
        type EventIterator = MockEventIterator;

        fn replay_events(
            &self,
            _instance_id: &InstanceId,
            _start_sequence: u64,
        ) -> Self::EventIterator {
            MockEventIterator {
                events: self.events.borrow().clone(),
                index: 0,
            }
        }
    }

    pub struct MockEventIterator {
        events: Vec<vo_types::EventEnvelope>,
        index: usize,
    }

    impl Iterator for MockEventIterator {
        type Item = Result<vo_types::EventEnvelope, StorageError>;

        fn next(&mut self) -> Option<Self::Item> {
            if self.index < self.events.len() {
                let event = self.events[self.index].clone();
                self.index += 1;
                Some(Ok(event))
            } else {
                None
            }
        }
    }
}
