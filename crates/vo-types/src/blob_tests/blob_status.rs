use crate::{BlobStatus, ParseError, INLINED_MAX_BYTES};
use rstest::rstest;

#[test]
fn pending_can_transition_to_durably_stored() {
    assert!(BlobStatus::Pending.can_transition_to(BlobStatus::DurablyStored));
}

#[test]
fn pending_can_transition_to_failed() {
    assert!(BlobStatus::Pending.can_transition_to(BlobStatus::Failed));
}

#[test]
fn pending_cannot_skip_to_published() {
    assert!(!BlobStatus::Pending.can_transition_to(BlobStatus::Published));
}

#[test]
fn pending_cannot_transition_to_itself() {
    assert!(!BlobStatus::Pending.can_transition_to(BlobStatus::Pending));
}

#[test]
fn durably_stored_can_transition_to_published() {
    assert!(BlobStatus::DurablyStored.can_transition_to(BlobStatus::Published));
}

#[test]
fn durably_stored_cannot_revert_to_pending() {
    assert!(!BlobStatus::DurablyStored.can_transition_to(BlobStatus::Pending));
}

#[test]
fn durably_stored_cannot_transition_to_itself() {
    assert!(!BlobStatus::DurablyStored.can_transition_to(BlobStatus::DurablyStored));
}

#[test]
fn durably_stored_cannot_transition_to_failed() {
    assert!(!BlobStatus::DurablyStored.can_transition_to(BlobStatus::Failed));
}

#[test]
fn published_is_terminal_state() {
    let variants = BlobStatus::all_variants();
    for &target in variants {
        assert!(
            !BlobStatus::Published.can_transition_to(target),
            "Published should not transition to {:?}",
            target
        );
    }
}

#[test]
fn failed_is_terminal_state() {
    let variants = BlobStatus::all_variants();
    for &target in variants {
        assert!(
            !BlobStatus::Failed.can_transition_to(target),
            "Failed should not transition to {:?}",
            target
        );
    }
}

#[test]
fn blob_status_all_variants_returns_four_in_declared_order() {
    let variants = BlobStatus::all_variants();
    assert_eq!(variants.len(), 4);
    assert_eq!(
        variants,
        &[
            BlobStatus::Pending,
            BlobStatus::DurablyStored,
            BlobStatus::Published,
            BlobStatus::Failed,
        ]
    );
}

#[rstest]
#[case(BlobStatus::Pending)]
#[case(BlobStatus::DurablyStored)]
#[case(BlobStatus::Published)]
#[case(BlobStatus::Failed)]
fn blob_status_serde_roundtrips(#[case] status: BlobStatus) {
    let json_str = serde_json::to_string(&status).expect("serialize");
    let recovered: BlobStatus = serde_json::from_str(&json_str).expect("deserialize");
    assert_eq!(status, recovered);
}

#[test]
fn blob_status_equality_works() {
    assert_eq!(BlobStatus::Pending, BlobStatus::Pending);
    assert_eq!(BlobStatus::Published, BlobStatus::Published);
    assert_ne!(BlobStatus::Pending, BlobStatus::Published);
}