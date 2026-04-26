//! Dead Letter Queue — Failed message storage.
//!
//! Types for capturing undeliverable messages and their failure reasons.

use super::data::{ChannelId, TimestampMs};

#[derive(Debug, Clone)]
pub struct DeadLetterEntry {
    pub channel_id: ChannelId,
    pub message: DeadLetterMessage,
    pub enqueued_at: TimestampMs,
    pub reason: DeadLetterReason,
}

#[derive(Debug, Clone)]
pub struct DeadLetterMessage {
    pub payload: Vec<u8>,
    pub type_name: String,
}

impl DeadLetterMessage {
    #[allow(dead_code)]
    pub fn new<T: serde::Serialize>(payload: &T) -> Result<Self, String> {
        let payload_bytes = serde_json::to_vec(payload)
            .map_err(|e| format!("failed to serialize payload: {}", e))?;
        let type_name = std::any::type_name::<T>().to_string();
        Ok(Self {
            payload: payload_bytes,
            type_name,
        })
    }

    #[allow(dead_code)]
    pub fn deserialize<T: serde::de::DeserializeOwned>(&self) -> Result<T, String> {
        if self.type_name != std::any::type_name::<T>() {
            return Err(format!(
                "type mismatch: expected {}, got {}",
                self.type_name,
                std::any::type_name::<T>()
            ));
        }
        serde_json::from_slice(&self.payload)
            .map_err(|e| format!("failed to deserialize payload: {}", e))
    }

    #[must_use]
    pub fn type_name(&self) -> &str {
        &self.type_name
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeadLetterReason {
    ChannelNotFound,
    NoActiveDestinations,
    DeliveryTimeout,
    ActorError(String),
    ExplicitDrop,
}

#[derive(Debug)]
pub struct DeadLetterQueue {
    entries: Vec<DeadLetterEntry>,
    max_size: usize,
}

impl DeadLetterQueue {
    #[must_use]
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_size,
        }
    }

    pub fn enqueue(&mut self, entry: DeadLetterEntry) {
        if self.entries.len() >= self.max_size {
            self.entries.remove(0);
        }
        self.entries.push(entry);
    }

    #[allow(dead_code)]
    pub fn dequeue(&mut self) -> Option<DeadLetterEntry> {
        if self.entries.is_empty() {
            None
        } else {
            Some(self.entries.remove(0))
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    #[must_use]
    pub fn entries(&self) -> &[DeadLetterEntry] {
        &self.entries
    }
}
