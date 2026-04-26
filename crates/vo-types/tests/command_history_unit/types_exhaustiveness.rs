//! Type exhaustiveness tests: CommandKind, ExtensionApplyMode, HistoryEntryStatus.
//!
//! Behaviors: B-001, B-007, B-009

use super::*;

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
