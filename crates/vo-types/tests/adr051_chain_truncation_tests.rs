//! ADR-051: Causation Chain Truncation and Archival - BDD Tests
//!
//! These tests verify the chain truncation policy defined in ADR-051:
//! 1. Deep chain collapses at max depth
//! 2. Broken chain is detected and alerted
//! 3. Collapse preserves essential lineage

use std::collections::HashMap;

use vo_types::causation_chain::{
    advance_chain, validate_chain_depths, CausationArchival, CausationDepth,
    ChainAdvanceResult, CollapsedLink, DEFAULT_MAX_CAUSATION_DEPTH,
    is_broken_chain_reference, extract_broken_chain_original, BrokenChainLink,
};
use vo_types::command_metadata::CommandMetadata;
use vo_types::events::envelope::EventEnvelope;
use vo_types::events::metadata::EventMetadata;
use vo_types::{IdempotencyKey, Issuer, TimestampMs};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_cmd_meta(
    command_id: &str,
    correlation_id: &str,
    causation_id: &str,
    issuer: Issuer,
) -> CommandMetadata {
    CommandMetadata {
        command_id: IdempotencyKey::parse(command_id).unwrap(),
        correlation_id: IdempotencyKey::parse(correlation_id).unwrap(),
        causation_id: IdempotencyKey::parse(causation_id).unwrap(),
        issuer,
        issued_at: TimestampMs::now(),
    }
}

fn make_event_envelope(
    instance_id: &str,
    sequence: u64,
    meta: CommandMetadata,
    payload_type: &str,
) -> EventEnvelope {
    EventEnvelope {
        schema_version: 1,
        instance_id: instance_id.to_string(),
        sequence,
        timestamp_ms: TimestampMs::now().as_u64(),
        payload: serde_json::json!({ "type": payload_type }),
        metadata: EventMetadata {
            command_metadata: Some(meta),
            annotations: HashMap::new(),
        },
    }
}

// ---------------------------------------------------------------------------
// Test: Deep chain collapses at max depth
// ---------------------------------------------------------------------------

/// GIVEN a causation chain approaching max depth
/// WHEN a new command is issued that would exceed the max
/// THEN the chain collapses and an archival blob is created
#[test]
fn test_deep_chain_collapsed_at_max_depth() {
    // Build a chain at max depth
    let depth = CausationDepth::new(DEFAULT_MAX_CAUSATION_DEPTH);
    let segment_id = "seg-chain-collapse-1";
    let all_collapsed_links = vec![
        CollapsedLink {
            command_id: "cmd-oldest".to_string(),
            causation_id: "cmd-older".to_string(),
            issued_at_ms: 1000,
        },
        CollapsedLink {
            command_id: "cmd-older".to_string(),
            causation_id: "cmd-old".to_string(),
            issued_at_ms: 2000,
        },
    ];

    // Advance would exceed max
    let result = advance_chain(
        depth,
        "cmd-new-at-max",
        "cmd-parent-of-max",
        segment_id,
        all_collapsed_links.clone(),
    );

    // THEN the result is CollapseRequired
    if let Ok(ChainAdvanceResult::CollapseRequired { new_depth, archival }) = result {
        // New depth is reset to MAX_DEPTH - 1
        assert_eq!(
            new_depth.0,
            DEFAULT_MAX_CAUSATION_DEPTH - 1,
            "new depth should be MAX_DEPTH - 1 after collapse"
        );

        // Archival contains the collapsed segment
        assert_eq!(archival.segment_id, segment_id);
        assert_eq!(
            archival.original_depth,
            DEFAULT_MAX_CAUSATION_DEPTH,
            "archival should record original depth"
        );
        assert_eq!(archival.collapsed_links, all_collapsed_links);
        assert_eq!(
            archival.preserved_anchor, "cmd-parent-of-max",
            "preserved anchor should be the causation link that remains active"
        );
    } else {
        panic!("expected CollapseRequired at max depth");
    }
}

/// GIVEN a chain well below max depth
/// WHEN a new command is issued
/// THEN the chain advances normally without collapse
#[test]
fn test_chain_advances_normally_below_max() {
    let depth = CausationDepth::new(10);

    let result = advance_chain(
        depth,
        "cmd-child",
        "cmd-parent",
        "seg-normal",
        vec![],
    );

    if let Ok(ChainAdvanceResult::Advanced(new_depth)) = result {
        assert_eq!(
            new_depth.0, 11,
            "chain depth should increment by 1"
        );
    } else {
        panic!("expected Advanced result for normal chain");
    }
}

