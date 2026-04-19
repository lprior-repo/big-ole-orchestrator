#[derive(Debug, Clone)]
pub struct RecoveryResult<S = ()> {
    pub state: S,
    pub events_applied: u64,
    pub starting_sequence: u64,
    pub ending_sequence: u64,
    pub recovery_type: RecoveryType,
    pub duration_ms: u64,
    pub snapshot_used: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryType {
    FullReplay,
    SnapshotAccelerated,
    Incremental,
}

impl RecoveryResult {
    #[must_use]
    pub fn unit(events_applied: u64, starting_sequence: u64, ending_sequence: u64) -> Self {
        Self {
            state: (),
            events_applied,
            starting_sequence,
            ending_sequence,
            recovery_type: RecoveryType::FullReplay,
            duration_ms: 0,
            snapshot_used: false,
        }
    }
}

impl<S> RecoveryResult<S> {
    #[must_use]
    pub fn new(
        state: S,
        events_applied: u64,
        starting_sequence: u64,
        ending_sequence: u64,
        recovery_type: RecoveryType,
        duration_ms: u64,
        snapshot_used: bool,
    ) -> Self {
        Self {
            state,
            events_applied,
            starting_sequence,
            ending_sequence,
            recovery_type,
            duration_ms,
            snapshot_used,
        }
    }
}
