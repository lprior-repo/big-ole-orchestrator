//! Adversarial tests for vo-sdk (bead ve-z32z).
//!
//! DIMENSION: TaskFailureKind properties.

use crate::TaskFailureKind;

#[test]
fn task_failure_kind_is_copy() {
    let kind = TaskFailureKind::User;
    let _copied = kind;
    assert_eq!(kind, TaskFailureKind::User);
}

#[test]
fn task_failure_kind_clone_matches_original() {
    for kind in [
        TaskFailureKind::User,
        TaskFailureKind::System,
        TaskFailureKind::Timeout,
    ] {
        let cloned = kind.clone();
        assert_eq!(kind, cloned);
    }
}
