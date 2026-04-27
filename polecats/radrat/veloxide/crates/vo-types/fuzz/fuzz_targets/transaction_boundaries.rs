#![no_main]

use libfuzzer_sys::fuzz_target;
use vo_types::{
    apply_coordinator_transition, CoordinatorDecision, CoordinatorTransition,
    CoordinatorTransitionError, ParticipantRecord, ParticipantStatus, TransactionRecord,
    TransactionState,
};

const MAX_STEPS: usize = 500;

fn all_states() -> &'static [TransactionState] {
    TransactionState::all_variants()
}

fn all_transitions() -> &'static [CoordinatorTransition] {
    CoordinatorTransition::all_variants()
}

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    let first_byte = data[0];
    let corpus = &data[1..];

    match first_byte % 5 {
        0 => fuzz_transition_sequences(corpus),
        1 => fuzz_state_machine_exhaustive(corpus),
        2 => fuzz_serde_roundtrip(corpus),
        3 => fuzz_record_construction(corpus),
        4 => fuzz_transition_error_taxonomy(corpus),
        _ => {}
    }
});

fn fuzz_transition_sequences(data: &[u8]) {
    let all_trans = all_transitions();
    let trans_len = all_trans.len();

    let mut state = TransactionState::Init;
    let mut steps = 0usize;

    for &byte in data.iter().take(MAX_STEPS) {
        let idx = byte as usize % trans_len;
        let event = all_trans[idx];

        let was_terminal = state.is_terminal();

        match apply_coordinator_transition(state, event) {
            Ok(new_state) => {
                assert!(
                    !was_terminal,
                    "INV-TC-003 violated: transition from terminal state {:?} with event {:?}",
                    state, event
                );
                state = new_state;

                if state.is_terminal() {
                    assert!(
                        matches!(
                            state,
                            TransactionState::Committed
                                | TransactionState::RolledBack
                                | TransactionState::Aborted
                        ),
                        "INV-TC-014 violated: is_terminal() true for non-terminal {:?}",
                        state
                    );
                }
            }
            Err(CoordinatorTransitionError::TerminalStateTransition) => {
                assert!(
                    state.is_terminal(),
                    "TerminalStateTransition error but state {:?} is not terminal",
                    state
                );
            }
            Err(CoordinatorTransitionError::InvalidTransition) => {
                assert!(
                    !state.is_terminal(),
                    "InvalidTransition error from terminal state {:?}",
                    state
                );
            }
            Err(CoordinatorTransitionError::InsufficientVotes) => {
                assert!(
                    !state.is_terminal(),
                    "InsufficientVotes error from terminal state {:?}",
                    state
                );
            }
        }

        steps += 1;
    }

    let _ = steps;
}

fn fuzz_state_machine_exhaustive(data: &[u8]) {
    let states = all_states();
    let transitions = all_transitions();

    let seed = if data.is_empty() {
        0u64
    } else {
        u64::from_le_bytes(data[0..8.min(data.len())].try_into().unwrap_or([0; 8]))
    };

    let start_idx = (seed as usize) % states.len();
    let state = states[start_idx];

    let mut valid_count = 0usize;
    let mut invalid_count = 0usize;
    let mut terminal_rejections = 0usize;

    for &event in transitions {
        match apply_coordinator_transition(state, event) {
            Ok(_) => valid_count += 1,
            Err(CoordinatorTransitionError::TerminalStateTransition) => {
                assert!(
                    state.is_terminal(),
                    "INV-TC-003: non-terminal {:?} returned TerminalStateTransition",
                    state
                );
                terminal_rejections += 1;
            }
            Err(CoordinatorTransitionError::InvalidTransition) => {
                assert!(
                    !state.is_terminal(),
                    "INV-TC-003: terminal {:?} returned InvalidTransition",
                    state
                );
                invalid_count += 1;
            }
            Err(CoordinatorTransitionError::InsufficientVotes) => {
                invalid_count += 1;
            }
        }
    }

    if state.is_terminal() {
        assert_eq!(
            valid_count, 0,
            "Terminal state {:?} should have 0 valid transitions",
            state
        );
        assert_eq!(
            terminal_rejections,
            transitions.len(),
            "Terminal state {:?} should reject all {} transitions",
            state,
            transitions.len()
        );
    } else {
        assert!(
            valid_count > 0,
            "Non-terminal state {:?} should have at least 1 valid transition",
            state
        );
    }

    let _ = (valid_count, invalid_count, terminal_rejections);
}

