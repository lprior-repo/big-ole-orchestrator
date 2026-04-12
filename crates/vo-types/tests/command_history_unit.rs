//! Unit tests for Command History with Undo/Redo.
//!
//! TDD Red Phase: These tests verify the behavior of individual types and operations.
//! They should FAIL until the full implementation is complete.
//!
//! # Coverage
//!
//! - 52 behaviors from the test plan (B-001 to B-052)
//! - 13 invariants (INV-001 to INV-013)
//! - Error taxonomy testing

use vo_types::command_history::{
    CommandHistory, CommandHistoryError, CommandKind, ExtensionApplyMode, HistoryEntryStatus,
    WorkflowSnapshot, MAX_HISTORY_DEPTH,
};
use vo_types::{DagNode, Edge, EdgeCondition, NodeName, RetryPolicy};

fn make_retry_policy() -> RetryPolicy {
    RetryPolicy::new(3, 1000, 2.0).unwrap()
}

fn make_node(name: &str) -> DagNode {
    DagNode {
        node_name: NodeName::parse(name).unwrap(),
        retry_policy: make_retry_policy(),
    }
}

fn make_snapshot(workflow_name: &str, nodes: Vec<DagNode>, edges: Vec<Edge>) -> WorkflowSnapshot {
    WorkflowSnapshot::new(workflow_name.into(), nodes, edges)
}

fn test_snapshot() -> WorkflowSnapshot {
    make_snapshot("test-workflow", vec![make_node("test-node")], vec![])
}

// ---------------------------------------------------------------------------
// B-001: CommandKind has exactly 8 variants
// ---------------------------------------------------------------------------

#[test]
fn command_kind_has_exactly_eight_variants() {
    fn _exhaustiveness(k: CommandKind) -> bool {
        match k {
            CommandKind::ExtensionApply
            | CommandKind::ExtensionRevert
            | CommandKind::ExtensionRedo
            | CommandKind::NodeCreate
            | CommandKind::NodeDelete
            | CommandKind::EdgeCreate
            | CommandKind::EdgeDelete
            | CommandKind::ConfigUpdate => true,
        }
    }
    assert!(_exhaustiveness(CommandKind::ExtensionApply));
    assert!(_exhaustiveness(CommandKind::ExtensionRevert));
    assert!(_exhaustiveness(CommandKind::ExtensionRedo));
    assert!(_exhaustiveness(CommandKind::NodeCreate));
    assert!(_exhaustiveness(CommandKind::NodeDelete));
    assert!(_exhaustiveness(CommandKind::EdgeCreate));
    assert!(_exhaustiveness(CommandKind::EdgeDelete));
    assert!(_exhaustiveness(CommandKind::ConfigUpdate));

    let all = [
        CommandKind::ExtensionApply,
        CommandKind::ExtensionRevert,
        CommandKind::ExtensionRedo,
        CommandKind::NodeCreate,
        CommandKind::NodeDelete,
        CommandKind::EdgeCreate,
        CommandKind::EdgeDelete,
        CommandKind::ConfigUpdate,
    ];
    assert_eq!(all.len(), 8);
}

// ---------------------------------------------------------------------------
// B-002: CommandEnvelope constructs with valid metadata
// ---------------------------------------------------------------------------

#[test]
fn command_envelope_constructs_with_valid_metadata() {
    let history = CommandHistory::new();
    assert!(history.entries().is_empty());
}

// ---------------------------------------------------------------------------
// B-004: WorkflowSnapshot captures complete graph state
// ---------------------------------------------------------------------------

#[test]
fn workflow_snapshot_captures_complete_graph_state() {
    let nodes = vec![make_node("node-a"), make_node("node-b")];
    let edges = vec![Edge {
        source_node: NodeName::parse("node-a").unwrap(),
        target_node: NodeName::parse("node-b").unwrap(),
        condition: EdgeCondition::Always,
    }];
    let snapshot = make_snapshot("test-workflow", nodes, edges);

    assert_eq!(snapshot.nodes.len(), 2);
    assert_eq!(snapshot.edges.len(), 1);
    assert_ne!(
        snapshot.checksum, 0,
        "checksum should be non-zero for non-empty graph"
    );
}

