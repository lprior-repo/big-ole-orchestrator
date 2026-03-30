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
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        parse_nonzero_u64(input, "SequenceNumber").map(Self)
    }
    pub fn as_u64(self) -> u64 {
        self.0.get()
    }
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
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        parse_nonzero_u64(input, "EventVersion").map(Self)
    }
    pub fn as_u64(self) -> u64 {
        self.0.get()
    }
    pub fn new_unchecked(value: u64) -> Self {
        Self(NonZeroU64::new(value).expect("EventVersion must be nonzero"))
    }
}
nonzero_newtype!(EventVersion);

impl AttemptNumber {
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        parse_nonzero_u64(input, "AttemptNumber").map(Self)
    }
    pub fn as_u64(self) -> u64 {
        self.0.get()
    }
    pub fn new_unchecked(value: u64) -> Self {
        Self(NonZeroU64::new(value).expect("AttemptNumber must be nonzero"))
    }
}
nonzero_newtype!(AttemptNumber);

impl TimeoutMs {
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        parse_nonzero_u64(input, "TimeoutMs").map(Self)
    }
    pub fn as_u64(self) -> u64 {
        self.0.get()
    }
    pub fn to_duration(self) -> Duration {
        Duration::from_millis(self.0.get())
    }
    pub fn new_unchecked(value: u64) -> Self {
        Self(NonZeroU64::new(value).expect("TimeoutMs must be nonzero"))
    }
}
nonzero_newtype!(TimeoutMs);

impl DurationMs {
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        parse_u64_str(input, "DurationMs").map(Self)
    }
    pub fn as_u64(self) -> u64 {
        self.0
    }
    pub fn to_duration(self) -> Duration {
        Duration::from_millis(self.0)
    }
}
u64_newtype!(DurationMs);

impl TimestampMs {
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        parse_u64_str(input, "TimestampMs").map(Self)
    }
    pub fn as_u64(self) -> u64 {
        self.0
    }
    pub fn to_system_time(self) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_millis(self.0)
    }
    pub fn now() -> Self {
        Self(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() as u64),
        )
    }
}
u64_newtype!(TimestampMs);

impl FireAtMs {
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        parse_u64_str(input, "FireAtMs").map(Self)
    }
    pub fn as_u64(self) -> u64 {
        self.0
    }
    pub fn to_system_time(self) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_millis(self.0)
    }
    pub fn has_elapsed(self, now: TimestampMs) -> bool {
        self.0 < now.0
    }
}
u64_newtype!(FireAtMs);

