//! Coarse-grained instance status for the instances index partition.
//!
//! Architecture: Data (`InstanceStatus`) → Calc (`to_byte`, `from_byte`, `all_variants`).
//!
//! This is an index-level projection of `LifecycleState`, NOT a state machine.
//! It exists solely for key encoding in the instances partition.

/// Coarse-grained instance status for the instances index partition.
///
/// Invariant: Each variant maps to exactly one non-zero byte in `[0x01..=0x06]`.
/// Invariant: The byte mapping is stable and append-only (never reorder or reassign).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum InstanceStatus {
    Pending = 0x01,
    Running = 0x02,
    Paused = 0x03,
    Completed = 0x04,
    Failed = 0x05,
    Cancelled = 0x06,
}

impl InstanceStatus {
    /// Returns the `repr(u8)` byte value for this status.
    ///
    /// Total function — always succeeds.
    /// Result is in `[0x01..=0x06]`.
    #[must_use]
    pub const fn to_byte(self) -> u8 {
        self as u8
    }

    /// Converts a byte to an `InstanceStatus`.
    ///
    /// Returns `None` if `byte` is not in `[0x01..=0x06]`.
    ///
    /// Note: In `vo-storage`, callers map `None` to `StorageError::CorruptKey`.
    #[must_use]
    pub const fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0x01 => Some(Self::Pending),
            0x02 => Some(Self::Running),
            0x03 => Some(Self::Paused),
            0x04 => Some(Self::Completed),
            0x05 => Some(Self::Failed),
            0x06 => Some(Self::Cancelled),
            _ => None,
        }
    }

    /// Returns all 6 variants in byte-ascending order.
    #[must_use]
    pub const fn all_variants() -> &'static [Self; 6] {
        &[
            Self::Pending,
            Self::Running,
            Self::Paused,
            Self::Completed,
            Self::Failed,
            Self::Cancelled,
        ]
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use rstest::rstest;

    // ---- B01: to_byte returns repr(u8) discriminant ----

    #[test]
    fn instance_status_to_byte_returns_0x01_when_pending() {
        assert_eq!(InstanceStatus::Pending.to_byte(), 0x01);
    }

    #[test]
    fn instance_status_to_byte_returns_0x02_when_running() {
        assert_eq!(InstanceStatus::Running.to_byte(), 0x02);
    }

    #[test]
    fn instance_status_to_byte_returns_0x03_when_paused() {
        assert_eq!(InstanceStatus::Paused.to_byte(), 0x03);
    }

    #[test]
    fn instance_status_to_byte_returns_0x04_when_completed() {
        assert_eq!(InstanceStatus::Completed.to_byte(), 0x04);
    }

    #[test]
    fn instance_status_to_byte_returns_0x05_when_failed() {
        assert_eq!(InstanceStatus::Failed.to_byte(), 0x05);
    }

    #[test]
    fn instance_status_to_byte_returns_0x06_when_cancelled() {
        assert_eq!(InstanceStatus::Cancelled.to_byte(), 0x06);
    }

    // ---- B02: from_byte returns correct variant ----

    #[test]
    fn instance_status_from_byte_returns_pending_when_byte_is_0x01() {
        assert_eq!(
            InstanceStatus::from_byte(0x01),
            Some(InstanceStatus::Pending)
        );
    }

    #[test]
    fn instance_status_from_byte_returns_running_when_byte_is_0x02() {
        assert_eq!(
            InstanceStatus::from_byte(0x02),
            Some(InstanceStatus::Running)
        );
    }

    #[test]
    fn instance_status_from_byte_returns_paused_when_byte_is_0x03() {
        assert_eq!(
            InstanceStatus::from_byte(0x03),
            Some(InstanceStatus::Paused)
        );
    }

    #[test]
    fn instance_status_from_byte_returns_completed_when_byte_is_0x04() {
        assert_eq!(
            InstanceStatus::from_byte(0x04),
            Some(InstanceStatus::Completed)
        );
    }

    #[test]
    fn instance_status_from_byte_returns_failed_when_byte_is_0x05() {
        assert_eq!(
            InstanceStatus::from_byte(0x05),
            Some(InstanceStatus::Failed)
        );
    }

    #[test]
    fn instance_status_from_byte_returns_cancelled_when_byte_is_0x06() {
        assert_eq!(
            InstanceStatus::from_byte(0x06),
            Some(InstanceStatus::Cancelled)
        );
    }

    // ---- B03: from_byte rejects zero byte ----

    #[test]
    fn instance_status_from_byte_returns_none_when_byte_is_zero() {
        assert_eq!(InstanceStatus::from_byte(0x00), None);
    }

    // ---- B04: from_byte rejects bytes above valid range ----

    #[test]
    fn instance_status_from_byte_returns_none_when_byte_is_0x07() {
        assert_eq!(InstanceStatus::from_byte(0x07), None);
    }

    #[test]
    fn instance_status_from_byte_returns_none_when_byte_is_0xff() {
        assert_eq!(InstanceStatus::from_byte(0xFF), None);
    }

    // ---- B05: to_byte / from_byte round-trip (rstest parameterized) ----

    #[rstest]
    #[case(InstanceStatus::Pending, 0x01)]
    #[case(InstanceStatus::Running, 0x02)]
    #[case(InstanceStatus::Paused, 0x03)]
    #[case(InstanceStatus::Completed, 0x04)]
    #[case(InstanceStatus::Failed, 0x05)]
    #[case(InstanceStatus::Cancelled, 0x06)]
    fn instance_status_round_trips_through_byte(#[case] variant: InstanceStatus, #[case] byte: u8) {
        assert_eq!(variant.to_byte(), byte);
        assert_eq!(InstanceStatus::from_byte(byte), Some(variant));
    }

    // ---- B06: all_variants returns 6 variants in byte order ----

    #[test]
    fn instance_status_all_variants_returns_six_variants_in_byte_order() {
        let variants = InstanceStatus::all_variants();
        assert_eq!(variants.len(), 6);
        assert_eq!(
            variants,
            &[
                InstanceStatus::Pending,
                InstanceStatus::Running,
                InstanceStatus::Paused,
                InstanceStatus::Completed,
                InstanceStatus::Failed,
                InstanceStatus::Cancelled,
            ]
        );
        // Additionally verify byte ordering: all_variants()[i].to_byte() == (i + 1) as u8
        assert_eq!(variants[0].to_byte(), 1);
        assert_eq!(variants[1].to_byte(), 2);
        assert_eq!(variants[2].to_byte(), 3);
        assert_eq!(variants[3].to_byte(), 4);
        assert_eq!(variants[4].to_byte(), 5);
        assert_eq!(variants[5].to_byte(), 6);
    }
}