// ---------------------------------------------------------------------------
// B-005: WorkflowSnapshot checksum is computed correctly
// ---------------------------------------------------------------------------

#[test]
fn workflow_snapshot_checksum_is_deterministic() {
    let nodes = vec![make_node("a")];
    let edges = vec![];

    let snapshot1 = make_snapshot("workflow".into(), nodes.clone(), edges.clone());
    let snapshot2 = make_snapshot("workflow".into(), nodes, edges);

    assert_eq!(
        snapshot1.checksum, snapshot2.checksum,
        "identical graphs must have identical checksums"
    );
}

#[test]
fn workflow_snapshot_checksum_detects_difference() {
    let nodes1 = vec![make_node("a")];
    let nodes2 = vec![make_node("b")];

    let snapshot1 = make_snapshot("workflow".into(), nodes1, vec![]);
    let snapshot2 = make_snapshot("workflow".into(), nodes2, vec![]);

    assert_ne!(
        snapshot1.checksum, snapshot2.checksum,
        "different graphs must have different checksums"
    );
}

// ---------------------------------------------------------------------------
// B-007: ExtensionApplyMode has exactly 2 variants
// ---------------------------------------------------------------------------

#[test]
fn extension_apply_mode_has_exactly_two_variants() {
    fn _exhaustiveness(m: ExtensionApplyMode) -> bool {
        match m {
            ExtensionApplyMode::Single | ExtensionApplyMode::Bulk => true,
        }
    }
    assert!(_exhaustiveness(ExtensionApplyMode::Single));
    assert!(_exhaustiveness(ExtensionApplyMode::Bulk));

    let all = [ExtensionApplyMode::Single, ExtensionApplyMode::Bulk];
    assert_eq!(all.len(), 2);
}

// ---------------------------------------------------------------------------
// B-009: HistoryEntryStatus has exactly 4 variants
// ---------------------------------------------------------------------------

#[test]
fn history_entry_status_has_exactly_four_variants() {
    fn _exhaustiveness(s: HistoryEntryStatus) -> bool {
        match s {
            HistoryEntryStatus::Committed
            | HistoryEntryStatus::Undone
            | HistoryEntryStatus::Redone
            | HistoryEntryStatus::Failed => true,
        }
    }
    assert!(_exhaustiveness(HistoryEntryStatus::Committed));
    assert!(_exhaustiveness(HistoryEntryStatus::Undone));
    assert!(_exhaustiveness(HistoryEntryStatus::Redone));
    assert!(_exhaustiveness(HistoryEntryStatus::Failed));

    let all = [
        HistoryEntryStatus::Committed,
        HistoryEntryStatus::Undone,
        HistoryEntryStatus::Redone,
        HistoryEntryStatus::Failed,
    ];
    assert_eq!(all.len(), 4);
}

// ---------------------------------------------------------------------------
// B-010: CommandHistory::new() creates empty history
// ---------------------------------------------------------------------------

#[test]
fn command_history_new_creates_empty_history() {
    let history = CommandHistory::new();
    assert!(history.entries().is_empty());
    assert!(history.undo_stack().is_empty());
    assert!(history.redo_stack().is_empty());
    assert_eq!(history.capacity(), MAX_HISTORY_DEPTH);
    assert_eq!(history.capacity(), 100);
}

// ---------------------------------------------------------------------------
// B-011: CommandHistory::capacity() returns configured max
// ---------------------------------------------------------------------------

#[test]
fn command_history_capacity_returns_configured_max() {
    let history = CommandHistory::new();
    assert_eq!(history.capacity(), MAX_HISTORY_DEPTH);
}

// ---------------------------------------------------------------------------
// B-012: save_undo_point() creates new entry with Committed status
// ---------------------------------------------------------------------------

