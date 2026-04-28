//! Signal buffering module per ADR-042.
//!
//! Provides message buffering and backpressure for workflow signals that arrive
//! while an actor is waiting (blocked on a wait-key). Signals can be rejected,
//! buffered as a single pending message, or buffered as a queue depending on the
//! configured [`BufferPolicy`].
//!
//! # Buffering Policies
//!
//! - **`Reject`**: Drop signals when no matching wait-key exists. Zero memory overhead.
//! - **`BufferOne`**: Keep at most one pending signal per `(instance_id, wait_key)` pair.
//!   Subsequent signals arriving while one is buffered are rejected.
//! - **`BufferMany`**: Queue multiple pending signals per wait-key up to
//!   [`SignalBufferConfig::max_buffered_per_key`]. Signals exceeding the limit are dropped.
//!
//! # Backpressure
//!
//! When `BufferMany` is enabled, the buffer enforces a per-key capacity limit.
//! Once the queue is full, new signals are dropped (returning [`BufferResult::Dropped`])
//! rather than blocking the sender. This prevents unbounded memory growth under
//! high signal throughput.
//!
//! # Example
//!
//! ```
//! use vo_actor::signal_buffer::{SignalBuffer, SignalBufferConfig, BufferedSignal, BufferResult};
//! use vo_actor::WaitKey;
//! use vo_types::{BufferPolicy, SignalName, SignalPayload, InstanceId, TimestampMs};
//!
//! let config = SignalBufferConfig::new(10);
//! let mut buffer = SignalBuffer::new(config);
//!
//! let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
//! let wait_key = WaitKey::parse("approval").unwrap();
//! let signal = BufferedSignal::new(
//!     SignalName::parse("approved").unwrap(),
//!     SignalPayload::empty(),
//!     TimestampMs::now(),
//! );
//!
//! // Buffer a signal for a waiting actor
//! let result = buffer.buffer_signal(instance_id.clone(), &wait_key, signal, BufferPolicy::BufferOne);
//! assert!(matches!(result, BufferResult::Buffered));
//!
//! // Second signal is rejected (BufferOne limit reached)
//! let signal2 = BufferedSignal::new(
//!     SignalName::parse("approved").unwrap(),
//!     SignalPayload::empty(),
//!     TimestampMs::now(),
//! );
//! let result2 = buffer.buffer_signal(instance_id.clone(), &wait_key, signal2, BufferPolicy::BufferOne);
//! assert!(matches!(result2, BufferResult::Rejected));
//!
//! // Retrieve and deliver the buffered signal
//! let delivered = buffer.pop_buffered(&instance_id, &wait_key);
//! assert!(delivered.is_some());
//! ```
//!
//! # Related Modules
//!
//! - [`crate::signal_messages`] — Signal storage and work queue abstractions
//! - [`crate::lifecycle`] — Actor lifecycle states that determine buffering behavior
//! - [`crate::timers`] — Timer wait-keys, complementary to signal wait-keys

use crate::WaitKey;
use std::collections::{HashMap, VecDeque};
use vo_types::{BufferPolicy, SignalDelivery};
use vo_types::{InstanceId, SignalName, TimestampMs};

/// A signal that has been buffered for later delivery to a waiting actor.
///
/// Represents a signal that arrived while the target actor was blocked on a
/// wait-key. The signal is stored here until the actor becomes ready to
/// receive it via [`SignalBuffer::pop_buffered`].
///
/// # Fields
///
/// * `signal_id` — The name/type of the signal (e.g., `"approved"`, `"timeout"`)
/// * `payload` — The signal data, which the actor processes upon delivery
/// * `buffered_at` — Wall-clock timestamp when the signal was stored
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferedSignal {
    pub signal_id: String,
    pub payload: crate::SignalPayload,
    pub buffered_at: TimestampMs,
}

impl BufferedSignal {
    /// Creates a new buffered signal.
    ///
    /// # Example
    ///
    /// ```
    /// use vo_actor::signal_buffer::BufferedSignal;
    /// use vo_actor::{SignalName, SignalPayload};
    /// use vo_types::TimestampMs;
    ///
    /// let signal = BufferedSignal::new(
    ///     SignalName::parse("user_joined").unwrap(),
    ///     SignalPayload::from("alice"),
    ///     TimestampMs::now(),
    /// );
    /// ```
    #[must_use]
    pub fn new(signal_id: String, payload: crate::SignalPayload, buffered_at: TimestampMs) -> Self {
        Self {
            signal_id: signal_id.into(),
            payload,
            buffered_at,
        }
    }
}

