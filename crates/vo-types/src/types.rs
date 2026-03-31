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
    AttemptNumber, DurationMs, EventVersion, FireAtMs, MaxAttempts, SequenceNumber, TimeoutMs,
    TimestampMs,
};
pub use crate::string_types::{
    BinaryHash, IdempotencyKey, InstanceId, NodeName, TimerId, WorkflowName,
};