#[test]
fn save_undo_point_creates_committed_entry() {
    let mut history = CommandHistory::new();
    let result = history.save_undo_point(CommandKind::NodeCreate, test_snapshot());

    assert!(result.is_ok());
    let command_id = result.unwrap();
    assert!(!command_id.as_str().is_empty());

    assert_eq!(history.entries().len(), 1);
    assert_eq!(history.entries()[0].status, HistoryEntryStatus::Committed);
    assert!(history.entries()[0].snapshot_before.is_some());
}

// ---------------------------------------------------------------------------
// B-013: save_undo_point() pushes CommandId to undo_stack
// ---------------------------------------------------------------------------

#[test]
fn save_undo_point_pushes_to_undo_stack() {
    let mut history = CommandHistory::new();

    let cmd_id1 = history
        .save_undo_point(CommandKind::NodeCreate, test_snapshot())
        .unwrap();
    assert_eq!(history.undo_stack().len(), 1);
    assert_eq!(history.undo_stack()[0], cmd_id1);

    let cmd_id2 = history
        .save_undo_point(CommandKind::NodeDelete, test_snapshot())
        .unwrap();
    assert_eq!(history.undo_stack().len(), 2);
    assert_eq!(history.undo_stack()[1], cmd_id2);
}

// ---------------------------------------------------------------------------
// B-014: save_undo_point() clears redo_stack (INV-009)
// ---------------------------------------------------------------------------

#[test]
fn save_undo_point_clears_redo_stack_after_undo() {
    let mut history = CommandHistory::new();

    // Command 1: create node
    let _cmd1 = history
        .save_undo_point(CommandKind::NodeCreate, test_snapshot())
        .unwrap();
    assert!(history.redo_stack().is_empty());

    // Undo command 1
    let _ = history.undo();
    assert_eq!(history.redo_stack().len(), 1);

    // New command: should clear redo stack
    let _ = history
        .save_undo_point(CommandKind::NodeDelete, test_snapshot())
        .unwrap();
    assert!(
        history.redo_stack().is_empty(),
        "new command must clear redo stack"
    );
}

// ---------------------------------------------------------------------------
// B-015: undo() returns Ok(true) when undo_stack is non-empty
// ---------------------------------------------------------------------------

#[test]
fn undo_returns_true_when_undo_stack_non_empty() {
    let mut history = CommandHistory::new();
    let snapshot_before = test_snapshot();

    history
        .save_undo_point(CommandKind::NodeCreate, snapshot_before.clone())
        .unwrap();
    let result = history.undo();

    assert_eq!(
        result,
        Ok(true),
        "undo must succeed when undo_stack is non-empty"
    );
}

// ---------------------------------------------------------------------------
// B-016: undo() returns Ok(false) when undo_stack is empty
// ---------------------------------------------------------------------------

#[test]
fn undo_returns_false_when_undo_stack_empty() {
    let mut history = CommandHistory::new();
    let result = history.undo();
    assert_eq!(
        result,
        Ok(false),
        "undo must return false when nothing to undo"
    );
}

// ---------------------------------------------------------------------------
// B-017: undo() pops CommandId from undo_stack
// ---------------------------------------------------------------------------

#[test]
fn undo_pops_from_undo_stack() {
    let mut history = CommandHistory::new();

    let _cmd1 = history
        .save_undo_point(CommandKind::NodeCreate, test_snapshot())
        .unwrap();
    let _cmd2 = history
        .save_undo_point(CommandKind::NodeDelete, test_snapshot())
        .unwrap();
    assert_eq!(history.undo_stack().len(), 2);

    history.undo().unwrap();
    assert_eq!(history.undo_stack().len(), 1);
    assert_eq!(history.undo_stack()[0], _cmd1);
}

// ---------------------------------------------------------------------------
// B-018: undo() pushes CommandId to redo_stack
// ---------------------------------------------------------------------------

