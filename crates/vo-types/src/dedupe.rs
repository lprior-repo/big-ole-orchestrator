//! Dedupe key types for ADR-028 exactly-once ingress deduplication.

use serde::{Deserialize, Serialize};

use crate::string_newtype;
use crate::string_types::InstanceId;
use crate::ParseError;

/// Stable dedupe key supplied by caller or derived from provider-native event ID.
///
/// Used to detect duplicate ingress requests for exactly-once delivery.
/// Validated to be non-empty and at most 256 characters.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DedupeKey(pub(crate) String);

impl DedupeKey {
    /// Parse a `DedupeKey` from a string.
    ///
    /// # Errors
    ///
    /// Returns `ParseError::Empty` if the input is empty.
    /// Returns `ParseError::ExceedsMaxLength` if the input exceeds 256 characters.
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        const TYPE_NAME: &str = "DedupeKey";
        const MAX_LEN: usize = 256;
        if input.is_empty() {
            return Err(ParseError::Empty {
                type_name: TYPE_NAME,
            });
        }
        if input.chars().count() > MAX_LEN {
            return Err(ParseError::ExceedsMaxLength {
                type_name: TYPE_NAME,
                max: MAX_LEN,
                actual: input.chars().count(),
            });
        }
        Ok(Self(input.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
string_newtype!(DedupeKey);

/// Composite key of `InstanceId` + command type for partitioning dedupe records.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DedupePartitionKey {
    instance_id: InstanceId,
    command_type: String,
}

impl DedupePartitionKey {
    /// Construct a `DedupePartitionKey`.
    ///
    /// # Errors
    ///
    /// Returns `ParseError::Empty` if `command_type` is empty.
    /// Returns `ParseError::ExceedsMaxLength` if `command_type` exceeds 256 characters.
    pub fn new(instance_id: InstanceId, command_type: &str) -> Result<Self, ParseError> {
        const TYPE_NAME: &str = "DedupePartitionKey";
        const MAX_LEN: usize = 256;
        if command_type.is_empty() {
            return Err(ParseError::Empty {
                type_name: TYPE_NAME,
            });
        }
        if command_type.chars().count() > MAX_LEN {
            return Err(ParseError::ExceedsMaxLength {
                type_name: TYPE_NAME,
                max: MAX_LEN,
                actual: command_type.chars().count(),
            });
        }
        Ok(Self {
            instance_id,
            command_type: command_type.to_string(),
        })
    }

    #[must_use]
    pub fn instance_id(&self) -> &InstanceId {
        &self.instance_id
    }

    #[must_use]
    pub fn command_type(&self) -> &str {
        &self.command_type
    }
}
