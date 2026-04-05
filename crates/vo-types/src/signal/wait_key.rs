//! WaitKey — Opaque newtype string for signal wait keys

use serde::{Deserialize, Serialize};

use crate::ParseError;

/// Opaque string key for signal wait matching.
///
/// WaitKey is an opaque newtype because wait keys are matched by exact string
/// equality in the signal routing engine. The routing engine does not interpret
/// or validate wait keys — it simply compares them for equality.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct WaitKey(pub(crate) String);

impl WaitKey {
    /// Parse a `WaitKey` from a string.
    ///
    /// # Errors
    ///
    /// Returns `ParseError::Empty` if the input is empty.
    /// Returns `ParseError::ExceedsMaxLength` if the input exceeds 256 characters.
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        const TYPE_NAME: &str = "WaitKey";
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

    /// Returns the inner string value.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for WaitKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for WaitKey {
    type Error = ParseError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<WaitKey> for String {
    fn from(value: WaitKey) -> String {
        value.0
    }
}
