//! Event replay query engine — Data → Calc → Actions layering.
//!
//! Lineage-aware query routing (ADR-038, ADR-042): [`LineageQuery`] supports
//! instance-id, lineage-wide, and epoch-specific range scans.

pub use crate::codec::StorageError;
use vo_types::{Epoch, EventEnvelope, EventError, InstanceId};

pub mod lineage;
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
/// Returns `StorageError::InvalidArgument` if the instance ID exceeds 255 bytes or contains null bytes.
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

/// Produce the lineage prefix bytes for range-scanning.
/// Returns `StorageError::InvalidArgument` if the lineage ID is empty, too long, or contains null bytes.
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

/// Produce the epoch-specific prefix bytes for range-scanning.
/// Returns `StorageError::InvalidArgument` if the lineage ID is invalid.
pub fn epoch_prefix_generator(lineage_id: &str, epoch: Epoch) -> Result<Vec<u8>, StorageError> {
    let lineage_prefix = lineage_prefix_generator(lineage_id)?;
    let epoch_bytes = epoch.get().to_be_bytes();
    let mut prefix = lineage_prefix;
    prefix.extend_from_slice(&epoch_bytes);
    Ok(prefix)
}

impl LineageQuery<'_> {
    /// Converts this query into a prefix byte vector for range scanning.
    /// Returns `StorageError::InvalidArgument` if any component is invalid.
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
/// Intentionally collapses errors: `UnsupportedVersion` for version mismatches, `CorruptEventPayload` otherwise.
#[must_use]
pub const fn error_mapper(error: &EventError) -> StorageError {
    match error {
        EventError::UnsupportedEnvelopeVersion(_) => StorageError::UnsupportedVersion,
        _ => StorageError::CorruptEventPayload,
    }
}

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

pub struct EventReplayIterator {
    state: IteratorState,
    inner: Option<Box<dyn DoubleEndedIterator<Item = fjall::Guard>>>,
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
            Some(guard) => {
                if let Ok((k_bytes, v_bytes)) = guard.into_inner() {
                    self.process_kv(&k_bytes, &v_bytes)
                } else {
                    self.inner = None;
                    Some(Err(StorageError::Storage))
                }
            }
            None => None,
        }
    }
}

impl EventReplayIterator {
    pub(crate) fn error(err: StorageError) -> Self {
        Self {
            state: IteratorState::new(),
            inner: None,
            init_error: Some(err),
        }
    }

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
pub fn replay_events_by_prefix(keyspace: &fjall::Database, prefix: Vec<u8>) -> EventReplayIterator {
    let Ok(partition) = keyspace.keyspace("events", fjall::KeyspaceCreateOptions::default) else {
        return EventReplayIterator::error(StorageError::Storage);
    };
    let Ok(min_seq) = encode_key(1) else {
        return EventReplayIterator::error(StorageError::Storage);
    };
    let Ok(max_seq) = encode_key(u64::MAX) else {
        return EventReplayIterator::error(StorageError::Storage);
    };
    let mut start = prefix.clone();
    start.extend_from_slice(&min_seq);
    let mut end = prefix;
    end.extend_from_slice(&max_seq);
    EventReplayIterator {
        state: IteratorState::new(),
        inner: Some(Box::new(partition.range(start..=end))),
        init_error: None,
    }
}

#[must_use]
pub fn replay_events(keyspace: &fjall::Database, instance_id: &InstanceId) -> EventReplayIterator {
    match prefix_generator(instance_id) {
        Ok(prefix) => replay_events_by_prefix(keyspace, prefix),
        Err(e) => EventReplayIterator::error(e),
    }
}

pub use lineage::{replay_events_for_lineage, LineageReplayIterator};