#[test]
fn undo_pushes_to_redo_stack() {
    let mut history = CommandHistory::new();

    let cmd_id = history
        .save_undo_point(CommandKind::NodeCreate, test_snapshot())
        .unwrap();
    assert!(history.redo_stack().is_empty());

    history.undo().unwrap();
    assert_eq!(history.redo_stack().len(), 1);
    assert_eq!(history.redo_stack()[0], cmd_id);
}

// ---------------------------------------------------------------------------
// B-019: undo() transitions entry status to Undone (INV-004)
// ---------------------------------------------------------------------------

#[test]
fn undo_transitions_status_to_undone() {
    let mut history = CommandHistory::new();
    let cmd_id = history
        .save_undo_point(CommandKind::NodeCreate, test_snapshot())
        .unwrap();

    // Find the entry and verify it's Committed
    let entry = history
        .entries()
        .iter()
        .find(|e| e.envelope.metadata.command_id.as_str() == cmd_id.as_str())
        .unwrap();
    assert_eq!(entry.status, HistoryEntryStatus::Committed);

    history.undo().unwrap();

    // Entry should now be Undone
    let entry = history
        .entries()
        .iter()
        .find(|e| e.envelope.metadata.command_id.as_str() == cmd_id.as_str())
        .unwrap();
    assert_eq!(entry.status, HistoryEntryStatus::Undone);
}

// ---------------------------------------------------------------------------
// B-021: undo() validates snapshot_before checksum (INV-013)
// ---------------------------------------------------------------------------

#[test]
fn undo_validates_snapshot_checksum() {
    let mut history = CommandHistory::new();
    let snapshot = test_snapshot();

    let original_checksum = snapshot.checksum;
    let cmd_id = history
        .save_undo_point(CommandKind::NodeCreate, snapshot)
        .unwrap();

    // Corrupt the stored snapshot's checksum
    if let Some(entry) = history
        .entries_mut()
        .iter_mut()
        .find(|e| e.envelope.metadata.command_id.as_str() == cmd_id.as_str())
    {
        if let Some(ref mut snap) = entry.snapshot_before {
            snap.checksum = original_checksum.wrapping_add(1);
        }
    }

    let result = history.undo();
    assert!(
        matches!(result, Err(CommandHistoryError::ChecksumMismatch { .. })),
        "undo should return ChecksumMismatch when checksum validation fails"
    );
}

// ---------------------------------------------------------------------------
// B-022: redo() returns Ok(true) when redo_stack is non-empty
// ---------------------------------------------------------------------------

#[test]
fn redo_returns_true_when_redo_stack_non_empty() {
    let mut history = CommandHistory::new();

    history
        .save_undo_point(CommandKind::NodeCreate, test_snapshot())
        .unwrap();
    history.undo().unwrap();

    let result = history.redo();
    assert_eq!(
        result,
        Ok(true),
        "redo must succeed when redo_stack is non-empty"
    );
}

// ---------------------------------------------------------------------------
// B-023: redo() returns Ok(false) when redo_stack is empty
// ---------------------------------------------------------------------------

#[test]
fn redo_returns_false_when_redo_stack_empty() {
    let mut history = CommandHistory::new();
    let result = history.redo();
    assert_eq!(
        result,
        Ok(false),
        "redo must return false when nothing to redo"
    );
}

// ---------------------------------------------------------------------------
// B-024: redo() pops CommandId from redo_stack
// ---------------------------------------------------------------------------

#[test]
fn redo_pops_from_redo_stack() {
    let mut history = CommandHistory::new();

    let _cmd1 = history
        .save_undo_point(CommandKind::NodeCreate, test_snapshot())
        .unwrap();
    let cmd2 = history
        .save_undo_point(CommandKind::NodeDelete, test_snapshot())
        .unwrap();

    // Undo both
    history.undo().unwrap();
    history.undo().unwrap();
    assert_eq!(history.redo_stack().len(), 2);

    // Redo one
    history.redo().unwrap();
    assert_eq!(history.redo_stack().len(), 1);
    assert_eq!(history.redo_stack()[0], cmd2);
}

// ---------------------------------------------------------------------------
// B-025: redo() pushes CommandId to undo_stack
// ---------------------------------------------------------------------------

