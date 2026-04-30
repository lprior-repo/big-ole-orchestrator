//! ADR-036: Broken Causation Chain Detection and Alerting - Tests
//!
//! These tests verify the causation chain integrity mechanisms defined in ADR-036:
//! 1. Valid causation chains pass integrity checks
//! 2. Broken chains are detected and alerted
//! 3. Missing parents trigger quarantine
//! 4. Chain repair mechanism replaces broken references with archival placeholders

use std::collections::HashSet;

use vo_types::{
    check_causation_chain_integrity, CausationIntegrityResult, CausationIntegrityFinding,
    is_valid_placeholder_reference, quarantine_broken_events, repair_broken_reference,
    repair_chain, RepairedReference,
};

/// A minimal event store for testing causation integrity.
#[derive(Debug, Clone)]
struct EventStore {
    events: HashSet<String>,
}

impl EventStore {
    fn new() -> Self {
        Self {
            events: HashSet::new(),
        }
    }

    fn insert(&mut self, command_id: &str) {
        self.events.insert(command_id.to_string());
    }

    fn contains(&self, reference: &str) -> bool {
        self.events.contains(reference)
    }
}

// ---------------------------------------------------------------------------
// Acceptance Test 1: Valid causation chain passes integrity check
// ---------------------------------------------------------------------------

#[test]
fn test_valid_causation_chain_passes_integrity_check() {
    let mut store = EventStore::new();

    store.insert("cmd-1");
    store.insert("cmd-2");
    store.insert("cmd-3");

    let causation_ids = vec![
        "external-root".to_string(), // root is always valid
        "cmd-1".to_string(),
        "cmd-2".to_string(),
    ];
    let instance_ids = vec![
        "inst-1".to_string(),
        "inst-1".to_string(),
        "inst-1".to_string(),
    ];
    let command_ids = vec![
        "cmd-1".to_string(),
        "cmd-2".to_string(),
        "cmd-3".to_string(),
    ];

    let result = check_causation_chain_integrity(
        &causation_ids,
        &instance_ids,
        &command_ids,
        |ref_id| store.contains(ref_id),
    );

    assert!(
        result.is_intact,
        "valid causation chain should pass integrity check, but found {} broken links",
        result.broken_links.len()
    );
    assert!(
        result.broken_links.is_empty(),
        "should have no broken links for valid chain"
    );
}

// ---------------------------------------------------------------------------
// Acceptance Test 2: Broken chain is detected and alerted
// ---------------------------------------------------------------------------

#[test]
fn test_broken_chain_detected_and_alerted() {
    let mut store = EventStore::new();

    store.insert("cmd-1");
    store.insert("cmd-3");
    // cmd-missing is NOT in the store

    let causation_ids = vec![
        "external-root".to_string(),
        "cmd-missing".to_string(), // broken reference
        "cmd-1".to_string(),
    ];
    let instance_ids = vec![
        "inst-1".to_string(),
        "inst-1".to_string(),
        "inst-1".to_string(),
    ];
    let command_ids = vec![
        "cmd-1".to_string(),
        "cmd-2".to_string(),
        "cmd-3".to_string(),
    ];

    let result = check_causation_chain_integrity(
        &causation_ids,
        &instance_ids,
        &command_ids,
        |ref_id| store.contains(ref_id),
    );

    assert!(
        !result.is_intact,
        "chain should be broken when reference is missing"
    );
    assert_eq!(
        result.broken_links.len(),
        1,
        "should detect exactly one broken reference"
    );

    let broken = &result.broken_links[0];
    assert_eq!(broken.broken_reference, "cmd-missing");
    assert_eq!(broken.quarantine_recommended, true);
}

// ---------------------------------------------------------------------------
// Acceptance Test 3: Missing parent triggers quarantine
// ---------------------------------------------------------------------------

#[test]
fn test_missing_parent_triggers_quarantine() {
    let mut store = EventStore::new();
    store.insert("cmd-parent");
    // cmd-deleted is NOT in the store

    let causation_ids = vec![
        "external-root".to_string(),
        "cmd-deleted".to_string(), // missing parent
        "cmd-parent".to_string(),
    ];
    let command_ids = vec![
        "cmd-1".to_string(),
        "cmd-child".to_string(),
        "cmd-parent".to_string(),
    ];

    let quarantined = quarantine_broken_events(
        &causation_ids,
        &command_ids,
        |ref_id| store.contains(ref_id),
    );

    assert!(
        quarantined.contains(&"cmd-child".to_string()),
        "cmd-child referencing missing parent should be quarantined"
    );
    assert_eq!(
        quarantined.len(),
        1,
        "should quarantine exactly one event"
    );
}

#[test]
fn test_valid_parent_does_not_trigger_quarantine() {
    let mut store = EventStore::new();
    store.insert("cmd-parent");

    let causation_ids = vec![
        "external-root".to_string(),
        "cmd-parent".to_string(),
    ];
    let command_ids = vec![
        "cmd-1".to_string(),
        "cmd-child".to_string(),
    ];

    let quarantined = quarantine_broken_events(
        &causation_ids,
        &command_ids,
        |ref_id| store.contains(ref_id),
    );

    assert!(
        quarantined.is_empty(),
        "no events should be quarantined when all parents exist"
    );
}

// ---------------------------------------------------------------------------
// Acceptance Test 4: Chain repair mechanism works
// ---------------------------------------------------------------------------

#[test]
fn test_repair_broken_reference_replaces_with_unknown_placeholder() {
    let mut store = EventStore::new();
    store.insert("cmd-parent");
    // cmd-deleted is NOT in the store

    let repaired = repair_broken_reference("cmd-deleted", |ref_id| store.contains(ref_id));

    assert!(
        repaired.is_some(),
        "broken reference should produce a repair"
    );

    let repaired = repaired.unwrap();
    assert_eq!(repaired.original, "cmd-deleted");
    assert_eq!(repaired.repaired, "unknown:cmd-deleted");
}

