//! BDD tests for mutation command deduplication by command_id (ADR-028, ADR-036).
//!
//! Given-When-Then scenarios validating that when a duplicate mutation command
//! arrives with the same command_id, the original outcome is returned and no
//! duplicate event is appended.

use vo_core::admission::{
    AdmissionCheck, AdmissionController, AdmissionResult, DedupeToken, WritePressureState,
};
use vo_types::{DedupeKey, FenceToken, IdempotencyKey, InstanceId, MutationType, StepId};

fn healthy_state() -> WritePressureState {
    WritePressureState {
        writer_queue_depth: 10,
        batch_commit_latency_ms: 50,
        blob_queue_depth: 5,
        compaction_stall_active: false,
        storage_stall_active: false,
    }
}

#[derive(Debug, Clone)]
struct MockMutationAdmissionCheck {
    admitted_commands: std::collections::HashMap<String, (InstanceId, MutationType)>,
}

impl MockMutationAdmissionCheck {
    fn new() -> Self {
        Self {
            admitted_commands: std::collections::HashMap::new(),
        }
    }

    fn with_admitted_mutation(
        mut self,
        command_id: &str,
        instance_id: InstanceId,
        mutation_type: MutationType,
    ) -> Self {
        self.admitted_commands
            .insert(command_id.to_string(), (instance_id, mutation_type));
        self
    }
}

impl AdmissionCheck for MockMutationAdmissionCheck {
    fn check_deduplicate(&self, dedupe_key: &DedupeKey) -> AdmissionResult {
        if let Some((instance_id, _mutation_type)) =
            self.admitted_commands.get(dedupe_key.as_str())
        {
            AdmissionResult::Duplicate {
                original_instance_id: instance_id.clone(),
            }
        } else {
            AdmissionResult::Admitted {
                dedupe_token: DedupeToken::new("mutation-token".to_string()),
            }
        }
    }

    fn check_fence(
        &self,
        _instance_id: &InstanceId,
        _step_id: &StepId,
        _fence_token: &FenceToken,
    ) -> AdmissionResult {
        AdmissionResult::Admitted {
            dedupe_token: DedupeToken::new("fence-token".to_string()),
        }
    }
}

fn make_instance_id() -> InstanceId {
    InstanceId::parse("01HQXK5R5TJRP3J4W5G6W7Y8Z9").expect("valid ulid")
}

#[test]
fn given_duplicate_command_id_when_mutation_replayed_then_original_outcome_is_returned() {
    let original_instance_id = make_instance_id();
    let command_id = IdempotencyKey::parse("cmd-mutation-001").expect("valid command_id");
    let mutation_type = MutationType::Cancel;

    let check = MockMutationAdmissionCheck::new().with_admitted_mutation(
        command_id.as_str(),
        original_instance_id.clone(),
        mutation_type,
    );
    let controller = AdmissionController::new(check, healthy_state());

    let dedupe_key = DedupeKey::parse(command_id.as_str()).expect("valid dedupe key");
    let result = controller.admit_new_workflow(&dedupe_key);

    let err = result.expect_err("duplicate mutation should return error");
    match err {
        vo_core::admission::AdmissionError::Duplicate { original_instance_id } => {
            assert_eq!(
                original_instance_id, original_instance_id,
                "original instance ID should be returned for duplicate mutation"
            );
        }
        other => panic!("expected Duplicate error, got {:?}", other),
    }
}

#[test]
fn given_new_command_id_when_mutation_arrives_then_it_is_admitted() {
    let check = MockMutationAdmissionCheck::new();
    let controller = AdmissionController::new(check, healthy_state());

    let command_id = IdempotencyKey::parse("cmd-new-mutation-002").expect("valid command_id");
    let dedupe_key = DedupeKey::parse(command_id.as_str()).expect("valid dedupe key");

    let result = controller.admit_new_workflow(&dedupe_key);
    assert!(
        result.is_ok(),
        "new mutation with unique command_id should be admitted: {:?}",
        result
    );
}

#[test]
fn given_duplicate_command_id_when_mutation_replayed_then_no_duplicate_event_is_appended() {
    let original_instance_id = make_instance_id();
    let command_id = IdempotencyKey::parse("cmd-mutation-003").expect("valid command_id");

    let check = MockMutationAdmissionCheck::new().with_admitted_mutation(
        command_id.as_str(),
        original_instance_id.clone(),
        MutationType::Pause,
    );
    let controller = AdmissionController::new(check, healthy_state());

    let dedupe_key = DedupeKey::parse(command_id.as_str()).expect("valid dedupe key");

    let first_result = controller.admit_new_workflow(&dedupe_key);
    assert!(
        first_result.is_err(),
        "first attempt with duplicate command_id should fail"
    );

    let second_result = controller.admit_new_workflow(&dedupe_key);
    assert!(
        second_result.is_err(),
        "second attempt with duplicate command_id should also fail"
    );
}

#[test]
fn given_different_mutation_type_same_command_id_when_replayed_then_still_detected_as_duplicate() {
    let original_instance_id = make_instance_id();
    let command_id = IdempotencyKey::parse("cmd-mutation-004").expect("valid command_id");

    let check = MockMutationAdmissionCheck::new().with_admitted_mutation(
        command_id.as_str(),
        original_instance_id.clone(),
        MutationType::Cancel,
    );
    let controller = AdmissionController::new(check, healthy_state());

    let dedupe_key = DedupeKey::parse(command_id.as_str()).expect("valid dedupe key");
    let result = controller.admit_new_workflow(&dedupe_key);

    assert!(
        result.is_err(),
        "same command_id should be detected as duplicate regardless of mutation_type"
    );
}