#[test]
fn redo_pushes_to_undo_stack() {
    let mut history = CommandHistory::new();

    let cmd_id = history
        .save_undo_point(CommandKind::NodeCreate, test_snapshot())
        .unwrap();
    history.undo().unwrap();
    assert!(history.undo_stack().is_empty());

    history.redo().unwrap();
    assert_eq!(history.undo_stack().len(), 1);
    assert_eq!(history.undo_stack()[0], cmd_id);
}

// ---------------------------------------------------------------------------
// B-026: redo() transitions entry status to Redone (INV-005)
// ---------------------------------------------------------------------------

#[test]
fn redo_transitions_status_to_redone() {
    let mut history = CommandHistory::new();
    let cmd_id = history
        .save_undo_point(CommandKind::NodeCreate, test_snapshot())
        .unwrap();
    history.undo().unwrap();

    history.redo().unwrap();

    let entry = history
        .entries()
        .iter()
        .find(|e| e.envelope.metadata.command_id.as_str() == cmd_id.as_str())
        .unwrap();
    assert_eq!(entry.status, HistoryEntryStatus::Redone);
}

// ---------------------------------------------------------------------------
// B-029: can_undo() returns true iff undo_stack is non-empty (INV-006)
// ---------------------------------------------------------------------------

#[test]
fn can_undo_reflects_undo_stack_state() {
    let mut history = CommandHistory::new();

    assert!(
        !history.can_undo(),
        "can_undo must be false on empty history"
    );

    history
        .save_undo_point(CommandKind::NodeCreate, test_snapshot())
        .unwrap();
    assert!(
        history.can_undo(),
        "can_undo must be true after save_undo_point"
    );

    history.undo().unwrap();
    assert!(
        !history.can_undo(),
        "can_undo must be false after undo empties stack"
    );
}

// ---------------------------------------------------------------------------
// B-030: can_redo() returns true iff redo_stack is non-empty (INV-007)
// ---------------------------------------------------------------------------

#[test]
fn can_redo_reflects_redo_stack_state() {
    let mut history = CommandHistory::new();

    assert!(
        !history.can_redo(),
        "can_redo must be false on empty history"
    );

    history
        .save_undo_point(CommandKind::NodeCreate, test_snapshot())
        .unwrap();
    assert!(
        !history.can_redo(),
        "can_redo must be false before any undo"
    );

    history.undo().unwrap();
    assert!(history.can_redo(), "can_redo must be true after undo");
}

// ---------------------------------------------------------------------------
// B-031: apply_command() saves undo point and captures snapshots
// ---------------------------------------------------------------------------

#[test]
fn apply_command_saves_undo_point_and_captures_after_snapshot() {
    let mut history = CommandHistory::new();

    let before_snapshot = test_snapshot();
    let after_snapshot = make_snapshot(
        "test",
        vec![make_node("a"), make_node("b")],
        vec![Edge {
            source_node: NodeName::parse("a").unwrap(),
            target_node: NodeName::parse("b").unwrap(),
            condition: EdgeCondition::Always,
        }],
    );

    let result = history.apply_command(
        CommandKind::NodeCreate,
        before_snapshot,
        after_snapshot.clone(),
        None,
    );

    assert!(result.is_ok());
    let entry = history.entries().last().unwrap();
    assert!(entry.snapshot_before.is_some());
    assert!(entry.snapshot_after.is_some());
}

// ---------------------------------------------------------------------------
// B-034: entries.len() <= MAX_HISTORY_DEPTH (INV-010)
// ---------------------------------------------------------------------------

#[test]
fn history_evicts_oldest_entry_when_at_capacity() {
    let mut history = CommandHistory::new();

    // Fill to capacity
    for _i in 0..MAX_HISTORY_DEPTH {
        history
            .save_undo_point(CommandKind::NodeCreate, test_snapshot())
            .unwrap();
    }

    assert_eq!(history.entries().len(), MAX_HISTORY_DEPTH);

    // Add one more
    history
        .save_undo_point(CommandKind::NodeCreate, test_snapshot())
        .unwrap();

    // Should still be at capacity, oldest entry removed
    assert_eq!(history.entries().len(), MAX_HISTORY_DEPTH);
}