#[test]
fn test_repair_valid_reference_returns_none() {
    let mut store = EventStore::new();
    store.insert("cmd-parent");

    let repaired = repair_broken_reference("cmd-parent", |ref_id| store.contains(ref_id));

    assert!(
        repaired.is_none(),
        "valid reference should not need repair"
    );
}

#[test]
fn test_repair_placeholder_reference_returns_none() {
    let repaired = repair_broken_reference("unknown:cmd-abc", |_| false);

    assert!(
        repaired.is_none(),
        "placeholder reference should not need repair"
    );
}

#[test]
fn test_repair_root_reference_returns_none() {
    let repaired = repair_broken_reference("external-root", |_| false);

    assert!(
        repaired.is_none(),
        "root reference should not need repair"
    );
}

#[test]
fn test_repair_chain_replaces_all_broken() {
    let mut store = EventStore::new();
    store.insert("cmd-1");
    store.insert("cmd-2");
    store.insert("cmd-4");

    let causation_ids = vec![
        "cmd-1".to_string(),   // valid
        "cmd-missing".to_string(), // broken
        "cmd-2".to_string(),   // valid
        "cmd-deleted".to_string(), // broken
    ];

    let repaired = repair_chain(&causation_ids, |ref_id| store.contains(ref_id));

    assert_eq!(
        repaired.len(),
        2,
        "should repair exactly 2 broken references"
    );

    let has_missing = repaired.iter().any(|r| r.repaired == "unknown:cmd-missing");
    let has_deleted = repaired.iter().any(|r| r.repaired == "unknown:cmd-deleted");
    assert!(has_missing, "should repair cmd-missing");
    assert!(has_deleted, "should repair cmd-deleted");
}

#[test]
fn test_integrity_check_then_repair_mixed_chain() {
    let mut store = EventStore::new();
    store.insert("cmd-1");
    store.insert("cmd-2");
    store.insert("cmd-4");

    let causation_ids = vec![
        "cmd-1".to_string(),
        "cmd-missing".to_string(),
        "cmd-2".to_string(),
        "cmd-deleted".to_string(),
    ];
    let instance_ids = vec![
        "inst-1".to_string(),
        "inst-1".to_string(),
        "inst-1".to_string(),
        "inst-1".to_string(),
    ];
    let command_ids = vec![
        "cmd-1".to_string(),
        "cmd-2".to_string(),
        "cmd-3".to_string(),
        "cmd-4".to_string(),
    ];

    // Step 1: Integrity check
    let result = check_causation_chain_integrity(
        &causation_ids,
        &instance_ids,
        &command_ids,
        |ref_id| store.contains(ref_id),
    );

    assert!(!result.is_intact, "should detect broken links");
    assert_eq!(
        result.broken_links.len(),
        2,
        "should find exactly 2 broken references"
    );

    let broken_refs: Vec<&str> = result.broken_links.iter().map(|l| l.broken_reference.as_str()).collect();
    assert!(broken_refs.contains(&"cmd-missing"));
    assert!(broken_refs.contains(&"cmd-deleted"));

    // Step 2: Repair
    let repaired = repair_chain(&causation_ids, |ref_id| store.contains(ref_id));
    assert_eq!(repaired.len(), 2);

    // Step 3: After repair, integrity should pass
    let repaired_ids: Vec<String> = repaired
        .iter()
        .map(|r| r.repaired.clone())
        .collect();

    let combined_ids: Vec<String> = causation_ids
        .iter()
        .enumerate()
        .map(|(i, cid)| {
            if repaired.iter().any(|r| r.original == *cid) {
                repaired.iter().find(|r| r.original == *cid).unwrap().repaired.clone()
            } else {
                cid.clone()
            }
        })
        .collect();

    let final_result = check_causation_chain_integrity(
        &combined_ids,
        &instance_ids,
        &command_ids,
        |ref_id| store.contains(ref_id) || is_valid_placeholder_reference(ref_id),
    );

    assert!(
        final_result.is_intact,
        "after repair, chain should be intact"
    );
}

// ---------------------------------------------------------------------------
// Acceptance Test 5: Placeholder reference detection
// ---------------------------------------------------------------------------

#[test]
fn test_is_valid_placeholder_reference_archived() {
    assert!(is_valid_placeholder_reference("archived:seg-123"));
}

#[test]
fn test_is_valid_placeholder_reference_unknown() {
    assert!(is_valid_placeholder_reference("unknown:cmd-abc"));
}

#[test]
fn test_is_valid_placeholder_reference_root() {
    assert!(is_valid_placeholder_reference("external-root"));
}

#[test]
fn test_is_valid_placeholder_reference_normal_returns_false() {
    assert!(!is_valid_placeholder_reference("cmd-abc"));
}

// ---------------------------------------------------------------------------
// Acceptance Test 6: Quarantine respects placeholders
// ---------------------------------------------------------------------------

#[test]
fn test_quarantine_skips_placeholder_references() {
    let store = EventStore::new();

    let causation_ids = vec![
        "unknown:cmd-deleted".to_string(),
        "archived:seg-123".to_string(),
    ];
    let command_ids = vec![
        "cmd-repaired".to_string(),
        "cmd-collapsed".to_string(),
    ];

    let quarantined = quarantine_broken_events(
        &causation_ids,
        &command_ids,
        |_| false,
    );

    assert!(
        quarantined.is_empty(),
        "placeholder references should not trigger quarantine"
    );
}
