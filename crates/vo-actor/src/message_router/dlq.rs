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
    pub fn new<T: serde::Serialize>(payload: &T) -> Result<Self, String> {
        let payload_bytes = serde_json::to_vec(payload)
            .map_err(|e| format!("failed to serialize payload: {}", e))?;
        let type_name = std::any::type_name::<T>().to_string();
        Ok(Self {
            payload: payload_bytes,
            type_name,
        })
    }

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
    InstanceNotFound { instance_id: String },
    InstanceState { instance_id: String, state: String },
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

    pub fn enqueue_with_reason(&mut self, mut entry: DeadLetterEntry, reason: DeadLetterReason) {
        entry.reason = reason;
        self.enqueue(entry);
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dead_letter_message_new_and_deserialize_roundtrip() {
        #[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq)]
        struct TestPayload {
            value: String,
            count: u32,
        }

        let original = TestPayload {
            value: "hello".to_string(),
            count: 42,
        };

        let message = DeadLetterMessage::new(&original).expect("should serialize");
        assert_eq!(message.type_name(), std::any::type_name::<TestPayload>());

        let deserialized: TestPayload = message.deserialize().expect("should deserialize");
        assert_eq!(original, deserialized);
    }

    #[test]
    fn dead_letter_message_deserialize_type_mismatch() {
        #[derive(Debug, serde::Serialize, serde::Deserialize)]
        struct TypeA { a: u32 }

        #[derive(Debug, serde::Serialize, serde::Deserialize)]
        struct TypeB { b: String }

        let message = DeadLetterMessage::new(&TypeA { a: 1 }).expect("should serialize");

        let result: Result<TypeB, _> = message.deserialize();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("type mismatch"));
    }

    #[test]
    fn dead_letter_queue_dequeue_returns_oldest() {
        let mut queue = DeadLetterQueue::new(3);
        let channel_id = ChannelId::new("test-channel");

        for i in 0..3 {
            let msg = DeadLetterMessage::new(&i).expect("serialize");
            let entry = DeadLetterEntry {
                channel_id: channel_id.clone(),
                message: msg,
                enqueued_at: TimestampMs::now(),
                reason: DeadLetterReason::ChannelNotFound,
            };
            queue.enqueue(entry);
        }

        let first = queue.dequeue().expect("should have entries");
        let deserialized: u32 = first.message.deserialize().expect("should deserialize");
        assert_eq!(deserialized, 0);

        assert_eq!(queue.len(), 2);
    }

    #[test]
    fn dead_letter_queue_dequeue_empty_returns_none() {
        let mut queue = DeadLetterQueue::new(10);
        assert!(queue.dequeue().is_none());
    }

    #[test]
    fn dead_letter_queue_fifo_ordering() {
        let mut queue = DeadLetterQueue::new(5);
        let channel_id = ChannelId::new("fifo-test");

        for i in 0..5 {
            let msg = DeadLetterMessage::new(&i).expect("serialize");
            let entry = DeadLetterEntry {
                channel_id: channel_id.clone(),
                message: msg,
                enqueued_at: TimestampMs::now(),
                reason: DeadLetterReason::DeliveryTimeout,
            };
            queue.enqueue(entry);
        }

        for expected in 0..5 {
            let entry = queue.dequeue().expect("entry should exist");
            let value: u32 = entry.message.deserialize().expect("deserialize");
            assert_eq!(value, expected);
        }

        assert!(queue.dequeue().is_none());
    }
}
