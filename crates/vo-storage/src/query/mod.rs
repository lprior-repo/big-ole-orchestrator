//! Event replay query engine — pure key/encode/decode functions + stateful iterator.
//!
//! Architecture: Data (`StorageError`, `IteratorState`) → Calc (`encode_key`, `decode_key`,
//! `prefix_generator`, `error_mapper`) → Actions (`EventReplayIterator`, `replay_events`).
//!
//! ## Lineage-Aware Query Routing (ADR-038, ADR-042)
//!
//! Workflows may perform continue-as-new, creating new execution epochs while maintaining
//! a stable lineage_id. Lineage-aware query routing enables:
//!
//! - **Lineage-wide queries**: Retrieve all events across all epochs of a lineage
//! - **Epoch-specific queries**: Retrieve events for a specific epoch within a lineage
//!
//! The routing is determined by [`LineageQuery`] which specifies whether to query
//! by instance_id directly, or by lineage_id (+ optional epoch).

pub use crate::codec::StorageError;
use vo_types::{Epoch, EventEnvelope, EventError, InstanceId};

pub mod optimizer;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineageQuery<'a> {
    InstanceId(&'a InstanceId),
    LineageWide { lineage_id: &'a str },
    EpochSpecific { lineage_id: &'a str, epoch: Epoch },
}

// ---------------------------------------------------------------------------
// Calc layer — pure functions
// ---------------------------------------------------------------------------

/// Encode a sequence number as big-endian bytes.
///
/// # Errors
///
/// Returns `StorageError::InvalidArgument` if `sequence` is zero.
#[must_use = "encode_key performs a pure encoding computation"]
pub const fn encode_key(sequence: u64) -> Result<[u8; 8], StorageError> {
    if sequence == 0 {
        return Err(StorageError::InvalidArgument);
    }
    Ok(sequence.to_be_bytes())
}

/// Decode a big-endian 8-byte slice into a sequence number.
///
/// # Errors
///
/// Returns `StorageError::Storage` if the slice is not exactly 8 bytes.
/// Returns `StorageError::InvalidArgument` if the slice decodes to zero.
pub fn decode_key(bytes: &[u8]) -> Result<u64, StorageError> {
    let arr: [u8; 8] = bytes.try_into().map_err(|_| StorageError::Storage)?;
    let seq = u64::from_be_bytes(arr);
    if seq == 0 {
        return Err(StorageError::InvalidArgument);
    }
    Ok(seq)
}

/// Produce the prefix bytes for range-scanning a given instance.
///
/// Accepts the domain `InstanceId` type directly — callers should not
/// pre-extract the string representation.
///
/// # Errors
///
/// Returns `StorageError::InvalidArgument` if the instance ID exceeds 255 bytes.
/// Returns `StorageError::InvalidArgument` if the instance ID contains null bytes.
pub fn prefix_generator(instance_id: &InstanceId) -> Result<Vec<u8>, StorageError> {
    let id_str = instance_id.as_str();
    if id_str.len() > 255 {
        return Err(StorageError::InvalidArgument);
    }
    if id_str.as_bytes().contains(&b'\0') {
        return Err(StorageError::InvalidArgument);
    }
    Ok(id_str.as_bytes().to_vec())
}

pub const LINEAGE_ID_NULL_BYTE: u8 = 0xFF;
pub const LINEAGE_ID_MAX_LEN: usize = 255;

pub fn lineage_prefix_generator(lineage_id: &str) -> Result<Vec<u8>, StorageError> {
    if lineage_id.is_empty() {
        return Err(StorageError::InvalidArgument);
    }
    if lineage_id.len() > LINEAGE_ID_MAX_LEN {
        return Err(StorageError::InvalidArgument);
    }
    if lineage_id.as_bytes().contains(&b'\0') {
        return Err(StorageError::InvalidArgument);
    }
    let mut prefix = Vec::with_capacity(1 + lineage_id.len() + 1);
    prefix.push(LINEAGE_ID_NULL_BYTE);
    prefix.extend_from_slice(lineage_id.as_bytes());
    prefix.push(LINEAGE_ID_NULL_BYTE);
    Ok(prefix)
}

pub fn epoch_prefix_generator(lineage_id: &str, epoch: Epoch) -> Result<Vec<u8>, StorageError> {
    let lineage_prefix = lineage_prefix_generator(lineage_id)?;
    let epoch_bytes = epoch.0.to_be_bytes();
    let mut prefix = lineage_prefix;
    prefix.extend_from_slice(&epoch_bytes);
    Ok(prefix)
}

impl<'a> LineageQuery<'a> {
    pub fn to_prefix(&self) -> Result<Vec<u8>, StorageError> {
        match self {
            LineageQuery::InstanceId(instance_id) => prefix_generator(instance_id),
            LineageQuery::LineageWide { lineage_id } => lineage_prefix_generator(lineage_id),
            LineageQuery::EpochSpecific { lineage_id, epoch } => {
                epoch_prefix_generator(lineage_id, *epoch)
            }
        }
    }
}

/// Map an envelope decode error into the storage-layer replay taxonomy.
///
/// ## Why this intentionally collapses errors
///
/// `replay_events` is a storage-boundary API. Its responsibility is to:
/// - read bytes from storage,
/// - recover an `EventEnvelope`, and
/// - stop replay when storage ordering or envelope validity is violated.
///
/// At this layer we intentionally do **not** preserve every fine-grained
/// `EventError` variant. For replay callers, the actionable distinction is:
/// - `UnsupportedVersion`: the envelope is well-formed but from an unsupported version.
/// - `CorruptEventPayload`: the stored envelope bytes are malformed or incomplete.
///
/// This keeps the replay API stable while still distinguishing the only versioning
/// concern that callers can reasonably react to differently.
#[must_use]
pub const fn error_mapper(error: &EventError) -> StorageError {
    match error {
        EventError::UnsupportedEnvelopeVersion(_) => StorageError::UnsupportedVersion,
        _ => StorageError::CorruptEventPayload,
    }
}

// ---------------------------------------------------------------------------
// Data layer — iterator state machine
// ---------------------------------------------------------------------------

pub struct IteratorState {
    expected: Option<u64>,
    started: bool,
}

impl Default for IteratorState {
    fn default() -> Self {
        Self::new()
    }
}

impl IteratorState {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            expected: None,
            started: false,
        }
    }

    pub fn advance(
        &mut self,
        found: u64,
        record: EventEnvelope,
    ) -> Option<Result<EventEnvelope, StorageError>> {
        if found == 0 {
            return Some(Err(StorageError::InvalidArgument));
        }
        if !self.started {
            self.started = true;
            self.expected = found.checked_add(1);
            return Some(Ok(record));
        }
        match self.expected {
            Some(expected) if found != expected => Some(Err(StorageError::SequenceGap)),
            Some(expected) => {
                self.expected = expected.checked_add(1);
                Some(Ok(record))
            }
            None => Some(Err(StorageError::SequenceGap)),
        }
    }
}

