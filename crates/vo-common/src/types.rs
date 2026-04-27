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

    #[test]
    fn timestamp_ms_new_unchecked_zero() {
        let ts = TimestampMs::new_unchecked(0);
        assert_eq!(ts.as_u64(), 0);
    }

    #[test]
    fn timestamp_ms_new_unchecked_max() {
        let ts = TimestampMs::new_unchecked(u64::MAX);
        assert_eq!(ts.as_u64(), u64::MAX);
    }

    #[test]
    fn timestamp_ms_as_u64_roundtrip() {
        let val = 1234567890u64;
        let ts = TimestampMs::new_unchecked(val);
        assert_eq!(ts.as_u64(), val);
    }

    #[test]
    fn timestamp_ms_now_produces_reasonable_value() {
        let ts = TimestampMs::now();
        let val = ts.as_u64();
        assert!(val > 0, "now() should produce a positive timestamp");
        assert!(val <= u64::MAX, "now() should not overflow u64");
    }

    #[test]
    fn timestamp_ms_now_increases_over_time() {
        let ts1 = TimestampMs::now();
        std::thread::sleep(std::time::Duration::from_millis(1));
        let ts2 = TimestampMs::now();
        assert!(ts2 > ts1, "subsequent now() calls should produce larger values");
    }

    #[test]
    fn timestamp_ms_ord_implementation() {
        let ts1 = TimestampMs::new_unchecked(100);
        let ts2 = TimestampMs::new_unchecked(200);
        let ts3 = TimestampMs::new_unchecked(100);
        assert!(ts1 < ts2);
        assert!(ts2 > ts1);
        assert_eq!(ts1, ts3);
        assert!(ts1 <= ts2);
        assert!(ts1 <= ts3);
        assert!(ts2 >= ts1);
        assert!(ts1 >= ts3);
    }

    #[test]
    fn timestamp_ms_debug_format() {
        let ts = TimestampMs::new_unchecked(42);
        let debug = format!("{:?}", ts);
        assert!(debug.contains("42"), "Debug format should contain the value");
    }
}
