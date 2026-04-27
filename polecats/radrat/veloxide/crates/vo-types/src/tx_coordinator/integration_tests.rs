//! Integration tests for distributed transaction coordination across multiple nodes.
//!
//! These tests simulate realistic multi-participant 2PC scenarios using the
//! transaction coordinator types. Tests cover:
//! - Multi-participant enrollment and vote collection
//! - Coordinator decision logic based on participant votes
//! - Timeout handling with partial participant responses
//! - Recovery scenarios with ambiguous outcomes

use crate::tx_coordinator::{
    apply_coordinator_transition, CoordinatorDecision, CoordinatorTransition, ParticipantRecord,
    ParticipantStatus, ParticipantVote, TransactionState,
};

fn make_participant(
    id: &str,
    status: ParticipantStatus,
    vote: Option<ParticipantVote>,
) -> ParticipantRecord {
    ParticipantRecord::new(id.to_string(), status, vote).unwrap()
}

fn make_transaction_record(
    tx_id: &str,
    state: TransactionState,
    decision: Option<CoordinatorDecision>,
    participants: Vec<ParticipantRecord>,
) -> Option<crate::tx_coordinator::TransactionRecord> {
    crate::tx_coordinator::TransactionRecord::new(
        tx_id.to_string(),
        state,
        decision,
        participants,
        None,
        None,
        None,
    )
}

#[test]
fn two_phase_commit_with_three_participants() {
    let tx_id = "tx-3p";
    let participants = vec![
        make_participant("A", ParticipantStatus::Enrolled, None),
        make_participant("B", ParticipantStatus::Enrolled, None),
        make_participant("C", ParticipantStatus::Enrolled, None),
    ];
    let mut record =
        make_transaction_record(tx_id, TransactionState::Init, None, participants).unwrap();

    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::BeginEnroll).unwrap(),
        record.decision(),
        record.participants().to_vec(),
    )
    .unwrap();
    assert_eq!(record.state(), TransactionState::Enrolling);

    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::BeginPrepare).unwrap(),
        record.decision(),
        record.participants().to_vec(),
    )
    .unwrap();
    assert_eq!(record.state(), TransactionState::Preparing);

    let mut participants = record.participants().to_vec();
    for p in &mut participants {
        if p.participant_id() == "A" {
            *p = make_participant(
                "A",
                ParticipantStatus::Prepared,
                Some(ParticipantVote::Prepared),
            );
        }
    }
    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::ParticipantPrepared)
            .unwrap(),
        record.decision(),
        participants,
    )
    .unwrap();
    assert_eq!(record.state(), TransactionState::Preparing);

    let mut participants = record.participants().to_vec();
    for p in &mut participants {
        if p.participant_id() == "B" {
            *p = make_participant(
                "B",
                ParticipantStatus::Prepared,
                Some(ParticipantVote::Prepared),
            );
        }
    }
    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::ParticipantPrepared)
            .unwrap(),
        record.decision(),
        participants,
    )
    .unwrap();

    let mut participants = record.participants().to_vec();
    for p in &mut participants {
        if p.participant_id() == "C" {
            *p = make_participant(
                "C",
                ParticipantStatus::Prepared,
                Some(ParticipantVote::Prepared),
            );
        }
    }
    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::ParticipantPrepared)
            .unwrap(),
        record.decision(),
        participants,
    )
    .unwrap();

    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::AllResponded).unwrap(),
        record.decision(),
        record.participants().to_vec(),
    )
    .unwrap();
    assert_eq!(record.state(), TransactionState::Prepared);

    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::DecideCommit).unwrap(),
        Some(CoordinatorDecision::Commit),
        record.participants().to_vec(),
    )
    .unwrap();
    assert_eq!(record.state(), TransactionState::Committing);

    let mut participants = record.participants().to_vec();
    for p in &mut participants {
        *p = make_participant(p.participant_id(), ParticipantStatus::Committed, p.vote());
    }
    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::AllResponded).unwrap(),
        record.decision(),
        participants,
    )
    .unwrap();
    assert_eq!(record.state(), TransactionState::Committed);

    for p in record.participants() {
        assert_eq!(p.status(), ParticipantStatus::Committed);
    }
}