// ---------------------------------------------------------------------------
// Actions layer — iterator + constructor
// ---------------------------------------------------------------------------

pub struct EventReplayIterator {
    state: IteratorState,
    inner: Option<Box<dyn DoubleEndedIterator<Item = fjall::Result<fjall::KvPair>>>>,
    init_error: Option<StorageError>,
}

impl Iterator for EventReplayIterator {
    type Item = Result<EventEnvelope, StorageError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(err) = self.init_error.take() {
            return Some(Err(err));
        }
        let Some(inner) = &mut self.inner else {
            return None;
        };
        match inner.next() {
            Some(Ok((k_bytes, v_bytes))) => self.process_kv(&k_bytes, &v_bytes),
            Some(Err(_)) => {
                self.inner = None;
                Some(Err(StorageError::Storage))
            }
            None => None,
        }
    }
}

impl EventReplayIterator {
    fn process_kv(
        &mut self,
        k_bytes: &fjall::Slice,
        v_bytes: &fjall::Slice,
    ) -> Option<Result<EventEnvelope, StorageError>> {
        let seq_len: usize = 8;
        if k_bytes.len() < seq_len {
            self.inner = None;
            return Some(Err(StorageError::Storage));
        }
        let seq_bytes = &k_bytes[k_bytes.len() - seq_len..];
        let found_seq = match decode_key(seq_bytes) {
            Ok(s) => s,
            Err(e) => {
                self.inner = None;
                return Some(Err(e));
            }
        };
        let envelope = match EventEnvelope::from_bytes(v_bytes) {
            Ok(e) => e,
            Err(EventError::UnsupportedEnvelopeVersion(_)) => {
                self.inner = None;
                return Some(Err(StorageError::UnsupportedVersion));
            }
            Err(_) => {
                self.inner = None;
                return Some(Err(StorageError::CorruptEventPayload));
            }
        };
        match self.state.advance(found_seq, envelope) {
            Some(Err(e)) => {
                self.inner = None;
                Some(Err(e))
            }
            Some(Ok(env)) => Some(Ok(env)),
            None => None,
        }
    }
}

