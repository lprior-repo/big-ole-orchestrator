use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ParseError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CommandId(pub(crate) Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CorrelationId(pub(crate) Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CausationId(pub(crate) Uuid);

impl CommandId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    #[must_use]
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    #[must_use]
    pub fn to_uuid(&self) -> Uuid {
        self.0
    }

    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let uuid = Uuid::parse_str(input).map_err(|e| ParseError::InvalidFormat {
            type_name: "CommandId",
            reason: format!("invalid UUID: {e}"),
        })?;
        Ok(Self(uuid))
    }

    #[must_use]
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(Uuid::from_bytes(bytes))
    }
}

impl Default for CommandId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for CommandId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<String> for CommandId {
    type Error = ParseError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<CommandId> for String {
    fn from(value: CommandId) -> String {
        value.0.to_string()
    }
}

impl CorrelationId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    #[must_use]
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    #[must_use]
    pub fn to_uuid(&self) -> Uuid {
        self.0
    }

    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let uuid = Uuid::parse_str(input).map_err(|e| ParseError::InvalidFormat {
            type_name: "CorrelationId",
            reason: format!("invalid UUID: {e}"),
        })?;
        Ok(Self(uuid))
    }

    #[must_use]
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(Uuid::from_bytes(bytes))
    }
}

impl Default for CorrelationId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for CorrelationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<String> for CorrelationId {
    type Error = ParseError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<CorrelationId> for String {
    fn from(value: CorrelationId) -> String {
        value.0.to_string()
    }
}

impl CausationId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    #[must_use]
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    #[must_use]
    pub fn to_uuid(&self) -> Uuid {
        self.0
    }

    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let uuid = Uuid::parse_str(input).map_err(|e| ParseError::InvalidFormat {
            type_name: "CausationId",
            reason: format!("invalid UUID: {e}"),
        })?;
        Ok(Self(uuid))
    }

    #[must_use]
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(Uuid::from_bytes(bytes))
    }
}

impl Default for CausationId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for CausationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<String> for CausationId {
    type Error = ParseError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<CausationId> for String {
    fn from(value: CausationId) -> String {
        value.0.to_string()
    }
}
