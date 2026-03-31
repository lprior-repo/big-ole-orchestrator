use std::fmt;

use serde::{Deserialize, Serialize};

use crate::types::{
    check_identifier_boundaries, extract_invalid_chars, is_identifier_char, is_lowercase_hex,
};
use crate::ParseError;

macro_rules! string_newtype {
    ($name:ident) => {
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
        impl TryFrom<String> for $name {
            type Error = ParseError;
            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::parse(&value)
            }
        }
        impl From<$name> for String {
            fn from(value: $name) -> String {
                value.0
            }
        }
    };
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct InstanceId(pub(crate) String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct WorkflowName(pub(crate) String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct NodeName(pub(crate) String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct BinaryHash(pub(crate) String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct TimerId(pub(crate) String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct IdempotencyKey(pub(crate) String);

impl InstanceId {
    /// Parse an `InstanceId` from a ULID string.
    ///
    /// # Errors
    ///
    /// Returns `ParseError` if the input is empty, wrong length, or not a valid ULID.
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        const TYPE_NAME: &str = "InstanceId";
        if input.is_empty() {
            return Err(ParseError::Empty {
                type_name: TYPE_NAME,
            });
        }
        if input.len() != 26 {
            return Err(ParseError::InvalidFormat {
                type_name: TYPE_NAME,
                reason: format!("expected 26 characters, got {}", input.len()),
            });
        }
        if let Some(first_char) = input.chars().next() {
            if first_char > '7' {
                return Err(ParseError::InvalidFormat {
                    type_name: TYPE_NAME,
                    reason: "invalid ULID validation: exceeds maximum 128-bit value".to_string(),
                });
            }
        }
        let ulid = ulid::Ulid::from_string(input).map_err(|e| ParseError::InvalidFormat {
            type_name: TYPE_NAME,
            reason: format!("invalid ULID: {e}"),
        })?;
        if ulid.0 == 0 {
            return Err(ParseError::InvalidFormat {
                type_name: TYPE_NAME,
                reason: "invalid ULID validation: nil value not permitted".to_string(),
            });
        }
        Ok(Self(input.to_uppercase()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Converts this `InstanceId` to its 16-byte big-endian representation.
    ///
    /// # Errors
    ///
    /// Returns `ParseError::InvalidFormat` if the internal string cannot be
    /// decoded as a ULID (should never happen for a properly constructed value).
    pub fn to_bytes(&self) -> Result<[u8; 16], ParseError> {
        ulid::Ulid::from_string(&self.0)
            .map(|u| u.0.to_be_bytes())
            .map_err(|e| ParseError::InvalidFormat {
                type_name: "InstanceId",
                reason: format!("cannot convert to bytes: {e}"),
            })
    }

    #[must_use]
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(ulid::Ulid(u128::from_be_bytes(bytes)).to_string())
    }
}
string_newtype!(InstanceId);

impl WorkflowName {
    /// Parse a `WorkflowName` from an identifier string.
    ///
    /// # Errors
    ///
    /// Returns `ParseError` if the input is empty, contains invalid characters,
    /// exceeds the maximum length, or has invalid boundary characters.
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        const TYPE_NAME: &str = "WorkflowName";
        const MAX_LEN: usize = 128;
        if input.is_empty() {
            return Err(ParseError::Empty {
                type_name: TYPE_NAME,
            });
        }
        let invalid = extract_invalid_chars(input, is_identifier_char);
        if !invalid.is_empty() {
            return Err(ParseError::InvalidCharacters {
                type_name: TYPE_NAME,
                invalid_chars: invalid,
            });
        }
        let char_count = input.chars().count();
        if char_count > MAX_LEN {
            return Err(ParseError::ExceedsMaxLength {
                type_name: TYPE_NAME,
                max: MAX_LEN,
                actual: char_count,
            });
        }
        check_identifier_boundaries(input, TYPE_NAME)?;
        Ok(Self(input.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
string_newtype!(WorkflowName);

impl NodeName {
    /// Parse a `NodeName` from an identifier string.
    ///
    /// # Errors
    ///
    /// Returns `ParseError` if the input is empty, contains invalid characters,
    /// exceeds the maximum length, or has invalid boundary characters.
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        const TYPE_NAME: &str = "NodeName";
        const MAX_LEN: usize = 128;
        if input.is_empty() {
            return Err(ParseError::Empty {
                type_name: TYPE_NAME,
            });
        }
        let invalid = extract_invalid_chars(input, is_identifier_char);
        if !invalid.is_empty() {
            return Err(ParseError::InvalidCharacters {
                type_name: TYPE_NAME,
                invalid_chars: invalid,
            });
        }
        let char_count = input.chars().count();
        if char_count > MAX_LEN {
            return Err(ParseError::ExceedsMaxLength {
                type_name: TYPE_NAME,
                max: MAX_LEN,
                actual: char_count,
            });
        }
        check_identifier_boundaries(input, TYPE_NAME)?;
        Ok(Self(input.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
string_newtype!(NodeName);

impl BinaryHash {
    /// Parse a `BinaryHash` from a lowercase hex string.
    ///
    /// # Errors
    ///
    /// Returns `ParseError` if the input is empty, contains non-hex characters,
    /// has odd length, or is shorter than the minimum length.
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        const TYPE_NAME: &str = "BinaryHash";
        const MIN_LEN: usize = 8;
        if input.is_empty() {
            return Err(ParseError::Empty {
                type_name: TYPE_NAME,
            });
        }
        let invalid = extract_invalid_chars(input, is_lowercase_hex);
        if !invalid.is_empty() {
            return Err(ParseError::InvalidCharacters {
                type_name: TYPE_NAME,
                invalid_chars: invalid,
            });
        }
        if !input.len().is_multiple_of(2) {
            return Err(ParseError::InvalidFormat {
                type_name: TYPE_NAME,
                reason: "hex string has odd length".to_string(),
            });
        }
        if input.len() < MIN_LEN {
            return Err(ParseError::InvalidFormat {
                type_name: TYPE_NAME,
                reason: format!("hex string must be at least {MIN_LEN} characters (minimum)"),
            });
        }
        Ok(Self(input.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
string_newtype!(BinaryHash);

impl TimerId {
    /// Parse a `TimerId` from a string.
    ///
    /// # Errors
    ///
    /// Returns `ParseError` if the input is empty or exceeds the maximum length.
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        const TYPE_NAME: &str = "TimerId";
        const MAX_LEN: usize = 256;
        if input.is_empty() {
            return Err(ParseError::Empty {
                type_name: TYPE_NAME,
            });
        }
        let char_count = input.chars().count();
        if char_count > MAX_LEN {
            return Err(ParseError::ExceedsMaxLength {
                type_name: TYPE_NAME,
                max: MAX_LEN,
                actual: char_count,
            });
        }
        Ok(Self(input.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
string_newtype!(TimerId);

impl IdempotencyKey {
    /// Parse an `IdempotencyKey` from a string.
    ///
    /// # Errors
    ///
    /// Returns `ParseError` if the input is empty or exceeds the maximum length.
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        const TYPE_NAME: &str = "IdempotencyKey";
        const MAX_LEN: usize = 1024;
        if input.is_empty() {
            return Err(ParseError::Empty {
                type_name: TYPE_NAME,
            });
        }
        let char_count = input.chars().count();
        if char_count > MAX_LEN {
            return Err(ParseError::ExceedsMaxLength {
                type_name: TYPE_NAME,
                max: MAX_LEN,
                actual: char_count,
            });
        }
        Ok(Self(input.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
string_newtype!(IdempotencyKey);
