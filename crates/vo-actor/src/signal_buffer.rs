//! Signal buffering module per ADR-042.
use crate::WaitKey;
use std::collections::{HashMap, VecDeque};
use vo_types::{BufferPolicy, SignalDelivery};
use vo_types::{InstanceId, SignalName, TimestampMs};

/// A signal that has been buffered for later delivery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferedSignal {
    pub signal_id: SignalName,
    pub payload: crate::SignalPayload,
    pub buffered_at: TimestampMs,
}

impl BufferedSignal {
    pub fn new(
        signal_id: SignalName,
        payload: crate::SignalPayload,
        buffered_at: TimestampMs,
    ) -> Self {
        Self {
            signal_id,
            payload,
            buffered_at,
        }
    }
}

/// Configuration for signal buffering behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignalBufferConfig {
    pub max_buffered_per_key: usize,
}

impl Default for SignalBufferConfig {
    fn default() -> Self {
        Self {
            max_buffered_per_key: 100,
        }
    }
}

impl SignalBufferConfig {
    #[must_use]
    pub fn new(max_buffered_per_key: usize) -> Self {
        Self {
            max_buffered_per_key: max_buffered_per_key.max(1),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct BufferKey {
    instance_id: InstanceId,
    wait_key: WaitKey,
}

/// The signal buffer state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalBuffer {
    entries: HashMap<BufferKey, SignalBufferEntry>,
    config: SignalBufferConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SignalBufferEntry {
    Single(BufferedSignal),
    Many(VecDeque<BufferedSignal>),
}

/// Result of attempting to buffer a signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferResult {
    Buffered,
    Rejected,
    Dropped,
}

/// Pure function: applies the buffer policy.
pub fn apply_policy(
    policy: BufferPolicy,
    has_matching_wait: bool,
    has_existing_buffer: bool,
) -> (SignalDelivery, Option<BufferResult>) {
    if has_matching_wait {
        return (SignalDelivery::Accepted, None);
    }
    match policy {
        BufferPolicy::Reject => (SignalDelivery::Rejected, Some(BufferResult::Rejected)),
        BufferPolicy::BufferOne => {
            if has_existing_buffer {
                (SignalDelivery::Rejected, Some(BufferResult::Rejected))
            } else {
                (SignalDelivery::Buffered, Some(BufferResult::Buffered))
            }
        }
        BufferPolicy::BufferMany => (SignalDelivery::Buffered, Some(BufferResult::Buffered)),
    }
}

/// Pure function: determines if a new signal can be buffered.
pub fn can_buffer(
    policy: BufferPolicy,
    _has_existing_buffer: bool,
    current_buffer_len: usize,
    config: &SignalBufferConfig,
) -> bool {
    match policy {
        BufferPolicy::Reject => false,
        BufferPolicy::BufferOne => true,
        BufferPolicy::BufferMany => current_buffer_len < config.max_buffered_per_key,
    }
}

impl SignalBuffer {
    #[must_use]
    pub fn new(config: SignalBufferConfig) -> Self {
        Self {
            entries: HashMap::new(),
            config,
        }
    }

    #[must_use]
    pub fn with_default_config() -> Self {
        Self::new(SignalBufferConfig::default())
    }

    #[must_use]
    pub fn buffered_count(&self, instance_id: &InstanceId, wait_key: &WaitKey) -> usize {
        let key = BufferKey {
            instance_id: instance_id.clone(),
            wait_key: wait_key.clone(),
        };
        self.entries.get(&key).map(|entry| entry.len()).unwrap_or(0)
    }

    #[must_use]
    pub fn has_buffered_signals(&self, instance_id: &InstanceId, wait_key: &WaitKey) -> bool {
        self.buffered_count(instance_id, wait_key) > 0
    }

    pub fn buffer_signal(
        &mut self,
        instance_id: InstanceId,
        wait_key: WaitKey,
        signal: BufferedSignal,
        policy: BufferPolicy,
    ) -> BufferResult {
        let key = BufferKey {
            instance_id: instance_id.clone(),
            wait_key: wait_key.clone(),
        };
        match policy {
            BufferPolicy::Reject => BufferResult::Rejected,
            BufferPolicy::BufferOne => {
                if self.entries.contains_key(&key) {
                    return BufferResult::Rejected;
                }
                self.entries.insert(key, SignalBufferEntry::Single(signal));
                BufferResult::Buffered
            }
            BufferPolicy::BufferMany => {
                let entry = self
                    .entries
                    .entry(key)
                    .or_insert_with(|| SignalBufferEntry::Many(VecDeque::new()));
                match entry {
                    SignalBufferEntry::Many(queue) => {
                        if queue.len() >= self.config.max_buffered_per_key {
                            return BufferResult::Dropped;
                        }
                        queue.push_back(signal);
                        BufferResult::Buffered
                    }
                    SignalBufferEntry::Single(_) => {
                        let old_signal = match entry.get_single_cloned() {
                            Some(s) => s,
                            None => return BufferResult::Dropped,
                        };
                        let mut queue = VecDeque::new();
                        queue.push_back(old_signal);
                        queue.push_back(signal);
                        *entry = SignalBufferEntry::Many(queue);
                        BufferResult::Buffered
                    }
                }
            }
        }
    }

    #[must_use]
    pub fn pop_buffered(
        &mut self,
        instance_id: &InstanceId,
        wait_key: &WaitKey,
    ) -> Option<BufferedSignal> {
        let key = BufferKey {
            instance_id: instance_id.clone(),
            wait_key: wait_key.clone(),
        };
        let entry = self.entries.get_mut(&key)?;
        match entry {
            SignalBufferEntry::Single(signal) => {
                let signal = signal.clone();
                self.entries.remove(&key);
                Some(signal)
            }
            SignalBufferEntry::Many(queue) => {
                let signal = queue.pop_front()?;
                if queue.is_empty() {
                    self.entries.remove(&key);
                }
                Some(signal)
            }
        }
    }

    #[must_use]
    pub fn peek_all(&self, instance_id: &InstanceId, wait_key: &WaitKey) -> Vec<BufferedSignal> {
        let key = BufferKey {
            instance_id: instance_id.clone(),
            wait_key: wait_key.clone(),
        };
        self.entries
            .get(&key)
            .map(|entry| entry.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn clear(&mut self, instance_id: &InstanceId, wait_key: &WaitKey) {
        let key = BufferKey {
            instance_id: instance_id.clone(),
            wait_key: wait_key.clone(),
        };
        self.entries.remove(&key);
    }

    #[must_use]
    pub fn num_keys_with_signals(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn total_buffered_count(&self) -> usize {
        self.entries.values().map(|e| e.len()).sum()
    }

    #[must_use]
    pub fn config(&self) -> SignalBufferConfig {
        self.config
    }
}

impl SignalBufferEntry {
    fn len(&self) -> usize {
        match self {
            SignalBufferEntry::Single(_) => 1,
            SignalBufferEntry::Many(q) => q.len(),
        }
    }
    fn iter(&self) -> Box<dyn Iterator<Item = &BufferedSignal> + '_> {
        match self {
            SignalBufferEntry::Single(s) => Box::new(std::iter::once(s)),
            SignalBufferEntry::Many(q) => Box::new(q.iter()),
        }
    }
    fn get_single_cloned(&self) -> Option<BufferedSignal> {
        match self {
            SignalBufferEntry::Single(s) => Some(s.clone()),
            SignalBufferEntry::Many(_) => None,
        }
    }
}

#[cfg(test)]
mod signal_buffer_entry_tests {
    use super::*;
    #[test]
    fn signal_buffer_entry_single_len_is_one() {
        let signal = BufferedSignal::new(
            SignalName::parse("sig1").unwrap(),
            crate::SignalPayload::empty(),
            TimestampMs::now(),
        );
        let entry = SignalBufferEntry::Single(signal);
        assert_eq!(entry.len(), 1);
    }
    #[test]
    fn signal_buffer_entry_many_len_is_queue_len() {
        let signal1 = BufferedSignal::new(
            SignalName::parse("sig1").unwrap(),
            crate::SignalPayload::empty(),
            TimestampMs::now(),
        );
        let signal2 = BufferedSignal::new(
            SignalName::parse("sig2").unwrap(),
            crate::SignalPayload::empty(),
            TimestampMs::now(),
        );
        let mut queue = VecDeque::new();
        queue.push_back(signal1);
        queue.push_back(signal2);
        let entry = SignalBufferEntry::Many(queue);
        assert_eq!(entry.len(), 2);
    }
}