#[must_use]
pub fn replay_events(keyspace: &fjall::Keyspace, instance_id: &InstanceId) -> EventReplayIterator {
    let prefix = match prefix_generator(instance_id) {
        Ok(p) => p,
        Err(e) => {
            return EventReplayIterator {
                state: IteratorState::new(),
                inner: None,
                init_error: Some(e),
            };
        }
    };
    let Ok(partition) = keyspace.open_partition("events", fjall::PartitionCreateOptions::default())
    else {
        return EventReplayIterator {
            state: IteratorState::new(),
            inner: None,
            init_error: Some(StorageError::Storage),
        };
    };
    let Ok(min_seq) = encode_key(1) else {
        return EventReplayIterator {
            state: IteratorState::new(),
            inner: None,
            init_error: Some(StorageError::Storage),
        };
    };
    let Ok(max_seq) = encode_key(u64::MAX) else {
        return EventReplayIterator {
            state: IteratorState::new(),
            inner: None,
            init_error: Some(StorageError::Storage),
        };
    };
    let mut start = prefix.clone();
    start.extend_from_slice(&min_seq);
    let mut end = prefix;
    end.extend_from_slice(&max_seq);
    let iter = partition.range(start..=end);
    EventReplayIterator {
        state: IteratorState::new(),
        inner: Some(Box::new(iter)),
        init_error: None,
    }
}

#[allow(dead_code)]
pub struct LineageReplayIterator {
    instance_iter: Option<EventReplayIterator>,
    lineage_id: Option<String>,
    epoch: Option<Epoch>,
}

impl Iterator for LineageReplayIterator {
    type Item = Result<EventEnvelope, StorageError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(ref mut iter) = self.instance_iter {
            iter.next()
        } else {
            None
        }
    }
}

#[must_use]
pub fn replay_events_for_lineage(
    keyspace: &fjall::Keyspace,
    query: &LineageQuery,
) -> LineageReplayIterator {
    match query {
        LineageQuery::InstanceId(instance_id) => {
            let iter = replay_events(keyspace, instance_id);
            LineageReplayIterator {
                instance_iter: Some(iter),
                lineage_id: None,
                epoch: None,
            }
        }
        LineageQuery::LineageWide { lineage_id: _ } => LineageReplayIterator {
            instance_iter: None,
            lineage_id: Some(
                query
                    .to_prefix()
                    .map(|p| String::from_utf8_lossy(&p).to_string())
                    .unwrap_or_default(),
            ),
            epoch: None,
        },
        LineageQuery::EpochSpecific {
            lineage_id: _,
            epoch: _,
        } => LineageReplayIterator {
            instance_iter: None,
            lineage_id: Some(
                query
                    .to_prefix()
                    .map(|p| String::from_utf8_lossy(&p).to_string())
                    .unwrap_or_default(),
            ),
            epoch: Some(match query {
                LineageQuery::EpochSpecific { epoch, .. } => *epoch,
                _ => Epoch::ZERO,
            }),
        },
    }
}