/// Configuration for signal buffering behavior.
///
/// Controls the maximum number of signals that can be buffered per
/// `(instance_id, wait_key)` pair. The default is 100 signals per key.
///
/// # Example
///
/// ```
/// use vo_actor::signal_buffer::SignalBufferConfig;
///
/// // Use default: 100 signals per wait-key
/// let config = SignalBufferConfig::default();
///
/// // Custom limit: 10 signals per wait-key
/// let config = SignalBufferConfig::new(10);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignalBufferConfig {
    pub max_buffered_per_key: usize,
}

impl Default for SignalBufferConfig {
    /// Returns a config with a default limit of 100 signals per wait-key.
    fn default() -> Self {
        Self {
            max_buffered_per_key: 100,
        }
    }
}

impl SignalBufferConfig {
    /// Creates a new config with the specified maximum buffered signals per key.
    ///
    /// The value is clamped to a minimum of 1 to prevent zero-capacity configs.
    ///
    /// # Example
    ///
    /// ```
    /// use vo_actor::signal_buffer::SignalBufferConfig;
    ///
    /// let config = SignalBufferConfig::new(50);
    /// assert_eq!(config.max_buffered_per_key, 50);
    ///
    /// // Values below 1 are clamped to 1
    /// let config = SignalBufferConfig::new(0);
    /// assert_eq!(config.max_buffered_per_key, 1);
    /// ```
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
///
/// Maintains a collection of buffered signals organized by `(instance_id, wait_key)` pairs.
/// Supports both single-signal buffering (`BufferOne` policy) and queue-based buffering
/// (`BufferMany` policy) with configurable capacity limits.
///
/// # Internal Structure
///
/// Each buffer entry is either:
/// - `Single` — One buffered signal (used by `BufferOne` policy)
/// - `Many` — A queue of buffered signals (used by `BufferMany` policy, or when a `BufferOne`
///   entry needs to grow to hold a second signal)
///
/// # Thread Safety
///
/// This type is `Sync` and `Clone` but is not designed for concurrent access.
/// In production code, wrap it in an `Arc<RwLock<SignalBuffer>>` for async access,
/// or use it within a single async task.
///
/// # Example
///
/// ```
/// use vo_actor::signal_buffer::{SignalBuffer, SignalBufferConfig, BufferedSignal};
/// use vo_actor::WaitKey;
/// use vo_types::{BufferPolicy, SignalName, SignalPayload, InstanceId, TimestampMs};
///
/// let config = SignalBufferConfig::new(10);
/// let mut buffer = SignalBuffer::new(config);
///
/// let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
/// let wait_key = WaitKey::parse("approval").unwrap();
///
/// let signal = BufferedSignal::new(
///     SignalName::parse("approved").unwrap(),
///     SignalPayload::empty(),
///     TimestampMs::now(),
/// );
///
/// buffer.buffer_signal(instance_id.clone(), wait_key.clone(), signal, BufferPolicy::BufferOne);
/// assert!(buffer.has_buffered_signals(&instance_id, wait_key.clone()));
///
/// let delivered = buffer.pop_buffered(&instance_id, wait_key);
/// assert!(delivered.is_some());
/// assert!(!buffer.has_buffered_signals(&instance_id, "approval"));
/// ```
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
///
/// Returned by [`SignalBuffer::buffer_signal`] and pure functions like
/// [`apply_policy`] and [`can_buffer`] to indicate the outcome of the
/// buffering decision.
///
/// # Variants
///
/// * `Buffered` — The signal was successfully stored in the buffer
/// * `Rejected` — The signal was rejected per the [`BufferPolicy`] (e.g., `BufferOne` already has a pending signal)
/// * `Dropped` — The signal was dropped because the buffer is full (only for `BufferMany` policies)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferResult {
    Buffered,
    Rejected,
    Dropped,
}