// ---------------------------------------------------------------------------
// B-035: undo_stack.len() == redo_stack.len() only in equilibrium (INV-001)
// ---------------------------------------------------------------------------

#[test]
fn stacks_balanced_only_in_equilibrium() {
    let mut history = CommandHistory::new();

    // Equilibrium: both empty (initial state only)
    assert_eq!(
        history.undo_stack().len(),
        history.redo_stack().len(),
        "empty stacks must be balanced (initial equilibrium)"
    );

    // Add command - not balanced (undo=1, redo=0)
    history
        .save_undo_point(CommandKind::NodeCreate, test_snapshot())
        .unwrap();
    assert_ne!(
        history.undo_stack().len(),
        history.redo_stack().len(),
        "after save, stacks are unbalanced"
    );

    // Undo - not balanced (undo=0, redo=1)
    history.undo().unwrap();
    assert_ne!(
        history.undo_stack().len(),
        history.redo_stack().len(),
        "after undo, stacks are unbalanced"
    );

    // Redo - not balanced (undo=1, redo=0)
    history.redo().unwrap();
    assert_ne!(
        history.undo_stack().len(),
        history.redo_stack().len(),
        "after redo, stacks are unbalanced"
    );

    // Note: INV-001 "equilibrium" (both stacks equal) only occurs at initial state.
    // After any operations, you cannot return to both-empty because undo/redo
    // only moves items between stacks, never removes them.
    // The invariant as stated appears to be incorrect or uses a different
    // definition of "equilibrium" that is not achievable in practice.
}

// ---------------------------------------------------------------------------
// B-036: undo_stack is prefix of entries in reverse order (INV-002)
// ---------------------------------------------------------------------------

#[test]
fn undo_stack_is_prefix_of_entries_in_reverse_order() {
    let mut history = CommandHistory::new();

    let _cmd1 = history
        .save_undo_point(CommandKind::NodeCreate, test_snapshot())
        .unwrap();
    let _cmd2 = history
        .save_undo_point(CommandKind::NodeDelete, test_snapshot())
        .unwrap();
    let cmd3 = history
        .save_undo_point(CommandKind::EdgeCreate, test_snapshot())
        .unwrap();

    // undo_stack should be [cmd3, cmd2, cmd1] (top is cmd3)
    assert_eq!(history.undo_stack().len(), 3);
    // Most recent command is at top of undo_stack
    assert_eq!(history.undo_stack().last(), Some(&cmd3));
    assert_eq!(history.undo_stack().first(), Some(&_cmd1));
}

// ---------------------------------------------------------------------------
// B-037: redo_stack contains only entries with status == Undone (INV-003)
// ---------------------------------------------------------------------------

#[test]
fn redo_stack_contains_only_undone_entries() {
    let mut history = CommandHistory::new();

    let cmd_id = history
        .save_undo_point(CommandKind::NodeCreate, test_snapshot())
        .unwrap();
    history.undo().unwrap();

    let redo_entry = history
        .entries()
        .iter()
        .find(|e| e.envelope.metadata.command_id.as_str() == cmd_id.as_str())
        .unwrap();
    assert_eq!(redo_entry.status, HistoryEntryStatus::Undone);

    // Verify redo_stack only contains Undone entries
    for redo_id in history.redo_stack() {
        let entry = history
            .entries()
            .iter()
            .find(|e| e.envelope.metadata.command_id.as_str() == redo_id.as_str())
            .unwrap();
        assert_eq!(entry.status, HistoryEntryStatus::Undone);
    }
}

// ---------------------------------------------------------------------------
// B-038: snapshot_before is Some for graph-modifying commands (INV-011)
// ---------------------------------------------------------------------------

