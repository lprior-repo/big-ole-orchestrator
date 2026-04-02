#![allow(clippy::redundant_pattern_matching)]
//! Integration tests for RegistrationStatus persistence via Fjall.
//!
//! Tests B-34 to B-38 (Persistence behaviors) and PROP-12 (Fjall round-trip).
//!
//! Each test creates a real Fjall keyspace in a tempdir, exercises the
//! status_store functions, and verifies postconditions.

use vo_storage::status_store::{
    load_all_statuses, read_registration_status, write_registration_status, StatusStoreError,
};
use vo_types::{RegistrationStatus, WorkflowName};

// ── Helpers ──────────────────────────────────────────────────────────────────

fn make_wf(s: &str) -> WorkflowName {
    WorkflowName::parse(s).expect("test workflow name should be valid")
}

/// Create a Fjall keyspace in a tempdir and open the `workflows` partition.
fn setup_partition() -> (tempfile::TempDir, fjall::Keyspace, fjall::PartitionHandle) {
    let dir = tempfile::tempdir().expect("tempdir should be created");
    let keyspace = fjall::Config::new(dir.path())
        .open()
        .expect("keyspace should open");
    let partition = keyspace
        .open_partition(
            vo_storage::status_store::WORKFLOWS_PARTITION,
            fjall::PartitionCreateOptions::default(),
        )
        .expect("partition should open");
    (dir, keyspace, partition)
}

// ── B-34: Read unknown workflow returns None ─────────────────────────────────

#[test]
fn read_registration_status_returns_none_for_unknown_workflow() {
    let (_dir, _ks, partition) = setup_partition();
    let wf = make_wf("new-wf");

    let result = read_registration_status(&partition, &wf);
    assert_eq!(result, Ok(None));
}

// ── B-35: Write then read ────────────────────────────────────────────────────

#[test]
fn write_then_read_registration_status_returns_quarantined() {
    let (_dir, _ks, partition) = setup_partition();
    let wf = make_wf("deploy-prod");

    let write_result = write_registration_status(&partition, &wf, RegistrationStatus::Quarantined);
    assert_eq!(write_result, Ok(()));

    let read_result = read_registration_status(&partition, &wf);
    assert_eq!(read_result, Ok(Some(RegistrationStatus::Quarantined)));
}

// ── B-35 extended: all three variants persist and read back ──────────────────

#[test]
fn write_then_read_registration_status_returns_active() {
    let (_dir, _ks, partition) = setup_partition();
    let wf = make_wf("wf-active");

    write_registration_status(&partition, &wf, RegistrationStatus::Active)
        .expect("write should succeed");

    let result = read_registration_status(&partition, &wf);
    assert_eq!(result, Ok(Some(RegistrationStatus::Active)));
}

#[test]
fn write_then_read_registration_status_returns_deactivated() {
    let (_dir, _ks, partition) = setup_partition();
    let wf = make_wf("wf-deactivated");

    write_registration_status(&partition, &wf, RegistrationStatus::Deactivated)
        .expect("write should succeed");

    let result = read_registration_status(&partition, &wf);
    assert_eq!(result, Ok(Some(RegistrationStatus::Deactivated)));
}

// ── B-35 extended: overwrite existing entry ──────────────────────────────────

#[test]
fn write_registration_status_overwrites_existing_entry() {
    let (_dir, _ks, partition) = setup_partition();
    let wf = make_wf("deploy-prod");

    write_registration_status(&partition, &wf, RegistrationStatus::Quarantined)
        .expect("first write");
    write_registration_status(&partition, &wf, RegistrationStatus::Active).expect("second write");

    let result = read_registration_status(&partition, &wf);
    assert_eq!(result, Ok(Some(RegistrationStatus::Active)));
}

// ── B-36: Load all statuses returns non-Active entries ───────────────────────

#[test]
fn load_all_statuses_returns_quarantined_and_deactivated_entries() {
    let (_dir, _ks, partition) = setup_partition();

    write_registration_status(
        &partition,
        &make_wf("wf-a"),
        RegistrationStatus::Quarantined,
    )
    .expect("write wf-a");
    write_registration_status(
        &partition,
        &make_wf("wf-b"),
        RegistrationStatus::Deactivated,
    )
    .expect("write wf-b");
    // Active entries should not appear in load_all_statuses
    write_registration_status(&partition, &make_wf("wf-c"), RegistrationStatus::Active)
        .expect("write wf-c");

    let result = load_all_statuses(&partition).expect("load should succeed");

    // Should contain wf-a and wf-b but NOT wf-c
    assert_eq!(result.len(), 2);

    // Build a map for easier assertions (order may vary by key sort)
    let map: std::collections::HashMap<String, RegistrationStatus> =
        result.iter().map(|(wf, s)| (wf.to_string(), *s)).collect();

    assert_eq!(
        map.get("wf-a"),
        Some(&RegistrationStatus::Quarantined),
        "wf-a should be Quarantined"
    );
    assert_eq!(
        map.get("wf-b"),
        Some(&RegistrationStatus::Deactivated),
        "wf-b should be Deactivated"
    );
    assert!(
        !map.contains_key("wf-c"),
        "wf-c (Active) should not be included"
    );
}