/// Pure function that applies a [`BufferPolicy`] to determine signal delivery behavior.
///
/// This function encodes the decision logic for what happens when a signal arrives
/// and the actor is not currently waiting (no matching wait-key):
///
/// | Policy | Matching wait-key | Existing buffer | Result |
/// |--------|------------------|-----------------|--------|
/// | `Reject` | Yes | — | `Accepted`, `None` |
/// | `Reject` | No | — | `Rejected`, `Some(Rejected)` |
/// | `BufferOne` | Yes | — | `Accepted`, `None` |
/// | `BufferOne` | No | Yes | `Rejected`, `Some(Rejected)` |
/// | `BufferOne` | No | No | `Buffered`, `Some(Buffered)` |
/// | `BufferMany` | Yes | — | `Accepted`, `None` |
/// | `BufferMany` | No | — | `Buffered`, `Some(Buffered)` |
///
/// # Returns
///
/// A tuple of `(SignalDelivery, Option<BufferResult>)` where the second element
/// is `None` when the signal is accepted immediately (matching wait-key found)
/// and `Some(...)` when a buffering decision was made.
///
/// # Example
///
/// ```
/// use vo_actor::signal_buffer::apply_policy;
/// use vo_types::{BufferPolicy, SignalDelivery};
///
/// // Signal arrives, actor is waiting → accept immediately
/// let (delivery, result) = apply_policy(BufferPolicy::BufferOne, true, false);
/// assert_eq!(delivery, SignalDelivery::Accepted);
/// assert!(result.is_none());
///
/// // Signal arrives, actor not waiting, no existing buffer → buffer
/// let (delivery, result) = apply_policy(BufferPolicy::BufferOne, false, false);
/// assert_eq!(delivery, SignalDelivery::Buffered);
/// assert!(matches!(result, Some(vo_actor::signal_buffer::BufferResult::Buffered)));
/// ```
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

