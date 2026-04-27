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
        Self(u64::try_from(millis).map_or(u64::MAX, |value| value))
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
}