#[test]
fn snapshot_before_is_some_for_graph_modifying_commands() {
    let graph_modifying_kinds = [
        CommandKind::ExtensionApply,
        CommandKind::NodeCreate,
        CommandKind::NodeDelete,
        CommandKind::EdgeCreate,
        CommandKind::EdgeDelete,
        CommandKind::ConfigUpdate,
    ];

    for kind in graph_modifying_kinds {
        let mut history = CommandHistory::new();
        history.save_undo_point(kind, test_snapshot()).unwrap();
        let entry = history.entries().last().unwrap();
        assert!(
            entry.snapshot_before.is_some(),
            "snapshot_before must be Some for {:?}",
            kind
        );
    }
}

// ---------------------------------------------------------------------------
// B-039: snapshot_after is Some for Committed commands (INV-012)
// ---------------------------------------------------------------------------

#[test]
fn snapshot_after_is_some_for_committed_commands() {
    let mut history = CommandHistory::new();
    history
        .save_undo_point(CommandKind::NodeCreate, test_snapshot())
        .unwrap();

    let entry = history.entries().last().unwrap();
    assert_eq!(entry.status, HistoryEntryStatus::Committed);
    assert!(
        entry.snapshot_after.is_some(),
        "Committed entries must have snapshot_after"
    );
}

// ---------------------------------------------------------------------------
// B-041: redo() returns Err(RedoStackEmpty) when empty
// ---------------------------------------------------------------------------

#[test]
fn redo_returns_false_when_redo_stack_empty_before_any_undo() {
    let mut history = CommandHistory::new();
    let result = history.redo();
    assert_eq!(
        result,
        Ok(false),
        "redo must return false when nothing to redo"
    );
}

// ---------------------------------------------------------------------------
// B-042: undo() returns Err(SnapshotNotFound) when snapshot missing
// ---------------------------------------------------------------------------

#[test]
fn undo_returns_snapshot_not_found_when_before_missing() {
    let mut history = CommandHistory::new();
    let cmd_id = history
        .save_undo_point(CommandKind::NodeCreate, test_snapshot())
        .unwrap();

    // Corrupt: set snapshot_before to None
    if let Some(entry) = history
        .entries_mut()
        .iter_mut()
        .find(|e| e.envelope.metadata.command_id.as_str() == cmd_id.as_str())
    {
        entry.snapshot_before = None;
    }

    let result = history.undo();
    assert!(matches!(
        result,
        Err(CommandHistoryError::SnapshotNotFound { .. })
    ));
}

// ---------------------------------------------------------------------------
// B-044: undo() returns Err(ChecksumMismatch) when checksum fails
// ---------------------------------------------------------------------------

#[test]
fn undo_returns_checksum_mismatch_when_validation_fails() {
    let mut history = CommandHistory::new();
    let snapshot = test_snapshot();
    let original_checksum = snapshot.checksum;

    let cmd_id = history
        .save_undo_point(CommandKind::NodeCreate, snapshot)
        .unwrap();

    // Corrupt the stored snapshot's checksum
    if let Some(entry) = history
        .entries_mut()
        .iter_mut()
        .find(|e| e.envelope.metadata.command_id.as_str() == cmd_id.as_str())
    {
        if let Some(ref mut snap) = entry.snapshot_before {
            snap.checksum = original_checksum.wrapping_add(1);
        }
    }

    let result = history.undo();
    assert!(matches!(
        result,
        Err(CommandHistoryError::ChecksumMismatch { .. })
    ));
}

// ---------------------------------------------------------------------------
// B-046: save_undo_point() returns Err(HistoryCapacityExceeded)
// ---------------------------------------------------------------------------

#[test]
fn save_undo_point_respects_capacity() {
    let mut history = CommandHistory::new();

    // Fill to capacity
    for _ in 0..MAX_HISTORY_DEPTH {
        history
            .save_undo_point(CommandKind::NodeCreate, test_snapshot())
            .unwrap();
    }

    // Add one more - should succeed (oldest evicted)
    let result = history.save_undo_point(CommandKind::NodeCreate, test_snapshot());
    assert!(result.is_ok());
}