fn fuzz_serde_roundtrip(data: &[u8]) {
    let Ok(s) = std::str::from_utf8(data) else {
        return;
    };

    let Ok(json_val) = serde_json::from_str::<serde_json::Value>(s) else {
        return;
    };

    let Some(state_str) = json_val.as_str() else {
        return;
    };

    let state_names = [
        "Init",
        "Enrolling",
        "Preparing",
        "Prepared",
        "Committing",
        "Committed",
        "RollingBack",
        "RolledBack",
        "Aborted",
        "Ambiguous",
    ];

    if !state_names.contains(&state_str) {
        return;
    }

    let Ok(state) = serde_json::from_str::<TransactionState>(&format!("\"{}\"", state_str)) else {
        return;
    };

    let re_json = serde_json::to_string(&state).unwrap_or_default();
    let Ok(re_state) = serde_json::from_str::<TransactionState>(&re_json) else {
        return;
    };

    assert_eq!(
        state, re_state,
        "TransactionState serde round-trip failed for {}",
        state_str
    );
}

fn fuzz_record_construction(data: &[u8]) {
    let Ok(s) = std::str::from_utf8(data) else {
        return;
    };

    let Ok(json_val) = serde_json::from_str::<serde_json::Value>(s) else {
        return;
    };

    let Some(obj) = json_val.as_object() else {
        return;
    };

    let tx_id = obj
        .get("transaction_id")
        .and_then(|v| v.as_str())
        .unwrap_or("fuzz-tx");

    let state_idx = obj.get("state").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

    let states = all_states();
    let state = states[state_idx % states.len()];

    let decision_idx = obj.get("decision").and_then(|v| v.as_u64()).unwrap_or(255) as usize;

    let decision = match decision_idx % 3 {
        0 => Some(CoordinatorDecision::Commit),
        1 => Some(CoordinatorDecision::Rollback),
        _ => None,
    };

    let num_participants = obj
        .get("participants")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;

    let num_participants = num_participants.min(100);

    let participants: Vec<ParticipantRecord> = (0..num_participants)
        .filter_map(|i| {
            let status_idx = (i + data.len()) % ParticipantStatus::all_variants().len();
            let status = ParticipantStatus::all_variants()[status_idx];
            ParticipantRecord::new(format!("p-{}", i), status, None)
        })
        .collect();

    let record = TransactionRecord::new(
        tx_id.to_string(),
        state,
        decision,
        participants,
        None,
        None,
        None,
    );

    assert!(
        record.is_some(),
        "TransactionRecord::new should accept non-empty id"
    );

    let record = record.unwrap();
    assert_eq!(record.transaction_id(), tx_id);
    assert_eq!(record.state(), state);
    assert_eq!(record.decision(), decision);

    let json = serde_json::to_value(&record).unwrap_or_default();
    let restored: TransactionRecord = serde_json::from_value(json).unwrap_or_else(|_| {
        TransactionRecord::new(
            "fallback".to_string(),
            TransactionState::Init,
            None,
            vec![],
            None,
            None,
            None,
        )
        .unwrap()
    });

    assert_eq!(record.transaction_id(), restored.transaction_id());
    assert_eq!(record.state(), restored.state());

    let empty_record = TransactionRecord::new(String::new(), state, None, vec![], None, None, None);
    assert!(
        empty_record.is_none(),
        "INV-TC-001: empty transaction_id must return None"
    );

    let empty_participant =
        ParticipantRecord::new(String::new(), ParticipantStatus::Enrolled, None);
    assert!(
        empty_participant.is_none(),
        "INV-TC-002: empty participant_id must return None"
    );
}

fn fuzz_transition_error_taxonomy(data: &[u8]) {
    if data.len() < 2 {
        return;
    }

    let state_idx = data[0] as usize % all_states().len();
    let event_idx = data[1] as usize % all_transitions().len();

    let state = all_states()[state_idx];
    let event = all_transitions()[event_idx];

    let result = apply_coordinator_transition(state, event);

    match result {
        Ok(new_state) => {
            assert_ne!(
                state,
                TransactionState::Committed,
                "Terminal Committed should not accept transitions"
            );
            assert_ne!(
                state,
                TransactionState::RolledBack,
                "Terminal RolledBack should not accept transitions"
            );
            assert_ne!(
                state,
                TransactionState::Aborted,
                "Terminal Aborted should not accept transitions"
            );

            if matches!(event, CoordinatorTransition::Recover) && !state.is_terminal() {
                assert_eq!(
                    new_state,
                    TransactionState::Ambiguous,
                    "INV-TC-005: Recover from any non-terminal should go to Ambiguous"
                );
            }

            if state.is_terminal() {
                unreachable!(
                    "INV-TC-003: transition from terminal state {:?} succeeded",
                    state
                );
            }
        }
        Err(CoordinatorTransitionError::TerminalStateTransition) => {
            assert!(
                state.is_terminal(),
                "TerminalStateTransition from non-terminal {:?} with {:?}",
                state,
                event
            );
        }
        Err(CoordinatorTransitionError::InvalidTransition) => {
            assert!(
                !state.is_terminal(),
                "InvalidTransition from terminal {:?} should be TerminalStateTransition",
                state
            );
        }
        Err(CoordinatorTransitionError::InsufficientVotes) => {
            assert!(
                !state.is_terminal(),
                "InsufficientVotes from terminal {:?} should be TerminalStateTransition",
                state
            );
        }
    }

    let _ = result;
}