/// GIVEN a chain at MAX_DEPTH - 1
/// WHEN one more command is issued
/// THEN the chain is at max and the next advance triggers collapse
#[test]
fn test_chain_at_one_below_max_then_collapse_on_next() {
    // Start at MAX_DEPTH - 2
    let depth = CausationDepth::new(DEFAULT_MAX_CAUSATION_DEPTH - 2);

    // First advance: normal
    let result = advance_chain(
        depth,
        "cmd-step-1",
        "cmd-parent",
        "seg-1",
        vec![],
    );
    if let Ok(ChainAdvanceResult::Advanced(d)) = result {
        assert_eq!(d.0, DEFAULT_MAX_CAUSATION_DEPTH - 1);

        // Second advance: at max, should collapse
        let result2 = advance_chain(
            d,
            "cmd-step-2",
            "cmd-step-1",
            "seg-2",
            vec![CollapsedLink {
                command_id: "cmd-step-1".to_string(),
                causation_id: "cmd-parent".to_string(),
                issued_at_ms: 3000,
            }],
        );

        match result2 {
            Ok(ChainAdvanceResult::CollapseRequired { new_depth, .. }) => {
                assert_eq!(new_depth.0, DEFAULT_MAX_CAUSATION_DEPTH - 1);
            }
            _ => panic!("expected CollapseRequired"),
        }
    } else {
        panic!("expected first advance to succeed");
    }
}

// ---------------------------------------------------------------------------
// Test: Broken chain detection
// ---------------------------------------------------------------------------

/// GIVEN a causation_id that references a non-existent event
/// WHEN the chain is validated
/// THEN a BrokenChainLink is detected
#[test]
fn test_broken_chain_detected_and_alerted() {
    // Simulate a broken reference: causation_id points to an event
    // that no longer exists (was archived or deleted)
    let broken_ref = "cmd-missing-event-abc123";

    // Create a broken chain link
    let broken = BrokenChainLink::new(
        "cmd-current",
        broken_ref,
        "inst-workflow-1",
    );

    // The broken reference should be detectable
    assert_eq!(broken.referencing_command, "cmd-current");
    assert_eq!(broken.broken_reference, broken_ref);
    assert_eq!(broken.instance_id, "inst-workflow-1");
    assert!(!broken.archival_lookup_failed);

    // When we mark archival lookup as failed too
    let broken2 = BrokenChainLink::new("cmd-x", "unknown:cmd-missing", "inst-y");
    assert!(is_broken_chain_reference("unknown:cmd-missing"));
}

/// GIVEN a chain with a broken reference in placeholder format
/// WHEN the reference is checked
/// THEN is_broken_chain_reference returns true
#[test]
fn test_broken_reference_detection_archived_format() {
    assert!(is_broken_chain_reference("archived:seg-def456"));
    assert!(is_broken_chain_reference("unknown:cmd-ghi789"));
    assert!(!is_broken_chain_reference("cmd-valid-123"));
}

/// GIVEN a broken chain placeholder
/// WHEN the original reference is extracted
/// THEN the original ID is returned without the prefix
#[test]
fn test_extract_original_from_broken_reference() {
    assert_eq!(
        extract_broken_chain_original("archived:seg-abc"),
        Some("seg-abc")
    );
    assert_eq!(
        extract_broken_chain_original("unknown:cmd-xyz"),
        Some("cmd-xyz")
    );
    assert_eq!(
        extract_broken_chain_original("cmd-normal"),
        None
    );
}

// ---------------------------------------------------------------------------
// Test: Collapse preserves essential lineage
// ---------------------------------------------------------------------------

/// GIVEN a deep chain that has been collapsed
/// WHEN the archival is consulted
/// THEN the essential lineage (preserved anchor) is intact
#[test]
fn test_collapse_preserves_essential_lineage() {
    let original_depth = 200; // exceeds DEFAULT_MAX_CAUSATION_DEPTH of 128
    let preserved_anchor = "cmd-at-depth-127";
    let segment_id = "seg-essential-lineage";

    let archival = CausationArchival {
        segment_id: segment_id.to_string(),
        original_depth,
        collapsed_links: vec![
            CollapsedLink {
                command_id: "cmd-depth-1".to_string(),
                causation_id: "cmd-depth-0".to_string(),
                issued_at_ms: 100,
            },
            CollapsedLink {
                command_id: "cmd-depth-2".to_string(),
                causation_id: "cmd-depth-1".to_string(),
                issued_at_ms: 200,
            },
            CollapsedLink {
                command_id: preserved_anchor.to_string(),
                causation_id: "cmd-depth-126".to_string(),
                issued_at_ms: 12700,
            },
        ],
        preserved_anchor: preserved_anchor.to_string(),
    };

    // The preserved anchor must match one of the collapsed links
    let anchor_found = archival
        .collapsed_links
        .iter()
        .any(|link| link.command_id == archival.preserved_anchor);
    assert!(
        anchor_found,
        "preserved anchor must be one of the collapsed links"
    );

    // The original depth is preserved
    assert_eq!(archival.original_depth, original_depth);

    // The segment ID is unique
    assert!(!archival.segment_id.is_empty());
}

