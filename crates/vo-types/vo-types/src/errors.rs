#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    #[error("{type_name}: value must not be empty")]
    Empty { type_name: &'static str },

    #[error("{type_name}: invalid characters: {invalid_chars:?}")]
    InvalidCharacters {
        type_name: &'static str,
        invalid_chars: String,
    },

    #[error("{type_name}: invalid format: {reason}")]
    InvalidFormat {
        type_name: &'static str,
        reason: String,
    },

    #[error("{type_name}: exceeds maximum length of {max} (got {actual})")]
    ExceedsMaxLength {
        type_name: &'static str,
        max: usize,
        actual: usize,
    },

    #[error("{type_name}: {reason}")]
    BoundaryViolation {
        type_name: &'static str,
        reason: String,
    },

    #[error("{type_name}: not a valid unsigned integer: {input}")]
    NotAnInteger {
        type_name: &'static str,
        input: String,
    },

    #[error("{type_name}: value must not be zero")]
    ZeroValue { type_name: &'static str },

    #[error("{type_name}: value {value} is out of range (must be {min}..={max})")]
    OutOfRange {
        type_name: &'static str,
        value: u64,
        min: u64,
        max: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_error_empty_displays_type_name_when_formatted() {
        let err = ParseError::Empty {
            type_name: "InstanceId",
        };
        let msg = err.to_string();
        assert!(msg.contains("InstanceId: value must not be empty"));
    }

    #[test]
    fn parse_error_invalid_characters_displays_details_when_formatted() {
        let err = ParseError::InvalidCharacters {
            type_name: "WorkflowName",
            invalid_chars: " @!".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("WorkflowName: invalid characters: \" @!\""));
    }

    #[test]
    fn parse_error_invalid_format_displays_reason_when_formatted() {
        let err = ParseError::InvalidFormat {
            type_name: "InstanceId",
            reason: "expected 26 characters, got 5".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("InstanceId: invalid format: expected 26 characters, got 5"));
    }

    #[test]
    fn parse_error_exceeds_max_length_displays_bounds_when_formatted() {
        let err = ParseError::ExceedsMaxLength {
            type_name: "WorkflowName",
            max: 128,
            actual: 200,
        };
        let msg = err.to_string();
        assert!(msg.contains("WorkflowName: exceeds maximum length of 128 (got 200)"));
    }

    #[test]
    fn parse_error_boundary_violation_displays_reason_when_formatted() {
        let err = ParseError::BoundaryViolation {
            type_name: "WorkflowName",
            reason: "must not start with hyphen".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("WorkflowName: must not start with hyphen"));
    }

    #[test]
    fn parse_error_not_an_integer_displays_input_when_formatted() {
        let err = ParseError::NotAnInteger {
            type_name: "SequenceNumber",
            input: "abc".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("SequenceNumber: not a valid unsigned integer: abc"));
    }

    #[test]
    fn parse_error_zero_value_displays_type_name_when_formatted() {
        let err = ParseError::ZeroValue {
            type_name: "SequenceNumber",
        };
        let msg = err.to_string();
        assert!(msg.contains("SequenceNumber: value must not be zero"));
    }

    #[test]
    fn parse_error_out_of_range_displays_bounds_when_formatted() {
        let err = ParseError::OutOfRange {
            type_name: "MaxAttempts",
            value: 0,
            min: 1,
            max: 100,
        };
        let msg = err.to_string();
        assert!(msg.contains("MaxAttempts: value 0 is out of range (must be 1..=100)"));
    }
}
