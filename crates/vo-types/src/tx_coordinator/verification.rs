//! Kani verification harnesses for transaction coordinator types.

#[cfg(kani)]
mod verification {
    use crate::tx_coordinator::apply_coordinator_transition;
    use crate::tx_coordinator::{
        CoordinatorTransition, ParticipantRecord, ParticipantStatus, TransactionRecord,
        TransactionState,
    };

    /// K-01: Verify apply_coordinator_transition exhaustiveness.
    /// All 10×12 = 120 combinations must be covered without panic.
    #[kani::proof]
    fn verify_coordinator_transition_exhaustiveness() {
        let state: u8 = kani::any();
        let event: u8 = kani::any();
        kani::assume(state < 10);
        kani::assume(event < 12);

        let current = match state {
            0 => TransactionState::Init,
            1 => TransactionState::Enrolling,
            2 => TransactionState::Preparing,
            3 => TransactionState::Prepared,
            4 => TransactionState::Committing,
            5 => TransactionState::Committed,
            6 => TransactionState::RollingBack,
            7 => TransactionState::RolledBack,
            8 => TransactionState::Aborted,
            _ => TransactionState::Ambiguous,
        };

        let evt = match event {
            0 => CoordinatorTransition::BeginEnroll,
            1 => CoordinatorTransition::BeginPrepare,
            2 => CoordinatorTransition::ParticipantPrepared,
            3 => CoordinatorTransition::ParticipantRollback,
            4 => CoordinatorTransition::AllResponded,
            5 => CoordinatorTransition::DecideCommit,
            6 => CoordinatorTransition::DecideRollback,
            7 => CoordinatorTransition::Timeout,
            8 => CoordinatorTransition::Recover,
            9 => CoordinatorTransition::ReconcileCommitted,
            10 => CoordinatorTransition::ReconcileRolledBack,
            _ => CoordinatorTransition::ReconcileRetry,
        };

        // Must not panic — all combinations handled
        let _ = apply_coordinator_transition(current, evt);
    }

    /// K-02: Verify TransactionRecord::new rejects empty transaction_id.
    #[kani::proof]
    fn verify_transaction_record_rejects_empty_id() {
        let result = TransactionRecord::new(
            String::new(),
            TransactionState::Init,
            None,
            vec![],
            None,
            None,
            None,
        );
        assert!(result.is_none());
    }

    /// K-03: Verify ParticipantRecord::new rejects empty participant_id.
    #[kani::proof]
    fn verify_participant_record_rejects_empty_id() {
        let result = ParticipantRecord::new(String::new(), ParticipantStatus::Enrolled, None);
        assert!(result.is_none());
    }
}
