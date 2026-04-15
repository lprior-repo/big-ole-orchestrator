use super::epoch_prefix_generator;
use super::IteratorState;
use super::LineageQuery;
use super::{decode_key, encode_key, lineage_prefix_generator, prefix_generator};
use crate::codec::StorageError;
use vo_types::{EventEnvelope, EventError};

pub struct LineageReplayIterator {
    inner: Option<Box<dyn DoubleEndedIterator<Item = fjall::Result<fjall::KvPair>>>>,
    state: IteratorState,
    prefix_len: usize,
    epoch_len: usize,
    last_epoch: Vec<u8>,
    init_error: Option<StorageError>,
}

impl LineageReplayIterator {
    fn error(err: StorageError) -> Self {
        Self {
            inner: None,
            state: IteratorState::new(),
            prefix_len: 0,
            epoch_len: 0,
            last_epoch: Vec::new(),
            init_error: Some(err),
        }
    }
}

impl Iterator for LineageReplayIterator {
    type Item = Result<EventEnvelope, StorageError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(err) = self.init_error.take() {
            return Some(Err(err));
        }
        let Some(inner) = &mut self.inner else {
            return None;
        };
        loop {
            match inner.next() {
                Some(Ok((k_bytes, v_bytes))) => {
                    let seq_len: usize = 8;
                    let min_key_len = self.prefix_len + self.epoch_len + seq_len;
                    if k_bytes.len() < min_key_len {
                        self.inner = None;
                        return Some(Err(StorageError::Storage));
                    }
                    let seq_start = self.prefix_len + self.epoch_len;
                    let seq_bytes = &k_bytes[seq_start..seq_start + seq_len];
                    let found_seq = match decode_key(seq_bytes) {
                        Ok(s) => s,
                        Err(e) => {
                            self.inner = None;
                            return Some(Err(e));
                        }
                    };
                    let envelope = match EventEnvelope::from_bytes(&v_bytes) {
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
                    if self.epoch_len > 0 && self.state.started {
                        let epoch_bytes =
                            &k_bytes[self.prefix_len..self.prefix_len + self.epoch_len];
                        if epoch_bytes != self.last_epoch.as_slice() {
                            self.state = IteratorState::new();
                            self.last_epoch = epoch_bytes.to_vec();
                        }
                    }
                    match self.state.advance(found_seq, envelope) {
                        Some(Err(e)) => {
                            self.inner = None;
                            return Some(Err(e));
                        }
                        Some(Ok(env)) => return Some(Ok(env)),
                        None => continue,
                    }
                }
                Some(Err(_)) => {
                    self.inner = None;
                    return Some(Err(StorageError::Storage));
                }
                None => return None,
            }
        }
    }
}

#[must_use]
pub fn replay_events_for_lineage(
    keyspace: &fjall::Keyspace,
    query: &LineageQuery,
) -> LineageReplayIterator {
    let (prefix, epoch_len) = match query {
        LineageQuery::InstanceId(instance_id) => match prefix_generator(instance_id) {
            Ok(p) => (p, 0),
            Err(e) => return LineageReplayIterator::error(e),
        },
        LineageQuery::LineageWide { lineage_id } => match lineage_prefix_generator(lineage_id) {
            Ok(p) => (p, 8),
            Err(e) => return LineageReplayIterator::error(e),
        },
        LineageQuery::EpochSpecific { lineage_id, epoch } => {
            match epoch_prefix_generator(lineage_id, *epoch) {
                Ok(p) => (p, 0),
                Err(e) => return LineageReplayIterator::error(e),
            }
        }
    };
    let prefix_len = prefix.len();
    let Ok(partition) = keyspace.open_partition("events", fjall::PartitionCreateOptions::default())
    else {
        return LineageReplayIterator::error(StorageError::Storage);
    };
    let Ok(min_seq) = encode_key(1) else {
        return LineageReplayIterator::error(StorageError::Storage);
    };
    let Ok(max_seq) = encode_key(u64::MAX) else {
        return LineageReplayIterator::error(StorageError::Storage);
    };
    let mut start = prefix.clone();
    start.extend_from_slice(&min_seq);
    let mut end = prefix;
    end.extend_from_slice(&max_seq);
    LineageReplayIterator {
        inner: Some(Box::new(partition.range(start..=end))),
        state: IteratorState::new(),
        prefix_len,
        epoch_len,
        last_epoch: Vec::new(),
        init_error: None,
    }
}
