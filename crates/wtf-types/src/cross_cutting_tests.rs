use crate::*;

macro_rules! test_integer_accepts_leading_zeros {
    ($type:ty, $name:ident) => {
        #[test]
        fn $name() {
            let result = <$type>::parse("007");
            assert_eq!(result.map(|v| v.as_u64()), Ok(7));
        }
    };
}

test_integer_accepts_leading_zeros!(SequenceNumber, sequence_number_accepts_leading_zeros);
test_integer_accepts_leading_zeros!(EventVersion, event_version_accepts_leading_zeros);
test_integer_accepts_leading_zeros!(AttemptNumber, attempt_number_accepts_leading_zeros);
test_integer_accepts_leading_zeros!(TimeoutMs, timeout_ms_accepts_leading_zeros);
test_integer_accepts_leading_zeros!(DurationMs, duration_ms_accepts_leading_zeros);
test_integer_accepts_leading_zeros!(TimestampMs, timestamp_ms_accepts_leading_zeros);
test_integer_accepts_leading_zeros!(FireAtMs, fire_at_ms_accepts_leading_zeros);
test_integer_accepts_leading_zeros!(MaxAttempts, max_attempts_accepts_leading_zeros);

macro_rules! test_integer_rejects_hex_prefix {
    ($type:ty, $name:ident, $type_name:expr) => {
        #[test]
        fn $name() {
            let result = <$type>::parse("0xFF");
            assert_eq!(
                result,
                Err(ParseError::NotAnInteger {
                    type_name: $type_name,
                    input: "0xFF".to_string(),
                })
            );
        }
    };
}

test_integer_rejects_hex_prefix!(
    SequenceNumber,
    sequence_number_rejects_hex_prefix,
    "SequenceNumber"
);
test_integer_rejects_hex_prefix!(
    EventVersion,
    event_version_rejects_hex_prefix,
    "EventVersion"
);
test_integer_rejects_hex_prefix!(
    AttemptNumber,
    attempt_number_rejects_hex_prefix,
    "AttemptNumber"
);
test_integer_rejects_hex_prefix!(TimeoutMs, timeout_ms_rejects_hex_prefix, "TimeoutMs");
test_integer_rejects_hex_prefix!(DurationMs, duration_ms_rejects_hex_prefix, "DurationMs");
test_integer_rejects_hex_prefix!(TimestampMs, timestamp_ms_rejects_hex_prefix, "TimestampMs");
test_integer_rejects_hex_prefix!(FireAtMs, fire_at_ms_rejects_hex_prefix, "FireAtMs");
test_integer_rejects_hex_prefix!(MaxAttempts, max_attempts_rejects_hex_prefix, "MaxAttempts");

macro_rules! test_integer_rejects_octal_prefix {
    ($type:ty, $name:ident, $type_name:expr) => {
        #[test]
        fn $name() {
            let result = <$type>::parse("0o77");
            assert_eq!(
                result,
                Err(ParseError::NotAnInteger {
                    type_name: $type_name,
                    input: "0o77".to_string(),
                })
            );
        }
    };
}

test_integer_rejects_octal_prefix!(
    SequenceNumber,
    sequence_number_rejects_octal_prefix,
    "SequenceNumber"
);
test_integer_rejects_octal_prefix!(
    EventVersion,
    event_version_rejects_octal_prefix,
    "EventVersion"
);
test_integer_rejects_octal_prefix!(
    AttemptNumber,
    attempt_number_rejects_octal_prefix,
    "AttemptNumber"
);
test_integer_rejects_octal_prefix!(TimeoutMs, timeout_ms_rejects_octal_prefix, "TimeoutMs");
test_integer_rejects_octal_prefix!(DurationMs, duration_ms_rejects_octal_prefix, "DurationMs");
test_integer_rejects_octal_prefix!(
    TimestampMs,
    timestamp_ms_rejects_octal_prefix,
    "TimestampMs"
);
test_integer_rejects_octal_prefix!(FireAtMs, fire_at_ms_rejects_octal_prefix, "FireAtMs");
test_integer_rejects_octal_prefix!(
    MaxAttempts,
    max_attempts_rejects_octal_prefix,
    "MaxAttempts"
);

macro_rules! test_integer_rejects_binary_prefix {
    ($type:ty, $name:ident, $type_name:expr) => {
        #[test]
        fn $name() {
            let result = <$type>::parse("0b101");
            assert_eq!(
                result,
                Err(ParseError::NotAnInteger {
                    type_name: $type_name,
                    input: "0b101".to_string(),
                })
            );
        }
    };
}

test_integer_rejects_binary_prefix!(
    SequenceNumber,
    sequence_number_rejects_binary_prefix,
    "SequenceNumber"
);
test_integer_rejects_binary_prefix!(
    EventVersion,
    event_version_rejects_binary_prefix,
    "EventVersion"
);
test_integer_rejects_binary_prefix!(
    AttemptNumber,
    attempt_number_rejects_binary_prefix,
    "AttemptNumber"
);
test_integer_rejects_binary_prefix!(TimeoutMs, timeout_ms_rejects_binary_prefix, "TimeoutMs");
test_integer_rejects_binary_prefix!(DurationMs, duration_ms_rejects_binary_prefix, "DurationMs");
test_integer_rejects_binary_prefix!(
    TimestampMs,
    timestamp_ms_rejects_binary_prefix,
    "TimestampMs"
);
test_integer_rejects_binary_prefix!(FireAtMs, fire_at_ms_rejects_binary_prefix, "FireAtMs");
test_integer_rejects_binary_prefix!(
    MaxAttempts,
    max_attempts_rejects_binary_prefix,
    "MaxAttempts"
);