#[test]
fn two_phase_rollback_with_one_dissenting() {
    let tx_id = "tx-rollback-1";
    let participants = vec![
        make_participant("A", ParticipantStatus::Enrolled, None),
        make_participant("B", ParticipantStatus::Enrolled, None),
        make_participant("C", ParticipantStatus::Enrolled, None),
    ];
    let mut record =
        make_transaction_record(tx_id, TransactionState::Init, None, participants).unwrap();

    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::BeginEnroll).unwrap(),
        None,
        record.participants().to_vec(),
    )
    .unwrap();
    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::BeginPrepare).unwrap(),
        None,
        record.participants().to_vec(),
    )
    .unwrap();

    let mut participants = record.participants().to_vec();
    for p in &mut participants {
        if p.participant_id() == "A" {
            *p = make_participant(
                "A",
                ParticipantStatus::Prepared,
                Some(ParticipantVote::Prepared),
            );
        }
    }
    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::ParticipantPrepared)
            .unwrap(),
        None,
        participants,
    )
    .unwrap();

    let mut participants = record.participants().to_vec();
    for p in &mut participants {
        if p.participant_id() == "B" {
            *p = make_participant(
                "B",
                ParticipantStatus::Prepared,
                Some(ParticipantVote::Prepared),
            );
        }
    }
    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::ParticipantPrepared)
            .unwrap(),
        None,
        participants,
    )
    .unwrap();

    let mut participants = record.participants().to_vec();
    for p in &mut participants {
        if p.participant_id() == "C" {
            *p = make_participant(
                "C",
                ParticipantStatus::VotedRollback,
                Some(ParticipantVote::Rollback),
            );
        }
    }
    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::ParticipantRollback)
            .unwrap(),
        None,
        participants,
    )
    .unwrap();

    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::AllResponded).unwrap(),
        None,
        record.participants().to_vec(),
    )
    .unwrap();
    assert_eq!(record.state(), TransactionState::Prepared);

    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::DecideRollback)
            .unwrap(),
        Some(CoordinatorDecision::Rollback),
        record.participants().to_vec(),
    )
    .unwrap();
    assert_eq!(record.state(), TransactionState::RollingBack);

    let mut participants = record.participants().to_vec();
    for p in &mut participants {
        *p = make_participant(p.participant_id(), ParticipantStatus::RolledBack, p.vote());
    }
    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::AllResponded).unwrap(),
        record.decision(),
        participants,
    )
    .unwrap();
    assert_eq!(record.state(), TransactionState::RolledBack);

    for p in record.participants() {
        assert_eq!(p.status(), ParticipantStatus::RolledBack);
    }
}

#[test]
fn two_phase_commit_with_five_participants() {
    let tx_id = "tx-5p";
    let participants: Vec<ParticipantRecord> = (1..=5)
        .map(|i| make_participant(&format!("p-{}", i), ParticipantStatus::Enrolled, None))
        .collect();
    let mut record =
        make_transaction_record(tx_id, TransactionState::Init, None, participants).unwrap();

    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::BeginEnroll).unwrap(),
        None,
        record.participants().to_vec(),
    )
    .unwrap();
    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::BeginPrepare).unwrap(),
        None,
        record.participants().to_vec(),
    )
    .unwrap();

    for i in 1..=5 {
        let pid = format!("p-{}", i);
        let mut participants = record.participants().to_vec();
        for p in &mut participants {
            if p.participant_id() == pid {
                *p = make_participant(
                    &pid,
                    ParticipantStatus::Prepared,
                    Some(ParticipantVote::Prepared),
                );
            }
        }
        record = make_transaction_record(
            tx_id,
            apply_coordinator_transition(
                record.state(),
                CoordinatorTransition::ParticipantPrepared,
            )
            .unwrap(),
            None,
            participants,
        )
        .unwrap();
    }

    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::AllResponded).unwrap(),
        None,
        record.participants().to_vec(),
    )
    .unwrap();
    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::DecideCommit).unwrap(),
        Some(CoordinatorDecision::Commit),
        record.participants().to_vec(),
    )
    .unwrap();
    let mut participants = record.participants().to_vec();
    for p in &mut participants {
        *p = make_participant(p.participant_id(), ParticipantStatus::Committed, p.vote());
    }
    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::AllResponded).unwrap(),
        record.decision(),
        participants,
    )
    .unwrap();

    assert_eq!(record.state(), TransactionState::Committed);
}

