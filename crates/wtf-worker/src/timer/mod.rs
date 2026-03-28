pub mod record;
pub mod loop_impl;

pub use record::TimerRecord;
pub use loop_impl::*;

#[cfg(test)]
mod tests {
    use super::*;
    use super::record::TimerRecord;
    use chrono::{DateTime, Utc, Duration as ChronoDuration};
    use std::time::Duration;
    use wtf_common::{InstanceId, NamespaceId, TimerId};

    fn make_record(timer_id: &str, fire_at: DateTime<Utc>) -> TimerRecord {
        TimerRecord {
            timer_id: TimerId::new(timer_id),
            namespace: NamespaceId::new("payments"),
            instance_id: InstanceId::new("inst-001"),
            fire_at,
        }
    }

    #[test]
    fn timer_record_is_due_when_fire_at_is_in_the_past() {
        let past = Utc::now() - ChronoDuration::seconds(5);
        let record = make_record("timer-001", past);
        assert!(record.is_due(Utc::now()));
    }

    #[test]
    fn timer_record_is_due_when_fire_at_equals_now() {
        let now = Utc::now();
        let record = make_record("timer-002", now);
        assert!(record.is_due(now));
    }

    #[test]
    fn timer_record_is_not_due_when_fire_at_is_in_the_future() {
        let future = Utc::now() + ChronoDuration::hours(1);
        let record = make_record("timer-003", future);
        assert!(!record.is_due(Utc::now()));
    }

    #[test]
    fn timer_record_msgpack_roundtrip() -> anyhow::Result<()> {
        let fire_at = Utc::now();
        let record = make_record("timer-004", fire_at);
        let bytes = record.to_msgpack().map_err(|e| anyhow::anyhow!(e))?;
        assert!(!bytes.is_empty());
        let decoded = TimerRecord::from_msgpack(&bytes).map_err(|e| anyhow::anyhow!(e))?;
        assert_eq!(decoded.timer_id.as_str(), "timer-004");
        assert_eq!(decoded.namespace.as_str(), "payments");
        assert_eq!(decoded.instance_id.as_str(), "inst-001");
        Ok(())
    }

    #[test]
    fn timer_record_from_msgpack_invalid_bytes_returns_error() {
        let result = TimerRecord::from_msgpack(b"not valid msgpack!!!");
        assert!(matches!(result, Err(_)));
    }

    #[test]
    fn timer_poll_interval_is_one_second() {
        assert_eq!(TIMER_POLL_INTERVAL, Duration::from_secs(1));
    }

    #[test]
    fn timer_record_far_future_not_due() {
        let far_future = Utc::now() + ChronoDuration::days(365);
        let record = make_record("timer-future", far_future);
        assert!(!record.is_due(Utc::now()));
    }
}