impl MaxAttempts {
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        parse_nonzero_u64(input, "MaxAttempts").map(Self)
    }
    pub fn as_u64(self) -> u64 {
        self.0.get()
    }
    pub fn is_exhausted(self, attempt: AttemptNumber) -> bool {
        attempt.as_u64() >= self.0.get()
    }
    pub fn new_unchecked(value: u64) -> Self {
        Self(NonZeroU64::new(value).expect("MaxAttempts must be nonzero"))
    }
}
nonzero_newtype!(MaxAttempts);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ParseError;
    use std::num::NonZeroU64;
    use std::time::{Duration, SystemTime};

    // ========== SequenceNumber ==========

    #[test]
    fn sequence_number_accepts_valid_nonzero_decimal_when_input_parses() {
        let sn = SequenceNumber::parse("42").expect("valid");
        assert_eq!(sn.as_u64(), 42);
    }

    #[test]
    fn sequence_number_accepts_u64_max_when_at_upper_boundary() {
        let sn = SequenceNumber::parse("18446744073709551615").expect("valid");
        assert_eq!(sn.as_u64(), u64::MAX);
    }

    #[test]
    fn sequence_number_rejects_non_integer_with_not_an_integer_when_input_is_alpha() {
        assert_eq!(
            SequenceNumber::parse("abc"),
            Err(ParseError::NotAnInteger {
                type_name: "SequenceNumber",
                input: "abc".to_string(),
            })
        );
    }

    #[test]
    fn sequence_number_rejects_zero_with_zero_value_when_input_is_zero() {
        assert_eq!(
            SequenceNumber::parse("0"),
            Err(ParseError::ZeroValue {
                type_name: "SequenceNumber"
            })
        );
    }

    #[test]
    fn sequence_number_accepts_minimum_when_value_is_1() {
        let sn = SequenceNumber::parse("1").expect("valid");
        assert_eq!(sn.as_u64(), 1);
    }

    #[test]
    fn sequence_number_display_equals_decimal() {
        let sn = SequenceNumber::new_unchecked(42);
        assert_eq!(format!("{sn}"), "42");
    }

    #[test]
    fn sequence_number_display_round_trips_through_parse_when_valid() {
        let sn = SequenceNumber::new_unchecked(42);
        let s = format!("{sn}");
        assert_eq!(SequenceNumber::parse(&s), Ok(sn));
    }

    #[test]
    fn sequence_number_new_unchecked_constructs_when_value_is_nonzero() {
        let sn = SequenceNumber::new_unchecked(42);
        assert_eq!(sn.as_u64(), 42);
    }

    #[test]
    #[should_panic(expected = "SequenceNumber must be nonzero")]
    fn sequence_number_new_unchecked_panics_when_value_is_zero() {
        SequenceNumber::new_unchecked(0);
    }

    #[test]
    fn from_sequence_number_returns_correct_nonzero_u64_when_converted() {
        let sn = SequenceNumber::new_unchecked(42);
        let nz: NonZeroU64 = sn.into();
        assert_eq!(nz.get(), 42);
    }

    // ========== EventVersion ==========

    #[test]
    fn event_version_accepts_valid_nonzero_decimal_when_input_parses() {
        let ev = EventVersion::parse("1").expect("valid");
        assert_eq!(ev.as_u64(), 1);
    }

    #[test]
    fn event_version_accepts_u64_max_when_at_upper_boundary() {
        let ev = EventVersion::parse("18446744073709551615").expect("valid");
        assert_eq!(ev.as_u64(), u64::MAX);
    }

    #[test]
    fn event_version_rejects_non_integer_with_not_an_integer_when_input_is_alpha() {
        assert_eq!(
            EventVersion::parse("not-a-version"),
            Err(ParseError::NotAnInteger {
                type_name: "EventVersion",
                input: "not-a-version".to_string(),
            })
        );
    }

    #[test]
    fn event_version_rejects_zero_with_zero_value_when_input_is_zero() {
        assert_eq!(
            EventVersion::parse("0"),
            Err(ParseError::ZeroValue {
                type_name: "EventVersion"
            })
        );
    }

    #[test]
    fn event_version_accepts_minimum_when_value_is_1() {
        let ev = EventVersion::parse("1").expect("valid");
        assert_eq!(ev.as_u64(), 1);
    }

    #[test]
    fn event_version_display_equals_decimal() {
        let ev = EventVersion::new_unchecked(1);
        assert_eq!(format!("{ev}"), "1");
    }

    #[test]
    fn event_version_display_round_trips_through_parse_when_valid() {
        let ev = EventVersion::new_unchecked(1);
        let s = format!("{ev}");
        assert_eq!(EventVersion::parse(&s), Ok(ev));
    }

    #[test]
    fn event_version_new_unchecked_constructs_when_value_is_nonzero() {
        let ev = EventVersion::new_unchecked(1);
        assert_eq!(ev.as_u64(), 1);
    }

    #[test]
    #[should_panic(expected = "EventVersion must be nonzero")]
    fn event_version_new_unchecked_panics_when_value_is_zero() {
        EventVersion::new_unchecked(0);
    }

    // ========== AttemptNumber ==========

    #[test]
    fn attempt_number_accepts_valid_nonzero_decimal_when_input_parses() {
        let an = AttemptNumber::parse("1").expect("valid");
        assert_eq!(an.as_u64(), 1);
    }

    #[test]
    fn attempt_number_accepts_u64_max_when_at_upper_boundary() {
        let an = AttemptNumber::parse("18446744073709551615").expect("valid");
        assert_eq!(an.as_u64(), u64::MAX);
    }

    #[test]
    fn attempt_number_rejects_non_integer_with_not_an_integer_when_input_is_alpha() {
        assert_eq!(
            AttemptNumber::parse("retry"),
            Err(ParseError::NotAnInteger {
                type_name: "AttemptNumber",
                input: "retry".to_string(),
            })
        );
    }

    #[test]
    fn attempt_number_rejects_zero_with_zero_value_when_input_is_zero() {
        assert_eq!(
            AttemptNumber::parse("0"),
            Err(ParseError::ZeroValue {
                type_name: "AttemptNumber"
            })
        );
    }

    #[test]
    fn attempt_number_accepts_minimum_when_value_is_1() {
        let an = AttemptNumber::parse("1").expect("valid");
        assert_eq!(an.as_u64(), 1);
    }

    #[test]
    fn attempt_number_display_equals_decimal() {
        let an = AttemptNumber::new_unchecked(3);
        assert_eq!(format!("{an}"), "3");
    }

    #[test]
    fn attempt_number_display_round_trips_through_parse_when_valid() {
        let an = AttemptNumber::new_unchecked(3);
        let s = format!("{an}");
        assert_eq!(AttemptNumber::parse(&s), Ok(an));
    }

    #[test]
    fn attempt_number_new_unchecked_constructs_when_value_is_nonzero() {
        let an = AttemptNumber::new_unchecked(1);
        assert_eq!(an.as_u64(), 1);
    }

    #[test]
    #[should_panic(expected = "AttemptNumber must be nonzero")]
    fn attempt_number_new_unchecked_panics_when_value_is_zero() {
        AttemptNumber::new_unchecked(0);
    }

    // ========== TimeoutMs ==========

    #[test]
    fn timeout_ms_accepts_valid_nonzero_decimal_when_input_parses() {
        let tm = TimeoutMs::parse("5000").expect("valid");
        assert_eq!(tm.as_u64(), 5000);
    }

    #[test]
    fn timeout_ms_accepts_minimum_when_value_is_1() {
        let tm = TimeoutMs::parse("1").expect("valid");
        assert_eq!(tm.as_u64(), 1);
    }

    #[test]
    fn timeout_ms_rejects_non_integer_with_not_an_integer_when_input_is_duration_string() {
        assert_eq!(
            TimeoutMs::parse("5s"),
            Err(ParseError::NotAnInteger {
                type_name: "TimeoutMs",
                input: "5s".to_string(),
            })
        );
    }

    #[test]
    fn timeout_ms_rejects_zero_with_zero_value_when_input_is_zero() {
        assert_eq!(
            TimeoutMs::parse("0"),
            Err(ParseError::ZeroValue {
                type_name: "TimeoutMs"
            })
        );
    }

    #[test]
    fn timeout_ms_to_duration_returns_correct_duration_when_called() {
        let tm = TimeoutMs::new_unchecked(5000);
        assert_eq!(tm.to_duration(), Duration::from_millis(5000));
    }

    #[test]
    fn timeout_ms_accepts_u64_max_when_at_upper_boundary() {
        let tm = TimeoutMs::parse("18446744073709551615").expect("valid");
        assert_eq!(tm.as_u64(), u64::MAX);
    }

    #[test]
    fn timeout_ms_rejects_negative_with_not_an_integer_when_input_starts_with_minus() {
        assert_eq!(
            TimeoutMs::parse("-1"),
            Err(ParseError::NotAnInteger {
                type_name: "TimeoutMs",
                input: "-1".to_string(),
            })
        );
    }

    #[test]
    fn timeout_ms_display_equals_decimal() {
        let tm = TimeoutMs::new_unchecked(5000);
        assert_eq!(format!("{tm}"), "5000");
    }

    #[test]
    fn timeout_ms_display_round_trips_through_parse_when_valid() {
        let tm = TimeoutMs::new_unchecked(5000);
        let s = format!("{tm}");
        assert_eq!(TimeoutMs::parse(&s), Ok(tm));
    }

    #[test]
    fn timeout_ms_new_unchecked_constructs_when_value_is_nonzero() {
        let tm = TimeoutMs::new_unchecked(1000);
        assert_eq!(tm.as_u64(), 1000);
    }

    #[test]
    #[should_panic(expected = "TimeoutMs must be nonzero")]
    fn timeout_ms_new_unchecked_panics_when_value_is_zero() {
        TimeoutMs::new_unchecked(0);
    }

    // ========== DurationMs ==========

    #[test]
    fn duration_ms_accepts_zero_when_input_is_zero() {
        let dm = DurationMs::parse("0").expect("valid");
        assert_eq!(dm.as_u64(), 0);
    }

    #[test]
    fn duration_ms_accepts_nonzero_decimal_when_input_parses() {
        let dm = DurationMs::parse("1500").expect("valid");
        assert_eq!(dm.as_u64(), 1500);
    }

    #[test]
    fn duration_ms_accepts_u64_max_when_at_upper_boundary() {
        let dm = DurationMs::parse("18446744073709551615").expect("valid");
        assert_eq!(dm.as_u64(), u64::MAX);
    }

    #[test]
    fn duration_ms_rejects_non_integer_with_not_an_integer_when_input_is_float_string() {
        assert_eq!(
            DurationMs::parse("1.5s"),
            Err(ParseError::NotAnInteger {
                type_name: "DurationMs",
                input: "1.5s".to_string(),
            })
        );
    }

    #[test]
    fn duration_ms_to_duration_returns_zero_duration_when_value_is_zero() {
        let dm = DurationMs(0);
        assert_eq!(dm.to_duration(), Duration::from_millis(0));
    }

    #[test]
    fn duration_ms_to_duration_returns_correct_duration_when_value_is_nonzero() {
        let dm = DurationMs(2000);
        assert_eq!(dm.to_duration(), Duration::from_millis(2000));
    }

    #[test]
    fn duration_ms_rejects_negative_with_not_an_integer_when_input_starts_with_minus() {
        assert_eq!(
            DurationMs::parse("-1"),
            Err(ParseError::NotAnInteger {
                type_name: "DurationMs",
                input: "-1".to_string(),
            })
        );
    }

    #[test]
    fn duration_ms_display_equals_decimal() {
        let dm = DurationMs(1500);
        assert_eq!(format!("{dm}"), "1500");
    }

    #[test]
    fn duration_ms_display_round_trips_through_parse_when_valid() {
        let dm = DurationMs(1500);
        let s = format!("{dm}");
        assert_eq!(DurationMs::parse(&s), Ok(dm));
    }

    // ========== TimestampMs ==========

    #[test]
    fn timestamp_ms_accepts_zero_when_input_is_zero() {
        let ts = TimestampMs::parse("0").expect("valid");
        assert_eq!(ts.as_u64(), 0);
    }

    #[test]
    fn timestamp_ms_accepts_nonzero_decimal_when_input_parses() {
        let ts = TimestampMs::parse("1710000000000").expect("valid");
        assert_eq!(ts.as_u64(), 1710000000000);
    }

    #[test]
    fn timestamp_ms_rejects_non_integer_with_not_an_integer_when_input_is_alpha() {
        assert_eq!(
            TimestampMs::parse("now"),
            Err(ParseError::NotAnInteger {
                type_name: "TimestampMs",
                input: "now".to_string(),
            })
        );
    }

    #[test]
    fn timestamp_ms_to_system_time_returns_unix_epoch_when_value_is_zero() {
        let ts = TimestampMs(0);
        assert_eq!(ts.to_system_time(), SystemTime::UNIX_EPOCH);
    }

    #[test]
    fn timestamp_ms_to_system_time_returns_correct_time_when_value_is_positive() {
        let ts = TimestampMs(1000);
        assert_eq!(
            ts.to_system_time(),
            SystemTime::UNIX_EPOCH + Duration::from_millis(1000)
        );
    }

    #[test]
    fn timestamp_ms_now_returns_parseable_value_when_system_clock_available() {
        let ts = TimestampMs::now();
        let parsed = TimestampMs::parse(&ts.to_string()).expect("parseable");
        assert_eq!(parsed.as_u64(), ts.as_u64());
    }

    #[test]
    fn timestamp_ms_now_is_approximately_current_time_when_called() {
        let ts = TimestampMs::now();
        let system_ms = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system clock")
            .as_millis() as u64;
        let diff = if ts.as_u64() > system_ms {
            ts.as_u64() - system_ms
        } else {
            system_ms - ts.as_u64()
        };
        assert!(
            diff < 5000,
            "TimestampMs::now() should be within 5000ms of system time, was {diff}ms off"
        );
    }

    #[test]
    fn timestamp_ms_accepts_u64_max_when_at_upper_boundary() {
        let ts = TimestampMs::parse("18446744073709551615").expect("valid");
        assert_eq!(ts.as_u64(), u64::MAX);
    }

    #[test]
    fn timestamp_ms_rejects_negative_with_not_an_integer_when_input_starts_with_minus() {
        assert_eq!(
            TimestampMs::parse("-1"),
            Err(ParseError::NotAnInteger {
                type_name: "TimestampMs",
                input: "-1".to_string(),
            })
        );
    }

    #[test]
    fn timestamp_ms_display_equals_decimal() {
        let ts = TimestampMs(1710000000000);
        assert_eq!(format!("{ts}"), "1710000000000");
    }

    #[test]
    fn timestamp_ms_display_round_trips_through_parse_when_valid() {
        let ts = TimestampMs(1710000000000);
        let s = format!("{ts}");
        assert_eq!(TimestampMs::parse(&s), Ok(ts));
    }

    // ========== FireAtMs ==========

    #[test]
    fn fire_at_ms_accepts_zero_when_input_is_zero() {
        let fa = FireAtMs::parse("0").expect("valid");
        assert_eq!(fa.as_u64(), 0);
    }

    #[test]
    fn fire_at_ms_accepts_nonzero_decimal_when_input_parses() {
        let fa = FireAtMs::parse("1710000000000").expect("valid");
        assert_eq!(fa.as_u64(), 1710000000000);
    }

    #[test]
    fn fire_at_ms_rejects_non_integer_with_not_an_integer_when_input_is_alpha() {
        assert_eq!(
            FireAtMs::parse("soon"),
            Err(ParseError::NotAnInteger {
                type_name: "FireAtMs",
                input: "soon".to_string(),
            })
        );
    }

    #[test]
    fn fire_at_ms_to_system_time_returns_correct_time_when_called() {
        let fa = FireAtMs(5000);
        assert_eq!(
            fa.to_system_time(),
            SystemTime::UNIX_EPOCH + Duration::from_millis(5000)
        );
    }

    #[test]
    fn fire_at_ms_has_elapsed_returns_true_when_fire_at_is_before_now() {
        let fa = FireAtMs(1000);
        let now = TimestampMs(2000);
        assert!(fa.has_elapsed(now));
    }

    #[test]
    fn fire_at_ms_has_elapsed_returns_false_when_fire_at_is_after_now() {
        let fa = FireAtMs(3000);
        let now = TimestampMs(2000);
        assert!(!fa.has_elapsed(now));
    }

    #[test]
    fn fire_at_ms_has_elapsed_returns_false_when_fire_at_equals_now() {
        let fa = FireAtMs(2000);
        let now = TimestampMs(2000);
        assert!(
            !fa.has_elapsed(now),
            "has_elapsed must be false when times are equal"
        );
    }

    #[test]
    fn fire_at_ms_accepts_u64_max_when_at_upper_boundary() {
        let fa = FireAtMs::parse("18446744073709551615").expect("valid");
        assert_eq!(fa.as_u64(), u64::MAX);
    }

    #[test]
    fn fire_at_ms_rejects_negative_with_not_an_integer_when_input_starts_with_minus() {
        assert_eq!(
            FireAtMs::parse("-1"),
            Err(ParseError::NotAnInteger {
                type_name: "FireAtMs",
                input: "-1".to_string(),
            })
        );
    }

    #[test]
    fn fire_at_ms_display_equals_decimal() {
        let fa = FireAtMs(1710000000000);
        assert_eq!(format!("{fa}"), "1710000000000");
    }

    #[test]
    fn fire_at_ms_display_round_trips_through_parse_when_valid() {
        let fa = FireAtMs(1710000000000);
        let s = format!("{fa}");
        assert_eq!(FireAtMs::parse(&s), Ok(fa));
    }

    // ========== MaxAttempts ==========

    #[test]
    fn max_attempts_accepts_valid_nonzero_decimal_when_input_parses() {
        let ma = MaxAttempts::parse("3").expect("valid");
        assert_eq!(ma.as_u64(), 3);
    }

    #[test]
    fn max_attempts_accepts_minimum_when_value_is_1() {
        let ma = MaxAttempts::parse("1").expect("valid");
        assert_eq!(ma.as_u64(), 1);
    }

    #[test]
    fn max_attempts_rejects_non_integer_with_not_an_integer_when_input_is_alpha() {
        assert_eq!(
            MaxAttempts::parse("unlimited"),
            Err(ParseError::NotAnInteger {
                type_name: "MaxAttempts",
                input: "unlimited".to_string(),
            })
        );
    }

    #[test]
    fn max_attempts_rejects_zero_with_zero_value_when_input_is_zero() {
        assert_eq!(
            MaxAttempts::parse("0"),
            Err(ParseError::ZeroValue {
                type_name: "MaxAttempts"
            })
        );
    }

    #[test]
    fn max_attempts_is_exhausted_returns_false_when_attempt_less_than_max() {
        let ma = MaxAttempts::new_unchecked(3);
        let attempt = AttemptNumber::new_unchecked(1);
        assert!(!ma.is_exhausted(attempt));
    }

    #[test]
    fn max_attempts_is_exhausted_returns_false_when_attempt_is_max_minus_one() {
        let ma = MaxAttempts::new_unchecked(3);
        let attempt = AttemptNumber::new_unchecked(2);
        assert!(!ma.is_exhausted(attempt));
    }

    #[test]
    fn max_attempts_is_exhausted_returns_true_when_attempt_equals_max() {
        let ma = MaxAttempts::new_unchecked(3);
        let attempt = AttemptNumber::new_unchecked(3);
        assert!(ma.is_exhausted(attempt));
    }

    #[test]
    fn max_attempts_is_exhausted_returns_true_when_attempt_exceeds_max() {
        let ma = MaxAttempts::new_unchecked(3);
        let attempt = AttemptNumber::new_unchecked(5);
        assert!(ma.is_exhausted(attempt));
    }

    #[test]
    fn max_attempts_is_exhausted_returns_true_when_max_is_1_and_attempt_is_1() {
        let ma = MaxAttempts::new_unchecked(1);
        let attempt = AttemptNumber::new_unchecked(1);
        assert!(ma.is_exhausted(attempt));
    }

    #[test]
    fn max_attempts_accepts_u64_max_when_at_upper_boundary() {
        let ma = MaxAttempts::parse("18446744073709551615").expect("valid");
        assert_eq!(ma.as_u64(), u64::MAX);
    }

    #[test]
    fn max_attempts_rejects_negative_with_not_an_integer_when_input_starts_with_minus() {
        assert_eq!(
            MaxAttempts::parse("-1"),
            Err(ParseError::NotAnInteger {
                type_name: "MaxAttempts",
                input: "-1".to_string(),
            })
        );
    }

    #[test]
    fn max_attempts_display_equals_decimal() {
        let ma = MaxAttempts::new_unchecked(3);
        assert_eq!(format!("{ma}"), "3");
    }

    #[test]
    fn max_attempts_display_round_trips_through_parse_when_valid() {
        let ma = MaxAttempts::new_unchecked(3);
        let s = format!("{ma}");
        assert_eq!(MaxAttempts::parse(&s), Ok(ma));
    }

    #[test]
    fn max_attempts_new_unchecked_constructs_when_value_is_nonzero() {
        let ma = MaxAttempts::new_unchecked(3);
        assert_eq!(ma.as_u64(), 3);
    }

    #[test]
    #[should_panic(expected = "MaxAttempts must be nonzero")]
    fn max_attempts_new_unchecked_panics_when_value_is_zero() {
        MaxAttempts::new_unchecked(0);
    }

    // ========== Serde round-trip (inline) ==========

    #[test]
    fn serde_round_trip_sequence_number_inline() {
        let original = SequenceNumber::new_unchecked(42);
        let json = serde_json::to_value(original).expect("serialize");
        let restored: SequenceNumber = serde_json::from_value(json).expect("deserialize");
        assert_eq!(restored, original);
    }

    #[test]
    fn serde_round_trip_event_version_inline() {
        let original = EventVersion::new_unchecked(1);
        let json = serde_json::to_value(original).expect("serialize");
        let restored: EventVersion = serde_json::from_value(json).expect("deserialize");
        assert_eq!(restored, original);
    }

    #[test]
    fn serde_round_trip_attempt_number_inline() {
        let original = AttemptNumber::new_unchecked(3);
        let json = serde_json::to_value(original).expect("serialize");
        let restored: AttemptNumber = serde_json::from_value(json).expect("deserialize");
        assert_eq!(restored, original);
    }

    #[test]
    fn serde_round_trip_timeout_ms_inline() {
        let original = TimeoutMs::new_unchecked(5000);
        let json = serde_json::to_value(original).expect("serialize");
        let restored: TimeoutMs = serde_json::from_value(json).expect("deserialize");
        assert_eq!(restored, original);
    }

    #[test]
    fn serde_round_trip_duration_ms_inline() {
        let original = DurationMs(5000);
        let json = serde_json::to_value(original).expect("serialize");
        let restored: DurationMs = serde_json::from_value(json).expect("deserialize");
        assert_eq!(restored, original);
    }

    #[test]
    fn serde_round_trip_timestamp_ms_inline() {
        let original = TimestampMs(1710000000000);
        let json = serde_json::to_value(original).expect("serialize");
        let restored: TimestampMs = serde_json::from_value(json).expect("deserialize");
        assert_eq!(restored, original);
    }

    #[test]
    fn serde_round_trip_fire_at_ms_inline() {
        let original = FireAtMs(1710000000000);
        let json = serde_json::to_value(original).expect("serialize");
        let restored: FireAtMs = serde_json::from_value(json).expect("deserialize");
        assert_eq!(restored, original);
    }

    #[test]
    fn serde_round_trip_max_attempts_inline() {
        let original = MaxAttempts::new_unchecked(3);
        let json = serde_json::to_value(original).expect("serialize");
        let restored: MaxAttempts = serde_json::from_value(json).expect("deserialize");
        assert_eq!(restored, original);
    }

    // ========== TryFrom<u64> ==========

    #[test]
    fn try_from_u64_sequence_number_valid() {
        let sn = SequenceNumber::try_from(42u64).expect("valid");
        assert_eq!(sn.as_u64(), 42);
    }

    #[test]
    fn try_from_u64_sequence_number_zero() {
        let result = SequenceNumber::try_from(0u64);
        assert_eq!(
            result,
            Err(ParseError::ZeroValue {
                type_name: "SequenceNumber"
            })
        );
    }

    #[test]
    fn try_from_u64_duration_ms_valid() {
        let dm = DurationMs::try_from(0u64).expect("valid");
        assert_eq!(dm.as_u64(), 0);
    }

    #[test]
    fn try_from_u64_duration_ms_nonzero() {
        let dm = DurationMs::try_from(1500u64).expect("valid");
        assert_eq!(dm.as_u64(), 1500);
    }

    // ========== From<T> for u64 ==========

    #[test]
    fn from_sequence_number_into_u64() {
        let sn = SequenceNumber::new_unchecked(42);
        let val: u64 = sn.into();
        assert_eq!(val, 42);
    }

    #[test]
    fn from_duration_ms_into_u64() {
        let dm = DurationMs(1500);
        let val: u64 = dm.into();
        assert_eq!(val, 1500);
    }

    // ========== Proptest round-trips ==========

    mod proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn sequence_number_round_trip_proptest(value in 1u64..) {
                let v = SequenceNumber(NonZeroU64::new(value).expect("nonzero"));
                prop_assert_eq!(SequenceNumber::parse(&v.to_string()), Ok(v));
            }

            #[test]
            fn event_version_round_trip_proptest(value in 1u64..) {
                let v = EventVersion(NonZeroU64::new(value).expect("nonzero"));
                prop_assert_eq!(EventVersion::parse(&v.to_string()), Ok(v));
            }

            #[test]
            fn attempt_number_round_trip_proptest(value in 1u64..) {
                let v = AttemptNumber(NonZeroU64::new(value).expect("nonzero"));
                prop_assert_eq!(AttemptNumber::parse(&v.to_string()), Ok(v));
            }

            #[test]
            fn timeout_ms_round_trip_proptest(value in 1u64..) {
                let v = TimeoutMs(NonZeroU64::new(value).expect("nonzero"));
                prop_assert_eq!(TimeoutMs::parse(&v.to_string()), Ok(v));
            }

            #[test]
            fn duration_ms_round_trip_proptest(value in 0u64..) {
                let v = DurationMs(value);
                prop_assert_eq!(DurationMs::parse(&v.to_string()), Ok(v));
            }

            #[test]
            fn timestamp_ms_round_trip_proptest(value in 0u64..) {
                let v = TimestampMs(value);
                prop_assert_eq!(TimestampMs::parse(&v.to_string()), Ok(v));
            }

            #[test]
            fn fire_at_ms_round_trip_proptest(value in 0u64..) {
                let v = FireAtMs(value);
                prop_assert_eq!(FireAtMs::parse(&v.to_string()), Ok(v));
            }

            #[test]
            fn max_attempts_round_trip_proptest(value in 1u64..) {
                let v = MaxAttempts(NonZeroU64::new(value).expect("nonzero"));
                prop_assert_eq!(MaxAttempts::parse(&v.to_string()), Ok(v));
            }

            #[test]
            fn timeout_ms_to_duration_proptest(value in 1u64..) {
                let v = TimeoutMs(NonZeroU64::new(value).expect("nonzero"));
                prop_assert_eq!(v.to_duration(), Duration::from_millis(value));
            }

            #[test]
            fn duration_ms_to_duration_proptest(value in 0u64..) {
                let v = DurationMs(value);
                prop_assert_eq!(v.to_duration(), Duration::from_millis(value));
            }

            #[test]
            fn timestamp_ms_to_system_time_proptest(value in 0u64..) {
                let v = TimestampMs(value);
                prop_assert_eq!(
                    v.to_system_time(),
                    SystemTime::UNIX_EPOCH + Duration::from_millis(value)
                );
            }

            #[test]
            fn fire_at_ms_has_elapsed_proptest(fire_at in 0u64.., now in 0u64..) {
                let f = FireAtMs(fire_at);
                let n = TimestampMs(now);
                prop_assert_eq!(f.has_elapsed(n), fire_at < now);
            }

            #[test]
            fn max_attempts_is_exhausted_proptest(max_val in 1u64.., attempt_val in 1u64..) {
                let m = MaxAttempts(NonZeroU64::new(max_val).expect("nonzero"));
                let a = AttemptNumber(NonZeroU64::new(attempt_val).expect("nonzero"));
                prop_assert_eq!(m.is_exhausted(a), attempt_val >= max_val);
            }

            #[test]
            fn serde_round_trip_sequence_number_proptest(value in 1u64..) {
                let v = SequenceNumber(NonZeroU64::new(value).expect("nonzero"));
                let json = serde_json::to_value(v).expect("serialize");
                let restored: SequenceNumber = serde_json::from_value(json).expect("deserialize");
                prop_assert_eq!(restored, v);
            }

            #[test]
            fn serde_round_trip_duration_ms_proptest(value in 0u64..) {
                let v = DurationMs(value);
                let json = serde_json::to_value(v).expect("serialize");
                let restored: DurationMs = serde_json::from_value(json).expect("deserialize");
                prop_assert_eq!(restored, v);
            }

            #[test]
            fn integer_display_is_decimal_no_padding(value in 0u64..) {
                let v = DurationMs(value);
                prop_assert_eq!(v.to_string(), value.to_string());
            }
        }
    }
}