/// Pure function that determines whether a new signal can be buffered given the
/// current policy, existing state, and buffer capacity.
///
/// This is a lightweight check useful for pre-decisions before calling
/// [`SignalBuffer::buffer_signal`]. The full buffering logic (which may
/// transform a `BufferOne` entry to `BufferMany`) lives in the buffer itself.
///
/// # Parameters
///
/// * `policy` — The buffering policy to apply
/// * `_has_existing_buffer` — Reserved for future use; currently unused
/// * `current_buffer_len` — Number of signals already buffered for the key
/// * `config` — The buffer configuration with capacity limits
///
/// # Returns
///
/// `true` if a new signal would be accepted into the buffer, `false` if it
/// would be rejected or dropped.
///
/// # Example
///
/// ```
/// use vo_actor::signal_buffer::{can_buffer, SignalBufferConfig};
/// use vo_types::{BufferPolicy, BufferResult};
///
/// let config = SignalBufferConfig::new(3);
///
/// // BufferMany with room: can buffer
/// assert!(can_buffer(BufferPolicy::BufferMany, false, 2, &config));
///
/// // BufferMany at capacity: cannot buffer
/// assert!(!can_buffer(BufferPolicy::BufferMany, false, 3, &config));
///
/// // Reject policy: never can buffer
/// assert!(!can_buffer(BufferPolicy::Reject, false, 0, &config));
/// ```
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
    /// Creates a new signal buffer with the given configuration.
    ///
    /// # Example
    ///
    /// ```
    /// use vo_actor::signal_buffer::{SignalBuffer, SignalBufferConfig};
    ///
    /// let config = SignalBufferConfig::new(50);
    /// let buffer = SignalBuffer::new(config);
    /// ```
    #[must_use]
    pub fn new(config: SignalBufferConfig) -> Self {
        Self {
            entries: HashMap::new(),
            config,
        }
    }

    /// Creates a new signal buffer with the default configuration (100 signals per key).
    #[must_use]
    pub fn with_default_config() -> Self {
        Self::new(SignalBufferConfig::default())
    }

    /// Returns the number of buffered signals for the given `(instance_id, wait_key)` pair.
    ///
    /// Returns 0 if no signals are buffered for the key.
    ///
    /// # Example
    ///
    /// ```
    /// use vo_actor::signal_buffer::{SignalBuffer, SignalBufferConfig, BufferedSignal};
    /// use vo_actor::WaitKey;
    /// use vo_types::{BufferPolicy, SignalName, SignalPayload, InstanceId, TimestampMs};
    ///
    /// let mut buffer = SignalBuffer::with_default_config();
    /// let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    ///
    /// let wk = WaitKey::parse("wait-1").unwrap();
    /// assert_eq!(buffer.buffered_count(&instance_id, &wk), 0);
    ///
    /// let signal = BufferedSignal::new(
    ///     SignalName::parse("sig").unwrap(),
    ///     SignalPayload::empty(),
    ///     TimestampMs::now(),
    /// );
    /// buffer.buffer_signal(instance_id.clone(), &wk, signal, BufferPolicy::BufferMany);
    ///
    /// assert_eq!(buffer.buffered_count(&instance_id, &wk), 1);
    /// ```
    #[must_use]
    pub fn buffered_count<W>(&self, instance_id: &InstanceId, wait_key: W) -> usize
    where
        W: Into<WaitKey>,
    {
        let key = BufferKey {
            instance_id: instance_id.clone(),
            wait_key: wait_key.into(),
        };
        self.entries.get(&key).map(|entry| entry.len()).unwrap_or(0)
    }

    /// Returns true if there are buffered signals for the given `(instance_id, wait_key)` pair.
    ///
    /// This is a convenience wrapper around [`buffered_count`](Self::buffered_count).
    ///
   /// # Example
    ///
    /// ```
    /// use vo_actor::signal_buffer::{SignalBuffer, BufferedSignal};
    /// use vo_actor::WaitKey;
    /// use vo_types::{BufferPolicy, SignalName, SignalPayload, InstanceId, TimestampMs};
    ///
    /// let mut buffer = SignalBuffer::with_default_config();
    /// let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    ///
    /// let wk = WaitKey::parse("wait-1").unwrap();
    /// assert!(!buffer.has_buffered_signals(&instance_id, &wk));
    ///
    /// let signal = BufferedSignal::new(
    ///     SignalName::parse("sig").unwrap(),
    ///     SignalPayload::empty(),
    ///     TimestampMs::now(),
    /// );
    /// buffer.buffer_signal(instance_id.clone(), &wk, signal, BufferPolicy::BufferOne);
    ///
    /// assert!(buffer.has_buffered_signals(&instance_id, &wk));
    /// ```
    #[must_use]
    pub fn has_buffered_signals<W>(&self, instance_id: &InstanceId, wait_key: W) -> bool
    where
        W: Into<WaitKey>,
    {
        self.buffered_count(instance_id, wait_key) > 0
    }

    /// Buffers a signal for later delivery to a waiting actor.
    ///
    /// The behavior depends on the [`BufferPolicy`]:
    ///
    /// - **`Reject`**: Always returns [`BufferResult::Rejected`]
    /// - **`BufferOne`**: If no signal is already buffered for the key, stores this signal
    ///   as a single entry. If one is already buffered, returns `Rejected`.
    /// - **`BufferMany`**: Appends to the queue for the key. Returns `Dropped` if the
    ///   queue is full (`max_buffered_per_key`). If the key previously had a `Single`
    ///   entry, it is upgraded to a `Many` queue containing both signals.
    ///
   /// # Example
    ///
    /// ```
    /// use vo_actor::signal_buffer::{SignalBuffer, BufferedSignal, BufferResult};
    /// use vo_actor::WaitKey;
    /// use vo_types::{BufferPolicy, SignalName, SignalPayload, InstanceId, TimestampMs};
    ///
    /// let mut buffer = SignalBuffer::with_default_config();
    /// let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    /// let wk = WaitKey::parse("wait-1").unwrap();
    ///
    /// let signal = BufferedSignal::new(
    ///     SignalName::parse("sig-1").unwrap(),
    ///     SignalPayload::empty(),
    ///     TimestampMs::now(),
    /// );
    ///
    /// let result = buffer.buffer_signal(instance_id.clone(), &wk, signal, BufferPolicy::BufferOne);
    /// assert!(matches!(result, BufferResult::Buffered));
    /// ```
    pub fn buffer_signal(
        &mut self,
        instance_id: InstanceId,
        wait_key: impl Into<WaitKey>,
        signal: BufferedSignal,
        policy: BufferPolicy,
    ) -> BufferResult {
        let key = BufferKey {
            instance_id: instance_id.clone(),
            wait_key: wait_key.into(),
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

    /// Retrieves and removes the first buffered signal for the given key.
    ///
    /// For `BufferOne` entries, the entire entry is removed. For `BufferMany`
    /// entries, only the front signal is removed; the queue entry remains if
    /// signals are still buffered.
    ///
    /// Returns `None` if no signals are buffered for the key.
    ///
    /// # Example
    ///
    /// ```
    /// use vo_actor::signal_buffer::{SignalBuffer, BufferedSignal};
    /// use vo_actor::WaitKey;
    /// use vo_types::{BufferPolicy, SignalName, SignalPayload, InstanceId, TimestampMs};
    ///
    /// let mut buffer = SignalBuffer::with_default_config();
    /// let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    /// let wk = WaitKey::parse("wait-1").unwrap();
    ///
    /// let signal = BufferedSignal::new(
    ///     SignalName::parse("sig").unwrap(),
    ///     SignalPayload::empty(),
    ///     TimestampMs::now(),
    /// );
    /// buffer.buffer_signal(instance_id.clone(), &wk, signal, BufferPolicy::BufferOne);
    ///
    /// let delivered = buffer.pop_buffered(&instance_id, &wk);
    /// assert!(delivered.is_some());
    /// assert_eq!(delivered.unwrap().signal_id.as_str(), "sig");
    /// assert!(buffer.pop_buffered(&instance_id, &wk).is_none());
    /// ```
    #[must_use]
    pub fn pop_buffered<W>(
        &mut self,
        instance_id: &InstanceId,
        wait_key: W,
    ) -> Option<BufferedSignal>
    where
        W: Into<WaitKey>,
    {
        let key = BufferKey {
            instance_id: instance_id.clone(),
            wait_key: wait_key.into(),
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

    /// Returns all buffered signals for the given key without removing them.
    ///
    /// For `BufferOne` entries, returns a vector with a single signal.
    /// For `BufferMany` entries, returns all queued signals in FIFO order.
    ///
    /// Returns an empty vector if no signals are buffered for the key.
    ///
    /// # Example
    ///
    /// ```
    /// use vo_actor::signal_buffer::{SignalBuffer, BufferedSignal};
    /// use vo_actor::WaitKey;
    /// use vo_types::{BufferPolicy, SignalName, SignalPayload, InstanceId, TimestampMs};
    ///
    /// let mut buffer = SignalBuffer::with_default_config();
    /// let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    /// let wk = WaitKey::parse("wait-1").unwrap();
    ///
    /// let signal = BufferedSignal::new(
    ///     SignalName::parse("sig").unwrap(),
    ///     SignalPayload::empty(),
    ///     TimestampMs::now(),
    /// );
    /// buffer.buffer_signal(instance_id.clone(), &wk, signal, BufferPolicy::BufferMany);
    ///
    /// let all = buffer.peek_all(&instance_id, &wk);
    /// assert_eq!(all.len(), 1);
    /// assert_eq!(all[0].signal_id.as_str(), "sig");
    /// ```
    #[must_use]
    pub fn peek_all<W>(&self, instance_id: &InstanceId, wait_key: W) -> Vec<BufferedSignal>
    where
        W: Into<WaitKey>,
    {
        let key = BufferKey {
            instance_id: instance_id.clone(),
            wait_key: wait_key.into(),
        };
        self.entries
            .get(&key)
            .map(|entry| entry.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Removes all buffered signals for the given `(instance_id, wait_key)` pair.
    ///
    /// This is useful when an actor's wait expires or is cancelled and buffered
    /// signals should be discarded rather than delivered.
    ///
    /// # Example
    ///
    /// ```
    /// use vo_actor::signal_buffer::{SignalBuffer, BufferedSignal};
    /// use vo_actor::WaitKey;
    /// use vo_types::{BufferPolicy, SignalName, SignalPayload, InstanceId, TimestampMs};
    ///
    /// let mut buffer = SignalBuffer::with_default_config();
    /// let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    /// let wk = WaitKey::parse("wait-1").unwrap();
    ///
    /// let signal = BufferedSignal::new(
    ///     SignalName::parse("sig").unwrap(),
    ///     SignalPayload::empty(),
    ///     TimestampMs::now(),
    /// );
    /// buffer.buffer_signal(instance_id.clone(), &wk, signal, BufferPolicy::BufferOne);
    ///
    /// buffer.clear(&instance_id, &wk);
    /// assert!(!buffer.has_buffered_signals(&instance_id, "wait-1"));
    /// ```
    pub fn clear<W>(&mut self, instance_id: &InstanceId, wait_key: W)
    where
        W: Into<WaitKey>,
    {
        let key = BufferKey {
            instance_id: instance_id.clone(),
            wait_key: wait_key.into(),
        };
        self.entries.remove(&key);
    }

    /// Returns the number of distinct `(instance_id, wait_key)` pairs that have buffered signals.
    #[must_use]
    pub fn num_keys_with_signals(&self) -> usize {
        self.entries.len()
    }

    /// Returns the total number of buffered signals across all keys.
    ///
    /// For `BufferOne` entries, counts as 1. For `BufferMany` entries, counts
    /// as the number of signals in the queue.
    #[must_use]
    pub fn total_buffered_count(&self) -> usize {
        self.entries.values().map(|e| e.len()).sum()
    }

    /// Returns the buffer's configuration.
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
