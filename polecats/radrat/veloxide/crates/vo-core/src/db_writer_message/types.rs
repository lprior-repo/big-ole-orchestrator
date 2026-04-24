//! DbWriterMessage types for atomic control-plane transitions.

use serde::{Deserialize, Serialize};
use thiserror::Error;
use vo_types::{FireAtMs, SequenceNumber, TimerId, MAX_SUPPORTED_SCHEMA_VERSION};

fn default_schema_version() -> u16 {
    MAX_SUPPORTED_SCHEMA_VERSION
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DbWriterMessageError {
    #[error("fence token must be nonzero")]
    ZeroFenceToken,
    #[error("sequence number must be nonzero")]
    ZeroSequenceNumber,
    #[error("serialization error: {0}")]
    SerializationError(String),
    #[error("unknown DbWriterMessage variant: {0}")]
    UnknownVariant(String),
    #[error("missing field: {0}")]
    MissingField(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimerOp {
    Upsert {
        timer_id: TimerId,
        fire_at: FireAtMs,
    },
    Delete {
        timer_id: TimerId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotData {
    sequence_number: SequenceNumber,
    #[serde(default = "default_schema_version")]
    schema_version: u16,
    state_bytes: Vec<u8>,
}

#[allow(dead_code)]
impl SnapshotData {
    #[must_use]
    pub fn new(
        sequence_number: SequenceNumber,
        schema_version: u16,
        state_bytes: Vec<u8>,
    ) -> Option<Self> {
        if state_bytes.is_empty() {
            return None;
        }
        Some(Self {
            sequence_number,
            schema_version,
            state_bytes,
        })
    }

    #[must_use]
    pub fn sequence_number(&self) -> SequenceNumber {
        self.sequence_number
    }

    #[must_use]
    pub fn schema_version(&self) -> u16 {
        self.schema_version
    }

    #[must_use]
    pub fn state_bytes(&self) -> &[u8] {
        &self.state_bytes
    }
}