#[test]
fn mixed_votes_result_in_rollback() {
    let tx_id = "tx-mixed";
    let participants = vec![
        make_participant("p1", ParticipantStatus::Enrolled, None),
        make_participant("p2", ParticipantStatus::Enrolled, None),
        make_participant("p3", ParticipantStatus::Enrolled, None),
        make_participant("p4", ParticipantStatus::Enrolled, None),
    ];
    let mut record =
        make_transaction_record(tx_id, TransactionState::Init, None, participants).unwrap();

    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::BeginEnroll).unwrap(),
        None,
        record.participants().to_vec(),
    )
    .unwrap();
    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::BeginPrepare).unwrap(),
        None,
        record.participants().to_vec(),
    )
    .unwrap();

    let votes = [("p1", true), ("p2", false), ("p3", true), ("p4", false)];
    for (pid, prepared) in &votes {
        let mut participants = record.participants().to_vec();
        for p in &mut participants {
            if p.participant_id() == *pid {
                if *prepared {
                    *p = make_participant(
                        pid,
                        ParticipantStatus::Prepared,
                        Some(ParticipantVote::Prepared),
                    );
                } else {
                    *p = make_participant(
                        pid,
                        ParticipantStatus::VotedRollback,
                        Some(ParticipantVote::Rollback),
                    );
                }
            }
        }
        let evt = if *prepared {
            CoordinatorTransition::ParticipantPrepared
        } else {
            CoordinatorTransition::ParticipantRollback
        };
        record = make_transaction_record(
            tx_id,
            apply_coordinator_transition(record.state(), evt).unwrap(),
            None,
            participants,
        )
        .unwrap();
    }

    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::AllResponded).unwrap(),
        None,
        record.participants().to_vec(),
    )
    .unwrap();
    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::DecideRollback)
            .unwrap(),
        Some(CoordinatorDecision::Rollback),
        record.participants().to_vec(),
    )
    .unwrap();
    let mut participants = record.participants().to_vec();
    for p in &mut participants {
        *p = make_participant(p.participant_id(), ParticipantStatus::RolledBack, p.vote());
    }
    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::AllResponded).unwrap(),
        record.decision(),
        participants,
    )
    .unwrap();

    assert_eq!(record.state(), TransactionState::RolledBack);
}

#[test]
fn prepare_timeout_aborts_transaction() {
    let tx_id = "tx-timeout";
    let participants = vec![
        make_participant("p1", ParticipantStatus::Enrolled, None),
        make_participant("p2", ParticipantStatus::Enrolled, None),
    ];
    let record =
        make_transaction_record(tx_id, TransactionState::Init, None, participants).unwrap();

    let record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::BeginEnroll).unwrap(),
        None,
        record.participants().to_vec(),
    )
    .unwrap();
    let record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::BeginPrepare).unwrap(),
        None,
        record.participants().to_vec(),
    )
    .unwrap();

    let result = apply_coordinator_transition(record.state(), CoordinatorTransition::Timeout);
    assert_eq!(result, Ok(TransactionState::Aborted));
}

#[test]
fn commit_timeout_goes_ambiguous() {
    let tx_id = "tx-commit-timeout";
    let participants = vec![make_participant(
        "p1",
        ParticipantStatus::Prepared,
        Some(ParticipantVote::Prepared),
    )];
    let record = make_transaction_record(
        tx_id,
        TransactionState::Prepared,
        Some(CoordinatorDecision::Commit),
        participants,
    )
    .unwrap();

    let record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::DecideCommit).unwrap(),
        record.decision(),
        record.participants().to_vec(),
    )
    .unwrap();

    let result = apply_coordinator_transition(record.state(), CoordinatorTransition::Timeout);
    assert_eq!(result, Ok(TransactionState::Ambiguous));
}

#[test]
fn rollback_timeout_goes_ambiguous() {
    let tx_id = "tx-rollback-timeout";
    let participants = vec![make_participant(
        "p1",
        ParticipantStatus::VotedRollback,
        Some(ParticipantVote::Rollback),
    )];
    let record = make_transaction_record(
        tx_id,
        TransactionState::Prepared,
        Some(CoordinatorDecision::Rollback),
        participants,
    )
    .unwrap();

    let record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::DecideRollback)
            .unwrap(),
        record.decision(),
        record.participants().to_vec(),
    )
    .unwrap();

    let result = apply_coordinator_transition(record.state(), CoordinatorTransition::Timeout);
    assert_eq!(result, Ok(TransactionState::Ambiguous));
}

#[test]
fn ambiguous_can_be_recovered_to_committed() {
    let result = apply_coordinator_transition(
        TransactionState::Ambiguous,
        CoordinatorTransition::ReconcileCommitted,
    );
    assert_eq!(result, Ok(TransactionState::Committed));
}

#[test]
fn ambiguous_can_be_recovered_to_rolled_back() {
    let result = apply_coordinator_transition(
        TransactionState::Ambiguous,
        CoordinatorTransition::ReconcileRolledBack,
    );
    assert_eq!(result, Ok(TransactionState::RolledBack));
}

