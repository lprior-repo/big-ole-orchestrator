//! Type definitions for vo-common.

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

pub type InstanceId = String;
pub type NamespaceId = String;
pub type TimerId = String;
pub type EventId = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TimestampMs(pub u64);

impl TimestampMs {
    #[must_use]
    pub const fn new_unchecked(value: u64) -> Self {
        Self(value)
    }
    #[must_use]
    pub fn as_u64(self) -> u64 {
        self.0
    }
    #[must_use]
    pub fn now() -> Self {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis());
        Self(if let Ok(v) = u64::try_from(millis) {
            v
        } else {
            u64::MAX
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_id_behaves_as_string() {
        let id: InstanceId = "test-instance-123".into();
        assert_eq!(id.len(), 17);
        assert_eq!(id.as_str(), "test-instance-123");
    }

    #[test]
    fn namespace_id_behaves_as_string() {
        let ns: NamespaceId = "namespace-abc".into();
        assert_eq!(ns.len(), 13);
        assert_eq!(ns.as_str(), "namespace-abc");
    }

    #[test]
    fn timer_id_behaves_as_string() {
        let timer: TimerId = "timer-xyz".into();
        assert_eq!(timer.len(), 9);
        assert_eq!(timer.as_str(), "timer-xyz");
    }

    #[test]
    fn event_id_behaves_as_string() {
        let eid: EventId = "evt-abc-123".into();
        assert_eq!(eid.len(), 11);
        assert_eq!(eid.as_str(), "evt-abc-123");
    }

    #[test]
    fn instance_id_empty_string() {
        let id: InstanceId = "".into();
        assert_eq!(id.len(), 0);
    }

    #[test]
    fn instance_id_unicode() {
        let id: InstanceId = "实例-123-🔱".into();
        assert_eq!(id.len(), 15); // UTF-8 bytes: 6 + 1 + 3 + 1 + 4
        assert_eq!(id.as_str(), "实例-123-🔱");
    }

    // ========================================================================
    // TimestampMs Tests
    // ========================================================================

    mod timestamp_ms {
        use super::*;

        #[test]
        fn timestamp_ms_new_unchecked() {
            let ts = TimestampMs::new_unchecked(12345);
            assert_eq!(ts.as_u64(), 12345);
        }

        #[test]
        fn timestamp_ms_zero() {
            let ts = TimestampMs::new_unchecked(0);
            assert_eq!(ts.as_u64(), 0);
        }

        #[test]
        fn timestamp_ms_max() {
            let ts = TimestampMs::new_unchecked(u64::MAX);
            assert_eq!(ts.as_u64(), u64::MAX);
        }

        #[test]
        fn timestamp_ms_now_returns_reasonable_value() {
            let before = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            let ts = TimestampMs::now();
            let after = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            let ts_val = u64::try_from(ts.as_u64()).unwrap();
            assert!(u64::try_from(before).unwrap() <= ts_val);
            assert!(ts_val <= u64::try_from(after).unwrap());
        }

        #[test]
        fn timestamp_ms_now_is_unique_or_increasing() {
            let ts1 = TimestampMs::now();
            let ts2 = TimestampMs::now();
            assert!(ts1.as_u64() <= ts2.as_u64());
        }

        #[test]
        fn timestamp_ms_clone_preserves_value() {
            let ts = TimestampMs::new_unchecked(99999);
            let cloned = ts.clone();
            assert_eq!(ts.as_u64(), cloned.as_u64());
        }

        #[test]
        fn timestamp_ms_debug_format() {
            let ts = TimestampMs::new_unchecked(42);
            let debug = format!("{:?}", ts);
            assert!(debug.contains("42"));
        }

        #[test]
        fn timestamp_ms_serde_roundtrip() {
            let ts = TimestampMs::new_unchecked(123456789);
            let json = serde_json::to_string(&ts).unwrap();
            let deserialized: TimestampMs = serde_json::from_str(&json).unwrap();
            assert_eq!(ts.as_u64(), deserialized.as_u64());
        }

        #[test]
        fn timestamp_ms_ordering() {
            let ts1 = TimestampMs::new_unchecked(100);
            let ts2 = TimestampMs::new_unchecked(200);
            let ts3 = TimestampMs::new_unchecked(100);

            assert!(ts1 < ts2);
            assert!(ts2 > ts1);
            assert_eq!(ts1, ts3);
            assert!(ts1 <= ts3);
            assert!(ts1 >= ts3);
        }

        #[test]
        fn timestamp_ms_boundary_values() {
            let min = TimestampMs::new_unchecked(u64::MIN);
            let max = TimestampMs::new_unchecked(u64::MAX);

            assert_eq!(min.as_u64(), u64::MIN);
            assert_eq!(max.as_u64(), u64::MAX);
            assert!(min < max);
        }

        #[test]
        fn timestamp_ms_ordering_total() {
            let ts1 = TimestampMs::new_unchecked(100);
            let ts2 = TimestampMs::new_unchecked(200);
            let ts3 = TimestampMs::new_unchecked(300);

            assert!(ts1 < ts2);
            assert!(ts2 < ts3);
            assert!(ts1 < ts3);

            assert!(ts1 <= ts1);
            assert!(ts1 >= ts1);
            assert!(ts2 <= ts2);
            assert!(ts2 >= ts2);
        }

        #[test]
        fn timestamp_ms_as_u64_exact_value() {
            let ts = TimestampMs::new_unchecked(12345678901234);
            assert_eq!(ts.as_u64(), 12345678901234);
        }

        #[test]
        fn timestamp_ms_neq_different_values() {
            let ts1 = TimestampMs::new_unchecked(100);
            let ts2 = TimestampMs::new_unchecked(200);
            assert_ne!(ts1, ts2);
        }
    }
}