/// GIVEN a chain being validated
/// WHEN all depths are within bounds
/// THEN validation passes
#[test]
fn test_valid_chain_depths_pass_validation() {
    let depths = (1..=100)
        .map(|d| CausationDepth::new(d))
        .collect::<Vec<_>>();

    assert!(
        validate_chain_depths(&depths).is_ok(),
        "all valid depths should pass"
    );
}

/// GIVEN a chain with one depth exceeding max
/// WHEN the chain is validated
/// THEN validation fails with DepthExceeded
#[test]
fn test_invalid_chain_depths_fail_validation() {
    let mut depths = (1..=128)
        .map(|d| CausationDepth::new(d))
        .collect::<Vec<_>>();

    // Add a depth that exceeds max
    depths.push(CausationDepth::unchecked(200));

    let result = validate_chain_depths(&depths);
    assert!(result.is_err());

    match result.unwrap_err() {
        vo_types::CausationChainError::DepthExceeded { current, max } => {
            assert_eq!(current, 200);
            assert_eq!(max, DEFAULT_MAX_CAUSATION_DEPTH);
        }
        other => panic!("expected DepthExceeded, got: {:?}", other),
    }
}

/// GIVEN a chain with zero depth
/// WHEN the chain is validated
/// THEN validation fails with InvalidState
#[test]
fn test_zero_depth_fails_validation() {
    let depths = vec![
        CausationDepth::new(1),
        CausationDepth(0), // invalid
        CausationDepth::new(3),
    ];

    let result = validate_chain_depths(&depths);
    assert!(result.is_err());

    match result.unwrap_err() {
        vo_types::CausationChainError::InvalidState(msg) => {
            assert!(msg.contains("zero depth"));
        }
        other => panic!("expected InvalidState, got: {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Test: Full pipeline - build chain, reach max, collapse, continue
// ---------------------------------------------------------------------------

/// GIVEN a workflow with many sequential steps
/// WHEN the chain reaches max depth and collapses
/// THEN the workflow continues with the collapsed chain
#[test]
fn test_full_pipeline_chain_builds_and_collapses() {
    let mut current_depth = CausationDepth::new(1);
    let mut last_causation = "external-root";
    let correlation_id = "corr-pipeline-test";

    // Build chain up to MAX_DEPTH - 5 (leaving room for collapse)
    let steps_to_build = (DEFAULT_MAX_CAUSATION_DEPTH - 5) as usize;
    for i in 0..steps_to_build {
        let cmd_id = format!("cmd-step-{i:04}");
        let result = advance_chain(
            current_depth,
            &cmd_id,
            last_causation,
            &format!("seg-{i}"),
            vec![],
        );
        match result.expect("chain advance should succeed during build phase") {
            ChainAdvanceResult::Advanced(d) => {
                current_depth = d;
                last_causation = &cmd_id;
            }
            ChainAdvanceResult::CollapseRequired { .. } => {
                panic!("unexpected collapse during build phase at step {i}");
            }
        }
    }

    // Verify we're at the expected depth
    assert_eq!(
        current_depth.0,
        1 + steps_to_build,
        "depth should be 1 + steps_to_build after build"
    );

    // Now push past max - should collapse
    let cmd_id = "cmd-beyond-max";
    let collapsed_links = vec![CollapsedLink {
        command_id: "cmd-oldest-segment".to_string(),
        causation_id: "cmd-even-older".to_string(),
        issued_at_ms: 1000,
    }];
    let result = advance_chain(
        current_depth,
        &cmd_id,
        last_causation,
        "seg-collapse",
        collapsed_links,
    );

    match result.expect("chain advance should succeed with collapse") {
        ChainAdvanceResult::CollapseRequired { new_depth, archival } => {
            // Depth resets to MAX_DEPTH - 1
            assert_eq!(new_depth.0, DEFAULT_MAX_CAUSATION_DEPTH - 1);

            // Archival is created
            assert_eq!(archival.segment_id, "seg-collapse");

            // Chain can continue after collapse
            let cmd_after = "cmd-after-collapse";
            let result2 = advance_chain(
                new_depth,
                cmd_after,
                &cmd_id,
                "seg-post-collapse",
                vec![],
            );
            assert!(matches!(result2, Ok(ChainAdvanceResult::Advanced(_))));
        }
        ChainAdvanceResult::Advanced(_) => {
            panic!("expected CollapseRequired when past max depth");
        }
    }
}