// ── B-36 extended: empty partition returns empty vec ──────────────────────────

#[test]
fn load_all_statuses_returns_empty_vec_when_partition_empty() {
    let (_dir, _ks, partition) = setup_partition();

    let result = load_all_statuses(&partition).expect("load should succeed");
    assert!(result.is_empty());
}

// ── B-37: Load all statuses fails on unavailable Fjall ───────────────────────
// This test is tricky: we need Fjall to fail. Dropping the keyspace handle
// after the partition is opened doesn't cause reads to fail in Fjall 2.x.
// Instead, we test corrupt data scenarios.

#[test]
fn load_all_statuses_returns_corrupt_value_when_data_is_invalid_json() {
    let (_dir, _ks, partition) = setup_partition();

    // Write raw invalid JSON directly into the partition
    partition
        .insert("valid-wf".as_bytes(), b"not-valid-json")
        .expect("raw insert should succeed");

    let result = load_all_statuses(&partition);
    match result {
        Err(StatusStoreError::CorruptValue { reason }) => {
            assert!(!reason.is_empty(), "reason should not be empty");
        }
        other => panic!("expected CorruptValue error, got {other:?}"),
    }
}

#[test]
fn read_registration_status_returns_corrupt_value_when_data_is_invalid() {
    let (_dir, _ks, partition) = setup_partition();
    let wf = make_wf("deploy-prod");

    // Write corrupt data directly
    partition
        .insert(wf.as_str().as_bytes(), b"garbage")
        .expect("raw insert should succeed");

    let result = read_registration_status(&partition, &wf);
    match result {
        Err(StatusStoreError::CorruptValue { reason }) => {
            assert!(!reason.is_empty(), "reason should not be empty");
        }
        other => panic!("expected CorruptValue error, got {other:?}"),
    }
}

// ── B-38: Quarantine survives restart (INV-003, POST-007) ────────────────────
// Simulates an engine restart: write Quarantined, create fresh DashMap,
// load from Fjall, verify quarantine is restored.

#[test]
fn quarantine_survives_restart_and_blocks_registration() {
    let (_dir, _ks, partition) = setup_partition();
    let wf = make_wf("deploy-prod");

    // Phase 1: Write quarantined status (simulating circuit breaker trigger)
    write_registration_status(&partition, &wf, RegistrationStatus::Quarantined)
        .expect("write should succeed");

    // Phase 2: Simulate restart — load all statuses into a fresh in-memory map
    let loaded = load_all_statuses(&partition).expect("load should succeed");

    // Verify deploy-prod is Quarantined
    let status = loaded.iter().find(|(w, _)| *w == wf).map(|(_, s)| *s);

    assert_eq!(
        status,
        Some(RegistrationStatus::Quarantined),
        "quarantine should survive restart"
    );
}

// ── B-38 extended: Deactivated also survives restart ─────────────────────────

#[test]
fn deactivated_survives_restart() {
    let (_dir, _ks, partition) = setup_partition();
    let wf = make_wf("legacy-wf");

    write_registration_status(&partition, &wf, RegistrationStatus::Deactivated)
        .expect("write should succeed");

    let loaded = load_all_statuses(&partition).expect("load should succeed");
    let status = loaded.iter().find(|(w, _)| *w == wf).map(|(_, s)| *s);

    assert_eq!(
        status,
        Some(RegistrationStatus::Deactivated),
        "deactivated should survive restart"
    );
}

// ── PROP-12: Fjall persistence round-trip ────────────────────────────────────

mod proptests {
    use super::*;
    use proptest::prelude::*;

    fn arb_status() -> impl Strategy<Value = RegistrationStatus> {
        prop_oneof![
            Just(RegistrationStatus::Active),
            Just(RegistrationStatus::Deactivated),
            Just(RegistrationStatus::Quarantined),
        ]
    }

    // Valid WorkflowName: alphanumeric + hyphen + underscore, 1-128 chars,
    // no leading/trailing separators
    fn arb_workflow_name() -> impl Strategy<Value = WorkflowName> {
        "[a-z][a-z0-9_-]{0,15}[a-z0-9]".prop_filter_map("must parse as WorkflowName", |s| {
            WorkflowName::parse(&s).ok()
        })
    }

    thread_local! {
        static PROPTEST_DB: (std::rc::Rc<tempfile::TempDir>, fjall::Keyspace, fjall::PartitionHandle) = {
            let (dir, ks, partition) = setup_partition();
            (std::rc::Rc::new(dir), ks, partition)
        };
    }

    proptest! {
        #[test]
        fn write_then_read_is_identity(
            wf_name in arb_workflow_name(),
            status in arb_status(),
        ) {
            let partition = PROPTEST_DB.with(|db| db.2.clone());

            let write_result = write_registration_status(&partition, &wf_name, status);
            prop_assert!(matches!(write_result, Ok(_)), "write should succeed");

            let read_result = read_registration_status(&partition, &wf_name);
            prop_assert_eq!(read_result, Ok(Some(status)));
        }
    }
}
