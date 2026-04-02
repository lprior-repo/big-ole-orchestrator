use std::fmt;
use std::num::NonZeroU64;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::types::{parse_nonzero_u64, parse_u64_str, require_nonzero};
use crate::ParseError;

macro_rules! nonzero_newtype {
    ($name:ident) => {
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0.get())
            }
        }
        impl TryFrom<u64> for $name {
            type Error = ParseError;
            fn try_from(value: u64) -> Result<Self, Self::Error> {
                const TN: &str = stringify!($name);
                require_nonzero(value, TN).map(Self)
            }
        }
        impl From<$name> for u64 {
            fn from(value: $name) -> u64 {
                value.0.get()
            }
        }
    };
}

macro_rules! u64_newtype {
    ($name:ident) => {
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }
        impl TryFrom<u64> for $name {
            type Error = ParseError;
            fn try_from(value: u64) -> Result<Self, Self::Error> {
                Ok(Self(value))
            }
        }
        impl From<$name> for u64 {
            fn from(value: $name) -> u64 {
                value.0
            }
        }
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct SequenceNumber(pub(crate) NonZeroU64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct EventVersion(pub(crate) NonZeroU64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct AttemptNumber(pub(crate) NonZeroU64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct TimeoutMs(pub(crate) NonZeroU64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct MaxAttempts(pub(crate) NonZeroU64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct DurationMs(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct TimestampMs(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct FireAtMs(pub(crate) u64);

impl SequenceNumber {
    /// Parse a `SequenceNumber` from a decimal string.
    ///
    /// # Errors
    ///
    /// Returns `ParseError` if the input is not a valid nonzero u64.
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        parse_nonzero_u64(input, "SequenceNumber").map(Self)
    }
    #[must_use]
    pub fn as_u64(self) -> u64 {
        self.0.get()
    }
    /// Create a `SequenceNumber` without validation.
    ///
    /// # Panics
    ///
    /// Panics if `value` is zero.
    #[must_use]
    #[allow(clippy::expect_used)] // Intentional: new_unchecked is a test-convenience constructor
    pub fn new_unchecked(value: u64) -> Self {
        Self(NonZeroU64::new(value).expect("SequenceNumber must be nonzero"))
    }
}
impl From<SequenceNumber> for NonZeroU64 {
    fn from(value: SequenceNumber) -> NonZeroU64 {
        value.0
    }
}
nonzero_newtype!(SequenceNumber);

impl EventVersion {
    /// Parse an `EventVersion` from a decimal string.
    ///
    /// # Errors
    ///
    /// Returns `ParseError` if the input is not a valid nonzero u64.
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        parse_nonzero_u64(input, "EventVersion").map(Self)
    }
    #[must_use]
    pub fn as_u64(self) -> u64 {
        self.0.get()
    }
    /// Create an `EventVersion` without validation.
    ///
    /// # Panics
    ///
    /// Panics if `value` is zero.
    #[must_use]
    #[allow(clippy::expect_used)] // Intentional: new_unchecked is a test-convenience constructor
    pub fn new_unchecked(value: u64) -> Self {
        Self(NonZeroU64::new(value).expect("EventVersion must be nonzero"))
    }
}
nonzero_newtype!(EventVersion);

impl AttemptNumber {
    /// Parse an `AttemptNumber` from a decimal string.
    ///
    /// # Errors
    ///
    /// Returns `ParseError` if the input is not a valid nonzero u64.
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        parse_nonzero_u64(input, "AttemptNumber").map(Self)
    }
    #[must_use]
    pub fn as_u64(self) -> u64 {
        self.0.get()
    }
    /// Create an `AttemptNumber` without validation.
    ///
    /// # Panics
    ///
    /// Panics if `value` is zero.
    #[must_use]
    #[allow(clippy::expect_used)] // Intentional: new_unchecked is a test-convenience constructor
    pub fn new_unchecked(value: u64) -> Self {
        Self(NonZeroU64::new(value).expect("AttemptNumber must be nonzero"))
    }
}
nonzero_newtype!(AttemptNumber);

impl TimeoutMs {
    /// Parse a `TimeoutMs` from a decimal string.
    ///
    /// # Errors
    ///
    /// Returns `ParseError` if the input is not a valid nonzero u64.
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        parse_nonzero_u64(input, "TimeoutMs").map(Self)
    }
    #[must_use]
    pub fn as_u64(self) -> u64 {
        self.0.get()
    }
    #[must_use]
    pub fn to_duration(self) -> Duration {
        Duration::from_millis(self.0.get())
    }
    /// Create a `TimeoutMs` without validation.
    ///
    /// # Panics
    ///
    /// Panics if `value` is zero.
    #[must_use]
    #[allow(clippy::expect_used)] // Intentional: new_unchecked is a test-convenience constructor
    pub fn new_unchecked(value: u64) -> Self {
        Self(NonZeroU64::new(value).expect("TimeoutMs must be nonzero"))
    }
}
nonzero_newtype!(TimeoutMs);

impl DurationMs {
    /// Parse a `DurationMs` from a decimal string.
    ///
    /// # Errors
    ///
    /// Returns `ParseError` if the input is not a valid u64.
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        parse_u64_str(input, "DurationMs").map(Self)
    }
    #[must_use]
    pub fn as_u64(self) -> u64 {
        self.0
    }
    #[must_use]
    pub fn to_duration(self) -> Duration {
        Duration::from_millis(self.0)
    }
}
u64_newtype!(DurationMs);

impl TimestampMs {
    /// Parse a `TimestampMs` from a decimal string.
    ///
    /// # Errors
    ///
    /// Returns `ParseError` if the input is not a valid u64.
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        parse_u64_str(input, "TimestampMs").map(Self)
    }
    #[must_use]
    pub fn as_u64(self) -> u64 {
        self.0
    }
    #[must_use]
    pub fn to_system_time(self) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_millis(self.0)
    }
    #[must_use]
    pub fn now() -> Self {
        let millis = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis());
        Self(
            u64::try_from(millis).expect("timestamp milliseconds exceed u64::MAX before year 584M"),
        )
    }
}
u64_newtype!(TimestampMs);