#[test]
fn ambiguous_retry_stays_ambiguous() {
    let result = apply_coordinator_transition(
        TransactionState::Ambiguous,
        CoordinatorTransition::ReconcileRetry,
    );
    assert_eq!(result, Ok(TransactionState::Ambiguous));
}

#[test]
fn all_participants_must_vote_prepared_for_commit() {
    let tx_id = "tx-all-must-prepare";
    let participants = vec![
        make_participant("p1", ParticipantStatus::Enrolled, None),
        make_participant("p2", ParticipantStatus::Enrolled, None),
        make_participant("p3", ParticipantStatus::Enrolled, None),
        make_participant("p4", ParticipantStatus::Enrolled, None),
    ];
    let mut record =
        make_transaction_record(tx_id, TransactionState::Init, None, participants).unwrap();

    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::BeginEnroll).unwrap(),
        None,
        record.participants().to_vec(),
    )
    .unwrap();
    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::BeginPrepare).unwrap(),
        None,
        record.participants().to_vec(),
    )
    .unwrap();

    let mut participants = record.participants().to_vec();
    for p in &mut participants {
        if p.participant_id() == "p1" {
            *p = make_participant(
                "p1",
                ParticipantStatus::Prepared,
                Some(ParticipantVote::Prepared),
            );
        }
    }
    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::ParticipantPrepared)
            .unwrap(),
        None,
        participants,
    )
    .unwrap();

    let mut participants = record.participants().to_vec();
    for p in &mut participants {
        if p.participant_id() == "p4" {
            *p = make_participant(
                "p4",
                ParticipantStatus::VotedRollback,
                Some(ParticipantVote::Rollback),
            );
        }
    }
    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::ParticipantRollback)
            .unwrap(),
        None,
        participants,
    )
    .unwrap();

    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::AllResponded).unwrap(),
        None,
        record.participants().to_vec(),
    )
    .unwrap();
    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::DecideRollback)
            .unwrap(),
        Some(CoordinatorDecision::Rollback),
        record.participants().to_vec(),
    )
    .unwrap();
    let mut participants = record.participants().to_vec();
    for p in &mut participants {
        *p = make_participant(p.participant_id(), ParticipantStatus::RolledBack, p.vote());
    }
    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::AllResponded).unwrap(),
        record.decision(),
        participants,
    )
    .unwrap();

    assert_eq!(record.state(), TransactionState::RolledBack);
}

#[test]
fn single_participant_can_commit() {
    let tx_id = "tx-single";
    let participants = vec![make_participant("only", ParticipantStatus::Enrolled, None)];
    let mut record =
        make_transaction_record(tx_id, TransactionState::Init, None, participants).unwrap();

    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::BeginEnroll).unwrap(),
        None,
        record.participants().to_vec(),
    )
    .unwrap();
    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::BeginPrepare).unwrap(),
        None,
        record.participants().to_vec(),
    )
    .unwrap();

    let mut participants = record.participants().to_vec();
    for p in &mut participants {
        *p = make_participant(
            "only",
            ParticipantStatus::Prepared,
            Some(ParticipantVote::Prepared),
        );
    }
    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::ParticipantPrepared)
            .unwrap(),
        None,
        participants,
    )
    .unwrap();

    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::AllResponded).unwrap(),
        None,
        record.participants().to_vec(),
    )
    .unwrap();
    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::DecideCommit).unwrap(),
        Some(CoordinatorDecision::Commit),
        record.participants().to_vec(),
    )
    .unwrap();
    let mut participants = record.participants().to_vec();
    for p in &mut participants {
        *p = make_participant(p.participant_id(), ParticipantStatus::Committed, p.vote());
    }
    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::AllResponded).unwrap(),
        record.decision(),
        participants,
    )
    .unwrap();

    assert_eq!(record.state(), TransactionState::Committed);
}

