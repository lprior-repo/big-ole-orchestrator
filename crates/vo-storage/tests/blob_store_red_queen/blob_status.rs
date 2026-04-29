use crate::helpers::make_blob_ref;
use vo_types::BlobStatus;

#[test]
fn red_queen_blob_status_pending_is_initial() {
    let all_statuses = BlobStatus::all_variants();
    assert!(
        all_statuses.contains(&BlobStatus::Pending),
        "Pending must be a valid status"
    );
}

#[test]
fn red_queen_blob_status_transitions_are_valid() {
    assert!(
        BlobStatus::Pending.can_transition_to(BlobStatus::DurablyStored),
        "Pending → DurablyStored must be valid"
    );
    assert!(
        BlobStatus::Pending.can_transition_to(BlobStatus::Failed),
        "Pending → Failed must be valid"
    );
    assert!(
        BlobStatus::DurablyStored.can_transition_to(BlobStatus::Published),
        "DurablyStored → Published must be valid"
    );
}

#[test]
fn red_queen_blob_status_invalid_transitions_rejected() {
    assert!(
        !BlobStatus::Pending.can_transition_to(BlobStatus::Published),
        "Pending cannot skip to Published"
    );
    assert!(
        !BlobStatus::Published.can_transition_to(BlobStatus::Pending),
        "Published cannot revert to Pending"
    );
    assert!(
        !BlobStatus::Published.can_transition_to(BlobStatus::Failed),
        "Published cannot transition to Failed"
    );
    assert!(
        !BlobStatus::Published.can_transition_to(BlobStatus::DurablyStored),
        "Published cannot transition to DurablyStored"
    );
    assert!(
        !BlobStatus::Failed.can_transition_to(BlobStatus::Pending),
        "Failed cannot revert to Pending"
    );
    assert!(
        !BlobStatus::Failed.can_transition_to(BlobStatus::DurablyStored),
        "Failed cannot transition to DurablyStored"
    );
    assert!(
        !BlobStatus::Failed.can_transition_to(BlobStatus::Published),
        "Failed cannot transition to Published"
    );
    assert!(
        !BlobStatus::DurablyStored.can_transition_to(BlobStatus::Pending),
        "DurablyStored cannot revert to Pending"
    );
    assert!(
        !BlobStatus::DurablyStored.can_transition_to(BlobStatus::Failed),
        "DurablyStored cannot transition to Failed"
    );
}

#[test]
fn red_queen_blob_status_terminal_states_are_truly_terminal() {
    for status in BlobStatus::all_variants() {
        assert!(
            !BlobStatus::Published.can_transition_to(*status),
            "Published must be terminal"
        );
        assert!(
            !BlobStatus::Failed.can_transition_to(*status),
            "Failed must be terminal"
        );
    }
}

#[test]
fn red_queen_blob_status_all_variants_count_is_four() {
    let variants = BlobStatus::all_variants();
    assert_eq!(variants.len(), 4, "Must have exactly 4 status variants");
}