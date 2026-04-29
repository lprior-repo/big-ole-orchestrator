//! BDD test: Dedupe mutating commands by command_id (ADR-028, ADR-036).
//!
//! Given a mutating CommandEnvelope with command_id C has already committed,
//! When the same command arrives again,
//! Then the original outcome is returned and no duplicate event is appended.

use vo_core::{check_command_duplicate, is_command_duplicate, CommandDedupResult};
use vo_storage::dedupe_partition::{DedupeStore, InMemoryDedupeStore};
use vo_types::{CommandEnvelope, CommandMetadata, IdempotencyKey, InstanceId, Issuer, TimestampMs};

fn make_envelope(command_id: &str) -> CommandEnvelope {
    CommandEnvelope {
        schema_version: 1,
        metadata: CommandMetadata {
            command_id: IdempotencyKey::parse(command_id).unwrap(),
            correlation_id: IdempotencyKey::parse("corr-bdd-test").unwrap(),
            causation_id: IdempotencyKey::parse("cause-bdd-test").unwrap(),
            issuer: Issuer::ApiClient,
            issued_at: TimestampMs::try_from(1_700_000_000u64).unwrap(),
        },
    }
}

#[test]
fn given_duplicate_command_id_when_mutation_replayed_then_original_outcome_is_returned() {
    // --- GIVEN ---
    // A mutating CommandEnvelope with command_id C has already committed
    let store = InMemoryDedupeStore::new();
    let envelope = make_envelope("cmd-bdd-dedupe-001");
    let instance_id = InstanceId::parse("inst-bdd-001").unwrap();
    let ttl_ms: u64 = 60_000;

    // First submission commits successfully
    let first_result = check_command_duplicate(&envelope, &store, &instance_id, ttl_ms).unwrap();
    assert_eq!(
        first_result,
        CommandDedupResult::Admitted,
        "first occurrence must be admitted"
    );

    // Verify it's now tracked
    assert!(
        is_command_duplicate(&envelope, &store).unwrap(),
        "after admission, command_id must be tracked as duplicate"
    );

    // --- WHEN ---
    // The same command arrives again
    let replayed_envelope = make_envelope("cmd-bdd-dedupe-001");
    let replayed_instance = InstanceId::parse("inst-bdd-999").unwrap();
    let second_result =
        check_command_duplicate(&replayed_envelope, &store, &replayed_instance, ttl_ms).unwrap();

    // --- THEN ---
    // The original outcome is returned
    assert_eq!(
        second_result,
        CommandDedupResult::Duplicate {
            original_instance_id: "inst-bdd-001".to_string(),
        },
        "duplicate must return the original instance_id, not the replayed one"
    );

    // No duplicate event is appended — the replayed instance_id is NOT stored
    let contains_replayed = store
        .contains(&vo_types::DedupeKey::parse("cmd:cmd-bdd-dedupe-001").unwrap())
        .unwrap();
    assert!(
        contains_replayed,
        "dedupe key must still point to the original entry only"
    );
}

#[test]
fn given_different_command_ids_when_both_submitted_then_both_admitted() {
    let store = InMemoryDedupeStore::new();
    let env_a = make_envelope("cmd-alpha-unique");
    let env_b = make_envelope("cmd-beta-unique");
    let iid = InstanceId::parse("inst-001").unwrap();

    let r_a = check_command_duplicate(&env_a, &store, &iid, 60_000).unwrap();
    let r_b = check_command_duplicate(&env_b, &store, &iid, 60_000).unwrap();

    assert_eq!(r_a, CommandDedupResult::Admitted);
    assert_eq!(r_b, CommandDedupResult::Admitted);
}

#[test]
fn given_no_prior_submission_when_checked_for_duplicate_then_returns_false() {
    let store = InMemoryDedupeStore::new();
    let envelope = make_envelope("cmd-never-seen");

    assert!(
        !is_command_duplicate(&envelope, &store).unwrap(),
        "unsubmitted command must not be flagged as duplicate"
    );
}