#[test]
fn large_number_of_participants_all_prepare() {
    let tx_id = "tx-100p";
    let participants: Vec<ParticipantRecord> = (0..100)
        .map(|i| make_participant(&format!("p-{:03}", i), ParticipantStatus::Enrolled, None))
        .collect();
    let mut record =
        make_transaction_record(tx_id, TransactionState::Init, None, participants).unwrap();

    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::BeginEnroll).unwrap(),
        None,
        record.participants().to_vec(),
    )
    .unwrap();
    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::BeginPrepare).unwrap(),
        None,
        record.participants().to_vec(),
    )
    .unwrap();

    for i in 0..100 {
        let pid = format!("p-{:03}", i);
        let mut participants = record.participants().to_vec();
        for p in &mut participants {
            if p.participant_id() == pid {
                *p = make_participant(
                    &pid,
                    ParticipantStatus::Prepared,
                    Some(ParticipantVote::Prepared),
                );
            }
        }
        record = make_transaction_record(
            tx_id,
            apply_coordinator_transition(
                record.state(),
                CoordinatorTransition::ParticipantPrepared,
            )
            .unwrap(),
            None,
            participants,
        )
        .unwrap();
    }

    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::AllResponded).unwrap(),
        None,
        record.participants().to_vec(),
    )
    .unwrap();
    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::DecideCommit).unwrap(),
        Some(CoordinatorDecision::Commit),
        record.participants().to_vec(),
    )
    .unwrap();
    let mut participants = record.participants().to_vec();
    for p in &mut participants {
        *p = make_participant(p.participant_id(), ParticipantStatus::Committed, p.vote());
    }
    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::AllResponded).unwrap(),
        record.decision(),
        participants,
    )
    .unwrap();

    assert_eq!(record.state(), TransactionState::Committed);
    assert_eq!(record.participants().len(), 100);
}

#[test]
fn participant_vote_history_preserved() {
    let tx_id = "tx-vote-history";
    let participants = vec![
        make_participant("p1", ParticipantStatus::Enrolled, None),
        make_participant("p2", ParticipantStatus::Enrolled, None),
    ];
    let mut record =
        make_transaction_record(tx_id, TransactionState::Init, None, participants).unwrap();

    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::BeginEnroll).unwrap(),
        None,
        record.participants().to_vec(),
    )
    .unwrap();
    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::BeginPrepare).unwrap(),
        None,
        record.participants().to_vec(),
    )
    .unwrap();

    let mut participants = record.participants().to_vec();
    for p in &mut participants {
        if p.participant_id() == "p1" {
            *p = make_participant(
                "p1",
                ParticipantStatus::Prepared,
                Some(ParticipantVote::Prepared),
            );
        }
    }
    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::ParticipantPrepared)
            .unwrap(),
        None,
        participants,
    )
    .unwrap();

    let mut participants = record.participants().to_vec();
    for p in &mut participants {
        if p.participant_id() == "p2" {
            *p = make_participant(
                "p2",
                ParticipantStatus::VotedRollback,
                Some(ParticipantVote::Rollback),
            );
        }
    }
    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::ParticipantRollback)
            .unwrap(),
        None,
        participants,
    )
    .unwrap();

    let p1 = record
        .participants()
        .iter()
        .find(|p| p.participant_id() == "p1")
        .unwrap();
    let p2 = record
        .participants()
        .iter()
        .find(|p| p.participant_id() == "p2")
        .unwrap();

    assert_eq!(p1.vote(), Some(ParticipantVote::Prepared));
    assert_eq!(p1.status(), ParticipantStatus::Prepared);

    assert_eq!(p2.vote(), Some(ParticipantVote::Rollback));
    assert_eq!(p2.status(), ParticipantStatus::VotedRollback);
}

#[test]
fn rapid_participant_state_changes_stay_stable() {
    let tx_id = "tx-rapid";
    let participants = vec![
        make_participant("p1", ParticipantStatus::Enrolled, None),
        make_participant("p2", ParticipantStatus::Enrolled, None),
        make_participant("p3", ParticipantStatus::Enrolled, None),
    ];
    let mut record =
        make_transaction_record(tx_id, TransactionState::Init, None, participants).unwrap();

    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::BeginEnroll).unwrap(),
        None,
        record.participants().to_vec(),
    )
    .unwrap();
    record = make_transaction_record(
        tx_id,
        apply_coordinator_transition(record.state(), CoordinatorTransition::BeginPrepare).unwrap(),
        None,
        record.participants().to_vec(),
    )
    .unwrap();

    for _ in 0..10 {
        for pid in &["p1", "p2", "p3"] {
            let mut participants = record.participants().to_vec();
            for p in &mut participants {
                if p.participant_id() == *pid {
                    *p = make_participant(
                        pid,
                        ParticipantStatus::Prepared,
                        Some(ParticipantVote::Prepared),
                    );
                }
            }
            record = make_transaction_record(
                tx_id,
                apply_coordinator_transition(
                    record.state(),
                    CoordinatorTransition::ParticipantPrepared,
                )
                .unwrap(),
                None,
                participants,
            )
            .unwrap();
            assert_eq!(record.state(), TransactionState::Preparing);
        }
    }
}