impl FireAtMs {
    /// Parse a `FireAtMs` from a decimal string.
    ///
    /// # Errors
    ///
    /// Returns `ParseError` if the input is not a valid u64.
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        parse_u64_str(input, "FireAtMs").map(Self)
    }
    #[must_use]
    pub fn as_u64(self) -> u64 {
        self.0
    }
    #[must_use]
    pub fn to_system_time(self) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_millis(self.0)
    }
    #[must_use]
    pub fn has_elapsed(self, now: TimestampMs) -> bool {
        self.0 < now.0
    }
}
u64_newtype!(FireAtMs);

impl MaxAttempts {
    /// Parse a `MaxAttempts` from a decimal string.
    ///
    /// # Errors
    ///
    /// Returns `ParseError` if the input is not a valid nonzero u64.
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        parse_nonzero_u64(input, "MaxAttempts").map(Self)
    }
    #[must_use]
    pub fn as_u64(self) -> u64 {
        self.0.get()
    }
    #[must_use]
    pub fn is_exhausted(self, attempt: AttemptNumber) -> bool {
        attempt.as_u64() >= self.0.get()
    }
    /// Create a `MaxAttempts` without validation.
    ///
    /// # Panics
    ///
    /// Panics if `value` is zero.
    #[must_use]
    #[allow(clippy::expect_used)] // Intentional: new_unchecked is a test-convenience constructor
    pub fn new_unchecked(value: u64) -> Self {
        Self(NonZeroU64::new(value).expect("MaxAttempts must be nonzero"))
    }
}
nonzero_newtype!(MaxAttempts);