macro_rules! test_integer_rejects_negative {
    ($type:ty, $name:ident, $type_name:expr) => {
        #[test]
        fn $name() {
            let result = <$type>::parse("-1");
            assert_eq!(
                result,
                Err(ParseError::NotAnInteger {
                    type_name: $type_name,
                    input: "-1".to_string(),
                })
            );
        }
    };
}

test_integer_rejects_negative!(
    SequenceNumber,
    sequence_number_rejects_negative,
    "SequenceNumber"
);
test_integer_rejects_negative!(EventVersion, event_version_rejects_negative, "EventVersion");
test_integer_rejects_negative!(
    AttemptNumber,
    attempt_number_rejects_negative,
    "AttemptNumber"
);
test_integer_rejects_negative!(TimeoutMs, timeout_ms_rejects_negative, "TimeoutMs");
test_integer_rejects_negative!(DurationMs, duration_ms_rejects_negative, "DurationMs");
test_integer_rejects_negative!(TimestampMs, timestamp_ms_rejects_negative, "TimestampMs");
test_integer_rejects_negative!(FireAtMs, fire_at_ms_rejects_negative, "FireAtMs");
test_integer_rejects_negative!(MaxAttempts, max_attempts_rejects_negative, "MaxAttempts");

macro_rules! test_integer_rejects_overflow {
    ($type:ty, $name:ident, $type_name:expr) => {
        #[test]
        fn $name() {
            let result = <$type>::parse("18446744073709551616");
            assert_eq!(
                result,
                Err(ParseError::NotAnInteger {
                    type_name: $type_name,
                    input: "18446744073709551616".to_string(),
                })
            );
        }
    };
}

test_integer_rejects_overflow!(
    SequenceNumber,
    sequence_number_rejects_overflow,
    "SequenceNumber"
);
test_integer_rejects_overflow!(EventVersion, event_version_rejects_overflow, "EventVersion");
test_integer_rejects_overflow!(
    AttemptNumber,
    attempt_number_rejects_overflow,
    "AttemptNumber"
);
test_integer_rejects_overflow!(TimeoutMs, timeout_ms_rejects_overflow, "TimeoutMs");
test_integer_rejects_overflow!(DurationMs, duration_ms_rejects_overflow, "DurationMs");
test_integer_rejects_overflow!(TimestampMs, timestamp_ms_rejects_overflow, "TimestampMs");
test_integer_rejects_overflow!(FireAtMs, fire_at_ms_rejects_overflow, "FireAtMs");
test_integer_rejects_overflow!(MaxAttempts, max_attempts_rejects_overflow, "MaxAttempts");

macro_rules! test_integer_rejects_leading_whitespace {
    ($type:ty, $name:ident, $type_name:expr) => {
        #[test]
        fn $name() {
            let result = <$type>::parse(" 42");
            assert_eq!(
                result,
                Err(ParseError::NotAnInteger {
                    type_name: $type_name,
                    input: " 42".to_string(),
                })
            );
        }
    };
}

test_integer_rejects_leading_whitespace!(
    SequenceNumber,
    sequence_number_rejects_leading_whitespace,
    "SequenceNumber"
);
test_integer_rejects_leading_whitespace!(
    EventVersion,
    event_version_rejects_leading_whitespace,
    "EventVersion"
);
test_integer_rejects_leading_whitespace!(
    AttemptNumber,
    attempt_number_rejects_leading_whitespace,
    "AttemptNumber"
);
test_integer_rejects_leading_whitespace!(
    TimeoutMs,
    timeout_ms_rejects_leading_whitespace,
    "TimeoutMs"
);
test_integer_rejects_leading_whitespace!(
    DurationMs,
    duration_ms_rejects_leading_whitespace,
    "DurationMs"
);
test_integer_rejects_leading_whitespace!(
    TimestampMs,
    timestamp_ms_rejects_leading_whitespace,
    "TimestampMs"
);
test_integer_rejects_leading_whitespace!(
    FireAtMs,
    fire_at_ms_rejects_leading_whitespace,
    "FireAtMs"
);
test_integer_rejects_leading_whitespace!(
    MaxAttempts,
    max_attempts_rejects_leading_whitespace,
    "MaxAttempts"
);

macro_rules! test_integer_rejects_float_notation {
    ($type:ty, $name:ident, $type_name:expr) => {
        #[test]
        fn $name() {
            let result = <$type>::parse("3.14");
            assert_eq!(
                result,
                Err(ParseError::NotAnInteger {
                    type_name: $type_name,
                    input: "3.14".to_string(),
                })
            );
        }
    };
}

test_integer_rejects_float_notation!(
    SequenceNumber,
    sequence_number_rejects_float_notation,
    "SequenceNumber"
);
test_integer_rejects_float_notation!(
    EventVersion,
    event_version_rejects_float_notation,
    "EventVersion"
);
test_integer_rejects_float_notation!(
    AttemptNumber,
    attempt_number_rejects_float_notation,
    "AttemptNumber"
);
test_integer_rejects_float_notation!(TimeoutMs, timeout_ms_rejects_float_notation, "TimeoutMs");
test_integer_rejects_float_notation!(DurationMs, duration_ms_rejects_float_notation, "DurationMs");
test_integer_rejects_float_notation!(
    TimestampMs,
    timestamp_ms_rejects_float_notation,
    "TimestampMs"
);
test_integer_rejects_float_notation!(FireAtMs, fire_at_ms_rejects_float_notation, "FireAtMs");
test_integer_rejects_float_notation!(
    MaxAttempts,
    max_attempts_rejects_float_notation,
    "MaxAttempts"
);
