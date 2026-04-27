use super::*;
use std::num::NonZeroU64;
use std::time::{Duration, SystemTime};

#[cfg(feature = "proptest")]
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
        fn fencetoken_round_trip_proptest(value in 1u64..) {
            let v = FenceToken::new(value).unwrap();
            prop_assert_eq!(FenceToken::parse(&v.to_string()), Ok(v));
        }

        #[test]
        fn fencetoken_next_monotonicity_proptest(value in 1u64..u64::MAX) {
            let v = FenceToken::new(value).unwrap();
            prop_assert_eq!(v.next().map(|token| token.inner().get()), Ok(v.inner().get() + 1));
        }

        #[test]
        fn fencetoken_inner_representation_proptest(value in 1u64..) {
            let v = FenceToken::new(value).unwrap();
            prop_assert_eq!(v.inner().get(), value);
        }

        #[test]
        fn integer_display_is_decimal_no_padding(value in 0u64..) {
            let v = DurationMs(value);
            prop_assert_eq!(v.to_string(), value.to_string());
        }

        #[test]
        fn try_from_nonzero_u64_sequence_number_proptest(value in 1u64..) {
            let sn = SequenceNumber::try_from(value).expect("nonzero should succeed");
            prop_assert_eq!(sn.as_u64(), value);
        }

        #[test]
        fn try_from_zero_u64_sequence_number_fails() {
            let result = SequenceNumber::try_from(0u64);
            prop_assert!(result.is_err(), "zero should fail for SequenceNumber");
        }

        #[test]
        fn try_from_u64_duration_ms_always_succeeds_proptest(value in 0u64..) {
            let dm = DurationMs::try_from(value).expect("u64 should always succeed for DurationMs");
            prop_assert_eq!(dm.as_u64(), value);
        }

        #[test]
        fn try_from_nonzero_u64_event_version_proptest(value in 1u64..) {
            let ev = EventVersion::try_from(value).expect("nonzero should succeed");
            prop_assert_eq!(ev.as_u64(), value);
        }

        #[test]
        fn try_from_nonzero_u64_attempt_number_proptest(value in 1u64..) {
            let an = AttemptNumber::try_from(value).expect("nonzero should succeed");
            prop_assert_eq!(an.as_u64(), value);
        }

        #[test]
        fn try_from_nonzero_u64_timeout_ms_proptest(value in 1u64..) {
            let tm = TimeoutMs::try_from(value).expect("nonzero should succeed");
            prop_assert_eq!(tm.as_u64(), value);
        }

        #[test]
        fn try_from_nonzero_u64_max_attempts_proptest(value in 1u64..) {
            let ma = MaxAttempts::try_from(value).expect("nonzero should succeed");
            prop_assert_eq!(ma.as_u64(), value);
        }

        #[test]
        fn try_from_nonzero_u64_fence_token_proptest(value in 1u64..) {
            let ft = FenceToken::try_from(value).expect("nonzero should succeed");
            prop_assert_eq!(ft.inner().get(), value);
        }

        #[test]
        fn try_from_zero_u64_fence_token_fails() {
            let result = FenceToken::try_from(0u64);
            prop_assert!(result.is_err(), "zero should fail for FenceToken");
        }

        #[test]
        fn from_sequence_number_into_u64_proptest(value in 1u64..) {
            let sn = SequenceNumber::try_from(value).expect("valid");
            let back: u64 = sn.into();
            prop_assert_eq!(back, value);
        }

        #[test]
        fn from_duration_ms_into_u64_proptest(value in 0u64..) {
            let dm = DurationMs(value);
            let back: u64 = dm.into();
            prop_assert_eq!(back, value);
        }

        #[test]
        fn from_event_version_into_u64_proptest(value in 1u64..) {
            let ev = EventVersion::try_from(value).expect("valid");
            let back: u64 = ev.into();
            prop_assert_eq!(back, value);
        }

        #[test]
        fn from_attempt_number_into_u64_proptest(value in 1u64..) {
            let an = AttemptNumber::try_from(value).expect("valid");
            let back: u64 = an.into();
            prop_assert_eq!(back, value);
        }

        #[test]
        fn from_timeout_ms_into_u64_proptest(value in 1u64..) {
            let tm = TimeoutMs::try_from(value).expect("valid");
            let back: u64 = tm.into();
            prop_assert_eq!(back, value);
        }

        #[test]
        fn from_max_attempts_into_u64_proptest(value in 1u64..) {
            let ma = MaxAttempts::try_from(value).expect("valid");
            let back: u64 = ma.into();
            prop_assert_eq!(back, value);
        }

        #[test]
        fn from_sequence_number_into_nonzero_u64_proptest(value in 1u64..) {
            let sn = SequenceNumber::try_from(value).expect("valid");
            let back: NonZeroU64 = sn.into();
            prop_assert_eq!(back.get(), value);
        }

        #[test]
        fn try_from_into_roundtrip_sequence_number_proptest(value in 1u64..) {
            let sn = SequenceNumber::try_from(value).expect("valid");
            let back: u64 = sn.into();
            let sn2 = SequenceNumber::try_from(back).expect("roundtrip");
            prop_assert_eq!(sn, sn2);
        }

        #[test]
        fn try_from_into_roundtrip_duration_ms_proptest(value in 0u64..) {
            let dm = DurationMs::try_from(value).expect("valid");
            let back: u64 = dm.into();
            let dm2 = DurationMs::try_from(back).expect("roundtrip");
            prop_assert_eq!(dm, dm2);
        }
    }
}