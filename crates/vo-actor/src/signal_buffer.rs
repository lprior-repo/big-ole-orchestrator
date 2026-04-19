//! Signal buffering module per ADR-042.
use crate::WaitKey;
use std::collections::{HashMap, VecDeque};
use vo_types::{BufferPolicy, SignalDelivery};
use vo_types::{InstanceId, TimestampMs};

/// A signal that has been buffered for later delivery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferedSignal {
    pub signal_id: String,
    pub payload: crate::SignalPayload,
    pub buffered_at: TimestampMs,
}

impl BufferedSignal {
    #[must_use]
    pub fn new(signal_id: String, payload: crate::SignalPayload, buffered_at: TimestampMs) -> Self {
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

#[cfg(feature = "proptest")]
mod proptest_invariants {
    use super::*;
    use proptest::prelude::*;

    fn make_instance_id(ulid_val: u128) -> InstanceId {
        let ulid = ulid::Ulid::from(ulid_val);
        InstanceId::parse(&ulid.to_string()).unwrap()
    }

    proptest! {
        #[test]
        fn buffer_policy_reject_never_buffers(has_matching_wait in proptest::bool::ANY) {
            let (delivery, result) = apply_policy(BufferPolicy::Reject, has_matching_wait, true);
            if !has_matching_wait {
                prop_assert_eq!(delivery, SignalDelivery::Rejected);
                prop_assert_eq!(result, Some(BufferResult::Rejected));
            } else {
                prop_assert_eq!(delivery, SignalDelivery::Accepted);
                prop_assert_eq!(result, None);
            }
        }

        #[test]
        fn buffer_policy_buffer_one_allows_at_most_one(has_matching_wait in proptest::bool::ANY, has_existing in proptest::bool::ANY) {
            let (delivery, result) = apply_policy(BufferPolicy::BufferOne, has_matching_wait, has_existing);
            if has_matching_wait {
                prop_assert_eq!(delivery, SignalDelivery::Accepted);
                prop_assert_eq!(result, None);
            } else if has_existing {
                prop_assert_eq!(delivery, SignalDelivery::Rejected);
                prop_assert_eq!(result, Some(BufferResult::Rejected));
            } else {
                prop_assert_eq!(delivery, SignalDelivery::Buffered);
                prop_assert_eq!(result, Some(BufferResult::Buffered));
            }
        }

        #[test]
        fn buffer_policy_buffer_many_always_buffers_without_wait(
            has_matching_wait in proptest::bool::ANY,
            has_existing in proptest::bool::ANY
        ) {
            let (delivery, result) = apply_policy(BufferPolicy::BufferMany, has_matching_wait, has_existing);
            if has_matching_wait {
                prop_assert_eq!(delivery, SignalDelivery::Accepted);
                prop_assert_eq!(result, None);
            } else {
                prop_assert_eq!(delivery, SignalDelivery::Buffered);
                prop_assert_eq!(result, Some(BufferResult::Buffered));
            }
        }

        #[test]
        fn can_buffer_reject_always_false(has_existing in proptest::bool::ANY, current_len in 0usize..1000usize) {
            let config = SignalBufferConfig::default();
            prop_assert!(!can_buffer(BufferPolicy::Reject, has_existing, current_len, &config));
        }

        #[test]
        fn can_buffer_buffer_one_always_true(has_existing in proptest::bool::ANY, current_len in 0usize..1000usize) {
            let config = SignalBufferConfig::default();
            prop_assert!(can_buffer(BufferPolicy::BufferOne, has_existing, current_len, &config));
        }

        #[test]
        fn can_buffer_buffer_many_respects_max(max_per_key in 1usize..100usize, current_len in 0usize..200usize) {
            let config = SignalBufferConfig::new(max_per_key);
            let expected = current_len < max_per_key;
            prop_assert_eq!(can_buffer(BufferPolicy::BufferMany, false, current_len, &config), expected);
        }
    }

    proptest! {
        #[test]
        fn signal_buffer_total_count_equals_sum_of_counts(
            max_per_key in 1usize..50usize,
            instance_seed in 1u128..1000u128,
            wait_key in "[a-z]+".prop_filter("non-empty", |s| !s.is_empty()),
            num_signals in 0usize..20usize
        ) {
            let config = SignalBufferConfig::new(max_per_key);
            let mut buffer = SignalBuffer::new(config);
            let wait_key = WaitKey::parse(&wait_key).unwrap();
            let instance = make_instance_id(instance_seed);

            for i in 0..num_signals {
                let signal = BufferedSignal::new(
                    format!("sig-{}", i),
                    crate::SignalPayload::empty(),
                    TimestampMs::now(),
                );
                buffer.buffer_signal(instance.clone(), wait_key.clone(), signal, BufferPolicy::BufferMany);
            }

            let total = buffer.total_buffered_count();
            let per_key = buffer.buffered_count(&instance, &wait_key);
            prop_assert_eq!(total, per_key);
        }

        #[test]
        fn signal_buffer_num_keys_equals_non_empty_entries(
            max_per_key in 1usize..50usize,
            instance1_seed in 1u128..1000u128,
            instance2_seed in 1001u128..2000u128,
            wait_key1 in "[a-z]+".prop_filter("non-empty", |s| !s.is_empty()),
            wait_key2 in "[a-z]+".prop_filter("non-empty", |s| !s.is_empty()),
        ) {
            let config = SignalBufferConfig::new(max_per_key);
            let mut buffer = SignalBuffer::new(config);
            let wk1 = WaitKey::parse(&wait_key1).unwrap();
            let wk2 = WaitKey::parse(&wait_key2).unwrap();
            let instance1 = make_instance_id(instance1_seed);
            let instance2 = make_instance_id(instance2_seed);

            let signal1 = BufferedSignal::new("sig-1".to_string(), crate::SignalPayload::empty(), TimestampMs::now());
            let signal2 = BufferedSignal::new("sig-2".to_string(), crate::SignalPayload::empty(), TimestampMs::now());

            buffer.buffer_signal(instance1.clone(), wk1.clone(), signal1, BufferPolicy::BufferMany);
            buffer.buffer_signal(instance2.clone(), wk2.clone(), signal2, BufferPolicy::BufferMany);

            prop_assert_eq!(buffer.num_keys_with_signals(), 2);
        }

        #[test]
        fn pop_buffered_returns_fifo_order(
            max_per_key in 3usize..20usize,
            instance_seed in 1u128..1000u128,
            wait_key in "[a-z]+".prop_filter("non-empty", |s| !s.is_empty()),
        ) {
            let config = SignalBufferConfig::new(max_per_key);
            let mut buffer = SignalBuffer::new(config);
            let wait_key = WaitKey::parse(&wait_key).unwrap();
            let instance = make_instance_id(instance_seed);

            let signal_ids = vec!["first", "second", "third"];
            for id in &signal_ids {
                let signal = BufferedSignal::new(
                    id.to_string(),
                    crate::SignalPayload::empty(),
                    TimestampMs::now(),
                );
                buffer.buffer_signal(instance.clone(), wait_key.clone(), signal, BufferPolicy::BufferMany);
            }

            for expected_id in signal_ids {
                let popped = buffer.pop_buffered(&instance, &wait_key);
                prop_assert!(popped.is_some());
                prop_assert_eq!(popped.unwrap().signal_id, expected_id);
            }
        }
    }
}

#[cfg(test)]
mod signal_buffer_entry_tests {
    use super::*;
    #[test]
    fn signal_buffer_entry_single_len_is_one() {
        let signal = BufferedSignal::new(
            "sig-1".to_string(),
            crate::SignalPayload::empty(),
            TimestampMs::now(),
        );
        let entry = SignalBufferEntry::Single(signal);
        assert_eq!(entry.len(), 1);
    }
    #[test]
    fn signal_buffer_entry_many_len_is_queue_len() {
        let signal1 = BufferedSignal::new(
            "sig-1".to_string(),
            crate::SignalPayload::empty(),
            TimestampMs::now(),
        );
        let signal2 = BufferedSignal::new(
            "sig-2".to_string(),
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