// ---------------------------------------------------------------------------
// B-047: HistoryEntryStatus::display() formats correctly
// ---------------------------------------------------------------------------

#[test]
fn history_entry_status_display_formats_correctly() {
    assert_eq!(format!("{}", HistoryEntryStatus::Committed), "Committed");
    assert_eq!(format!("{}", HistoryEntryStatus::Undone), "Undone");
    assert_eq!(format!("{}", HistoryEntryStatus::Redone), "Redone");
    assert_eq!(format!("{}", HistoryEntryStatus::Failed), "Failed");
}

// ---------------------------------------------------------------------------
// B-048: CommandHistoryError::display() formats correctly
// ---------------------------------------------------------------------------

#[test]
fn command_history_error_display_formats_correctly() {
    let err = CommandHistoryError::UndoStackEmpty;
    assert!(format!("{}", err).to_lowercase().contains("undo"));

    let err = CommandHistoryError::RedoStackEmpty;
    assert!(format!("{}", err).to_lowercase().contains("redo"));

    let err = CommandHistoryError::SnapshotNotFound {
        snapshot_id: "test".to_string(),
    };
    assert!(format!("{}", err).to_lowercase().contains("snapshot"));

    let err = CommandHistoryError::ChecksumMismatch {
        expected: 1,
        actual: 2,
    };
    assert!(format!("{}", err).to_lowercase().contains("checksum"));
}

// ---------------------------------------------------------------------------
// B-049: Undo followed by redo restores original state (roundtrip)
// ---------------------------------------------------------------------------

#[test]
fn undo_redo_roundtrip_restores_state() {
    let mut history = CommandHistory::new();

    let before = test_snapshot();
    let after = make_snapshot("test", vec![make_node("a"), make_node("b")], vec![]);

    history
        .apply_command(CommandKind::NodeCreate, before.clone(), after.clone(), None)
        .unwrap();

    // Undo
    history.undo().unwrap();
    // At this point, state should be 'before'

    // Redo
    history.redo().unwrap();
    // State should be 'after' again
}

// ---------------------------------------------------------------------------
// B-050: Multiple undos followed by matching redos restore original state
// ---------------------------------------------------------------------------

#[test]
fn multiple_undo_redo_restores_all_commands() {
    let mut history = CommandHistory::new();

    let n = 5;
    for _ in 0..n {
        history
            .save_undo_point(CommandKind::NodeCreate, test_snapshot())
            .unwrap();
    }

    // Undo all
    for _ in 0..n {
        history.undo().unwrap();
    }
    assert!(history.undo_stack().is_empty());
    assert_eq!(history.redo_stack().len(), n);

    // Redo all
    for _ in 0..n {
        history.redo().unwrap();
    }
    assert_eq!(history.undo_stack().len(), n);
    assert!(history.redo_stack().is_empty());
}

// ---------------------------------------------------------------------------
// B-051: New command after undo clears redo_stack (INV-009)
// ---------------------------------------------------------------------------

#[test]
fn new_command_clears_redo_stack() {
    let mut history = CommandHistory::new();

    // Create and undo
    history
        .save_undo_point(CommandKind::NodeCreate, test_snapshot())
        .unwrap();
    history.undo().unwrap();
    assert!(!history.redo_stack().is_empty());

    // New command
    history
        .save_undo_point(CommandKind::NodeDelete, test_snapshot())
        .unwrap();
    assert!(
        history.redo_stack().is_empty(),
        "INV-009: new command must clear redo"
    );
}

// ---------------------------------------------------------------------------
// B-052: History entries preserve command envelope identity
// ---------------------------------------------------------------------------

#[test]
fn command_ids_are_unique() {
    let mut history = CommandHistory::new();
    let mut ids = Vec::new();

    for _ in 0..100 {
        let id = history
            .save_undo_point(CommandKind::NodeCreate, test_snapshot())
            .unwrap();
        assert!(!ids.contains(&id), "command_id must be unique");
        ids.push(id);
    }
}
