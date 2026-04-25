use serde::{Deserialize, Serialize};

use crate::string_newtype;
use crate::types::{
    check_identifier_boundaries, extract_invalid_chars, is_identifier_char, is_lowercase_hex,
};
use crate::ParseError;

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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SpawnId(pub(crate) String);

impl InstanceId {
    /// Parse an `InstanceId` from a ULID string.
    ///
    /// # Errors
    ///
    /// Returns `ParseError` if the input is empty, has invalid length, or contains an invalid ULID.
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
        Ok(Self(ulid.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Convert to a 16-byte array.
    ///
    /// # Errors
    ///
    /// Returns `ParseError` if the inner string cannot be parsed as a ULID.
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
    /// Parse a `WorkflowName` from a string.
    ///
    /// # Errors
    ///
    /// Returns `ParseError` if the input is empty, exceeds max length, or contains invalid characters.
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
        if input.chars().count() > MAX_LEN {
            return Err(ParseError::ExceedsMaxLength {
                type_name: TYPE_NAME,
                max: MAX_LEN,
                actual: input.chars().count(),
            });
        }
        if input.contains("--") {
            return Err(ParseError::ConsecutiveHyphens {
                type_name: TYPE_NAME,
            });
        }
        if input.contains("__") || input.contains("-_") || input.contains("_-") {
            return Err(ParseError::ConsecutiveSeparators {
                type_name: TYPE_NAME,
            });
        }
        check_identifier_boundaries(input, TYPE_NAME)?;
        if input.chars().next().map_or(false, |c| c.is_ascii_digit()) {
            return Err(ParseError::BoundaryViolation {
                type_name: TYPE_NAME,
                reason: "must not start with digit".to_string(),
            });
        }
        Ok(Self(input.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
string_newtype!(WorkflowName);

impl NodeName {
    /// Parse a `NodeName` from a string.
    ///
    /// # Errors
    ///
    /// Returns `ParseError` if the input is empty, exceeds max length, or contains invalid characters.
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
        if input.chars().count() > MAX_LEN {
            return Err(ParseError::ExceedsMaxLength {
                type_name: TYPE_NAME,
                max: MAX_LEN,
                actual: input.chars().count(),
            });
        }
        if input.contains("--") {
            return Err(ParseError::ConsecutiveHyphens {
                type_name: TYPE_NAME,
            });
        }
        if input.contains("__") || input.contains("-_") || input.contains("_-") {
            return Err(ParseError::ConsecutiveSeparators {
                type_name: TYPE_NAME,
            });
        }
        check_identifier_boundaries(input, TYPE_NAME)?;
        if input.chars().next().map_or(false, |c| c.is_ascii_digit()) {
            return Err(ParseError::BoundaryViolation {
                type_name: TYPE_NAME,
                reason: "must not start with digit".to_string(),
            });
        }
        Ok(Self(input.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
string_newtype!(NodeName);

impl BinaryHash {
    /// Parse a `BinaryHash` from a string.
    ///
    /// # Errors
    ///
    /// Returns `ParseError` if the input is empty, not a valid lowercase hex, or has invalid length.
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
                reason: format!("hex string must be at least {MIN_LEN} characters"),
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
    /// Returns `ParseError` if the input is empty or exceeds max length.
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        const TYPE_NAME: &str = "TimerId";
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

    /// Convert to a 16-byte array.
    ///
    /// # Errors
    ///
    /// Returns `ParseError` if the inner string cannot be converted to bytes.
    pub fn to_bytes(&self) -> Result<[u8; 16], ParseError> {
        // Attempt to parse as UUID or ULID, otherwise fallback to hash if we must.
        if let Ok(u) = uuid::Uuid::parse_str(&self.0) {
            return Ok(*u.as_bytes());
        }
        if let Ok(u) = ulid::Ulid::from_string(&self.0) {
            return Ok(u.0.to_be_bytes());
        }
        // Fallback: hash
        let hash = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_DNS, self.0.as_bytes());
        Ok(*hash.as_bytes())
    }

    #[must_use]
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(uuid::Uuid::from_bytes(bytes).to_string())
    }
}
string_newtype!(TimerId);

impl IdempotencyKey {
    /// Parse an `IdempotencyKey` from a string.
    ///
    /// # Errors
    ///
    /// Returns `ParseError` if the input is empty, exceeds max length, or contains invalid characters.
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        const TYPE_NAME: &str = "IdempotencyKey";
        const MAX_LEN: usize = 1024;
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
        let invalid = extract_invalid_chars(input, is_identifier_char);
        if !invalid.is_empty() {
            return Err(ParseError::InvalidCharacters {
                type_name: TYPE_NAME,
                invalid_chars: invalid,
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

impl SpawnId {
    /// Create a new `SpawnId` from a string.
    #[must_use]
    pub fn new(s: String) -> Self {
        Self(s)
    }

    /// Parse a `SpawnId` from a string.
    ///
    /// # Errors
    /// Returns `ParseError` if the string contains invalid characters, violates boundary checks, or is empty.
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        const TYPE_NAME: &str = "SpawnId";
        if input.is_empty() {
            return Err(ParseError::InvalidCharacters {
                type_name: TYPE_NAME,
                invalid_chars: String::new(),
            });
        }
        let invalid = extract_invalid_chars(input, is_identifier_char);
        if !invalid.is_empty() {
            return Err(ParseError::InvalidCharacters {
                type_name: TYPE_NAME,
                invalid_chars: invalid,
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
string_newtype!(SpawnId);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct StepId(pub(crate) String);

impl StepId {
    /// Parse a `StepId` from a string.
    ///
    /// # Errors
    /// Returns `ParseError` if the string contains invalid characters, violates boundary checks, or is empty.
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        const TYPE_NAME: &str = "StepId";
        if input.is_empty() {
            return Err(ParseError::InvalidCharacters {
                type_name: TYPE_NAME,
                invalid_chars: String::new(),
            });
        }
        let invalid = extract_invalid_chars(input, is_identifier_char);
        if !invalid.is_empty() {
            return Err(ParseError::InvalidCharacters {
                type_name: TYPE_NAME,
                invalid_chars: invalid,
            });
        }
        check_identifier_boundaries(input, TYPE_NAME)?;
        if input.starts_with('_') {
            return Err(ParseError::BoundaryViolation {
                type_name: TYPE_NAME,
                reason: "must not start with underscore".to_string(),
            });
        }
        Ok(Self(input.to_string()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
string_newtype!(StepId);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SignalName(pub(crate) String);

impl SignalName {
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        const TYPE_NAME: &str = "SignalName";
        const MAX_LEN: usize = 256;

        if input.is_empty() {
            return Err(ParseError::Empty {
                type_name: TYPE_NAME,
            });
        }

        if input.len() > MAX_LEN {
            return Err(ParseError::ExceedsMaxLength {
                type_name: TYPE_NAME,
                max: MAX_LEN,
                actual: input.len(),
            });
        }

        if input.contains('\0') {
            return Err(ParseError::InvalidCharacters {
                type_name: TYPE_NAME,
                invalid_chars: "\\0".to_string(),
            });
        }

        let re = signal_name_regex().map_err(|e| ParseError::InternalError(e.to_string()))?;
        if !re.is_match(input) {
            return Err(ParseError::InvalidFormat {
                type_name: TYPE_NAME,
                reason: "must match ^[a-z][a-z0-9_]+$".to_string(),
            });
        }

        Ok(Self(input.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn signal_name_regex() -> Result<&'static regex::Regex, regex::Error> {
    static RE: std::sync::OnceLock<Result<regex::Regex, regex::Error>> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"^[a-z][a-z0-9_]+$"))
        .as_ref()
        .map_err(|e| e.clone())
}

string_newtype!(SignalName);
