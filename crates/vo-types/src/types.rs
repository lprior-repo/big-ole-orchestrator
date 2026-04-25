pub(crate) fn extract_invalid_chars(input: &str, is_valid: impl Fn(char) -> bool) -> String {
    input.chars().filter(|&c| !is_valid(c)).collect()
}

pub(crate) fn parse_u64_str(
    input: &str,
    type_name: &'static str,
) -> Result<u64, crate::ParseError> {
    input
        .parse::<u64>()
        .map_err(|_| crate::ParseError::NotAnInteger {
            type_name,
            input: input.to_string(),
        })
}

pub(crate) fn require_nonzero(
    value: u64,
    type_name: &'static str,
) -> Result<std::num::NonZeroU64, crate::ParseError> {
    std::num::NonZeroU64::new(value).ok_or(crate::ParseError::ZeroValue { type_name })
}

pub(crate) fn parse_nonzero_u64(
    input: &str,
    type_name: &'static str,
) -> Result<std::num::NonZeroU64, crate::ParseError> {
    let value = parse_u64_str(input, type_name)?;
    require_nonzero(value, type_name)
}

pub(crate) fn is_identifier_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_'
}

pub(crate) fn is_lowercase_hex(c: char) -> bool {
    matches!(c, '0'..='9' | 'a'..='f')
}

pub(crate) fn check_identifier_boundaries(
    input: &str,
    type_name: &'static str,
) -> Result<(), crate::ParseError> {
    let first = input.chars().next();
    let last = input.chars().next_back();

    match (first, last) {
        (Some('-'), _) => Err(crate::ParseError::BoundaryViolation {
            type_name,
            reason: "must not start with hyphen".to_string(),
        }),
        (_, Some('-')) => Err(crate::ParseError::BoundaryViolation {
            type_name,
            reason: "must not end with hyphen".to_string(),
        }),
        (_, Some('_')) => Err(crate::ParseError::BoundaryViolation {
            type_name,
            reason: "must not end with underscore".to_string(),
        }),
        _ => Ok(()),
    }
}

pub use crate::integer_types::{
    AttemptNumber, DurationMs, EventVersion, FenceToken, FireAtMs, MaxAttempts, SequenceNumber,
    TimeoutMs, TimestampMs,
};
pub use crate::state::LeaseRecord;
pub use crate::string_types::{
    BinaryHash, IdempotencyKey, InstanceId, NodeName, SignalName, SpawnId, StepId, TimerId,
    WorkflowName,
};

pub const MAX_SUPPORTED_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, PartialEq, Clone)]
pub struct State {
    pub version: u16,
}
impl Default for State {
    fn default() -> Self {
        Self {
            version: MAX_SUPPORTED_SCHEMA_VERSION,
        }
    }
}
impl State {
    #[must_use]
    pub const fn version(&self) -> u16 {
        self.version
    }
}
impl serde::Serialize for State {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("State", 1)?;
        state.serialize_field("version", &self.version)?;
        state.end()
    }
}
impl<'de> serde::Deserialize<'de> for State {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let version = extract_schema_version(&value, Some(0)).map_err(serde::de::Error::custom)?;
        Ok(Self { version })
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct WorkflowSpec {
    pub version: u16,
}
impl Default for WorkflowSpec {
    fn default() -> Self {
        Self {
            version: MAX_SUPPORTED_SCHEMA_VERSION,
        }
    }
}
impl WorkflowSpec {
    #[must_use]
    pub const fn version(&self) -> u16 {
        self.version
    }
}
impl serde::Serialize for WorkflowSpec {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("WorkflowSpec", 1)?;
        state.serialize_field("version", &self.version)?;
        state.end()
    }
}
impl<'de> serde::Deserialize<'de> for WorkflowSpec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let version = extract_schema_version(&value, None).map_err(serde::de::Error::custom)?;
        Ok(Self { version })
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Snapshot {
    pub version: u16,
}
impl Default for Snapshot {
    fn default() -> Self {
        Self {
            version: MAX_SUPPORTED_SCHEMA_VERSION,
        }
    }
}
impl Snapshot {
    #[must_use]
    pub const fn version(&self) -> u16 {
        self.version
    }
}
impl serde::Serialize for Snapshot {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("Snapshot", 1)?;
        state.serialize_field("version", &self.version)?;
        state.end()
    }
}
impl<'de> serde::Deserialize<'de> for Snapshot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let version = extract_schema_version(&value, None).map_err(serde::de::Error::custom)?;
        Ok(Self { version })
    }
}

