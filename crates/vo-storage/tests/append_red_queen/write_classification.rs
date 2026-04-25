//! DIMENSION: write_classification
//! ADR-016: WriteClass determines priority and durability guarantees

#![allow(clippy::unwrap_used)]

use vo_storage::append::{
    AppendEntry, BlobWrite, ControlPlaneWrite, ProjectionWrite, WriteClass,
};

use super::helpers::make_event;

#[test]
fn red_queen_write_class_tier_ordering() {
    assert!(
        WriteClass::CriticalControlPlane.tier() < WriteClass::OperatorProjection.tier(),
        "Critical tier (1) must be less than Projection tier (2)"
    );
    assert!(
        WriteClass::OperatorProjection.tier() < WriteClass::BulkBlob.tier(),
        "Projection tier (2) must be less than Blob tier (3)"
    );
}

#[test]
fn red_queen_write_class_never_drops() {
    assert!(
        WriteClass::CriticalControlPlane.never_drops(),
        "CriticalControlPlane writes must never be dropped"
    );
    assert!(
        !WriteClass::OperatorProjection.never_drops(),
        "OperatorProjection writes may be dropped under pressure"
    );
    assert!(
        !WriteClass::BulkBlob.never_drops(),
        "BulkBlob writes may be dropped under pressure"
    );
}

#[test]
fn red_queen_write_class_classification() {
    let event = make_event("test", 1);

    let cp_write = ControlPlaneWrite::new(event.clone(), 100);
    assert_eq!(
        cp_write.write_class(),
        WriteClass::CriticalControlPlane,
        "ControlPlaneWrite must classify as CriticalControlPlane"
    );

    let proj_write = ProjectionWrite::new("test".to_string(), 100);
    assert_eq!(
        proj_write.write_class(),
        WriteClass::OperatorProjection,
        "ProjectionWrite must classify as OperatorProjection"
    );

    let blob_write = BlobWrite::bulk("test".to_string(), 100);
    assert_eq!(
        blob_write.write_class(),
        WriteClass::BulkBlob,
        "BlobWrite::bulk must classify as BulkBlob"
    );
}

#[test]
fn red_queen_append_entry_classification() {
    let event = make_event("test", 1);

    let cp_entry = AppendEntry::ControlPlane(ControlPlaneWrite::new(event.clone(), 100));
    assert_eq!(cp_entry.write_class(), WriteClass::CriticalControlPlane);

    let proj_entry = AppendEntry::Projection(ProjectionWrite::new("test".to_string(), 100));
    assert_eq!(proj_entry.write_class(), WriteClass::OperatorProjection);

    let blob_entry = AppendEntry::Blob(BlobWrite::bulk("test".to_string(), 100));
    assert_eq!(blob_entry.write_class(), WriteClass::BulkBlob);
}