/// Extracts the schema version from a JSON payload.
///
/// # Errors
/// Returns an error if the payload is not an object, if the version field is missing and no fallback is provided,
/// or if the version field is invalid or unsupported.
pub fn extract_schema_version(
    payload: &serde_json::Value,
    fallback_policy: Option<u16>,
) -> Result<u16, crate::events::Error> {
    let obj = payload
        .as_object()
        .ok_or(crate::events::Error::InvalidSchemaVersionFormat)?;

    match obj.get("version") {
        Some(v) => {
            let version = v
                .as_u64()
                .ok_or(crate::events::Error::InvalidSchemaVersionFormat)?;
            if version > u64::from(u16::MAX) {
                return Err(crate::events::Error::InvalidSchemaVersionFormat);
            }
            #[allow(clippy::cast_possible_truncation)]
            let version = version as u16;
            if version > MAX_SUPPORTED_SCHEMA_VERSION {
                return Err(crate::events::Error::UnsupportedSchemaVersion(version));
            }
            Ok(version)
        }
        None => fallback_policy.ok_or(crate::events::Error::MissingSchemaVersion),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_invalid_chars_returns_invalid_characters_when_present() {
        assert_eq!(extract_invalid_chars("a-b_c", is_identifier_char), "");
        assert_eq!(extract_invalid_chars("a b!c", is_identifier_char), " !");
        assert_eq!(extract_invalid_chars("0123abcd", is_lowercase_hex), "");
        assert_eq!(extract_invalid_chars("0123abcdG", is_lowercase_hex), "G");
    }

    #[test]
    fn parse_u64_str_returns_integer_when_valid_and_error_when_invalid() {
        assert_eq!(parse_u64_str("123", "Test").unwrap(), 123);
        assert!(matches!(
            parse_u64_str("abc", "Test"),
            Err(crate::ParseError::NotAnInteger { .. })
        ));
    }

    #[test]
    fn require_nonzero_returns_nonzero_when_greater_than_zero() {
        assert_eq!(require_nonzero(123, "Test").unwrap().get(), 123);
        assert!(matches!(
            require_nonzero(0, "Test"),
            Err(crate::ParseError::ZeroValue { .. })
        ));
    }

    #[test]
    fn parse_nonzero_u64_returns_nonzero_when_valid_and_greater_than_zero() {
        assert_eq!(parse_nonzero_u64("123", "Test").unwrap().get(), 123);
        assert!(matches!(
            parse_nonzero_u64("0", "Test"),
            Err(crate::ParseError::ZeroValue { .. })
        ));
        assert!(matches!(
            parse_nonzero_u64("abc", "Test"),
            Err(crate::ParseError::NotAnInteger { .. })
        ));
    }

    #[test]
    fn check_identifier_boundaries_returns_error_when_boundaries_invalid() {
        assert_eq!(check_identifier_boundaries("valid-id", "Test"), Ok(()));
        assert_eq!(check_identifier_boundaries("valid_id", "Test"), Ok(()));
        assert!(matches!(
            check_identifier_boundaries("-invalid", "Test"),
            Err(crate::ParseError::BoundaryViolation { .. })
        ));
        assert!(matches!(
            check_identifier_boundaries("invalid-", "Test"),
            Err(crate::ParseError::BoundaryViolation { .. })
        ));
        assert!(matches!(
            check_identifier_boundaries("invalid_", "Test"),
            Err(crate::ParseError::BoundaryViolation { .. })
        ));
    }

    #[test]
    fn state_serializes_with_schema_version_when_serialized() {
        let state = State { version: 1 };
        let json = serde_json::to_string(&state).unwrap();
        assert_eq!(json, r#"{"version":1}"#);

        let deserialized: State = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, state);
        assert_eq!(state.version(), 1);

        let default_state = State::default();
        assert_eq!(default_state.version(), MAX_SUPPORTED_SCHEMA_VERSION);
    }

    #[test]
    fn workflow_spec_serializes_with_schema_version_when_serialized() {
        let spec = WorkflowSpec { version: 1 };
        let json = serde_json::to_string(&spec).unwrap();
        assert_eq!(json, r#"{"version":1}"#);

        let deserialized: WorkflowSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, spec);
        assert_eq!(spec.version(), 1);

        let default_spec = WorkflowSpec::default();
        assert_eq!(default_spec.version(), MAX_SUPPORTED_SCHEMA_VERSION);
    }

    #[test]
    fn snapshot_serializes_with_schema_version_when_serialized() {
        let snap = Snapshot { version: 1 };
        let json = serde_json::to_string(&snap).unwrap();
        assert_eq!(json, r#"{"version":1}"#);

        let deserialized: Snapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, snap);
        assert_eq!(snap.version(), 1);

        let default_snap = Snapshot::default();
        assert_eq!(default_snap.version(), MAX_SUPPORTED_SCHEMA_VERSION);
    }

    #[test]
    fn extract_schema_version_returns_version_when_valid() {
        // Happy path
        assert_eq!(extract_schema_version(&json!({"version": 1}), None), Ok(1));

        // Fallback
        assert_eq!(extract_schema_version(&json!({}), Some(0)), Ok(0));

        // Errors
        assert_eq!(
            extract_schema_version(&json!("not an object"), None),
            Err(crate::events::Error::InvalidSchemaVersionFormat)
        );
        assert_eq!(
            extract_schema_version(&json!({}), None),
            Err(crate::events::Error::MissingSchemaVersion)
        );
        assert_eq!(
            extract_schema_version(&json!({"version": "1"}), None),
            Err(crate::events::Error::InvalidSchemaVersionFormat)
        );
        assert_eq!(
            extract_schema_version(&json!({"version": -1}), None),
            Err(crate::events::Error::InvalidSchemaVersionFormat)
        );
        assert_eq!(
            extract_schema_version(&json!({"version": 999999}), None),
            Err(crate::events::Error::InvalidSchemaVersionFormat)
        );
        assert_eq!(
            extract_schema_version(&json!({"version": 2}), None),
            Err(crate::events::Error::UnsupportedSchemaVersion(2))
        );
    }
}
