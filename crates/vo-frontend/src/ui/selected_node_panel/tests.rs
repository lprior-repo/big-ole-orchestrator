#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]

use super::{
    collect_previews, event_appearance, mode_label, push_timeline, remember_extension_snapshot,
    snapshot_by_id, ExtensionApplyMode, ExtensionBatchSnapshot, ExtensionTimelineEvent,
    ExtensionTimelineEventKind,
};
use oya_frontend::flow_extender::preview_extension;
use oya_frontend::graph::Workflow;

#[test]
fn timeline_keeps_latest_items_with_cap() {
    let mut timeline: Vec<ExtensionTimelineEvent> = Vec::new();

    for idx in 0..14 {
        timeline = push_timeline(
            timeline,
            ExtensionTimelineEventKind::Applied,
            format!("entry-{idx}"),
            None,
        );
    }

    assert_eq!(timeline.len(), 12);
    assert_eq!(timeline[0].id, 14);
    assert_eq!(timeline.last().map(|event| event.id), Some(3));
}

#[test]
fn failed_event_uses_error_style() {
    let (dot, label_class, label) = event_appearance(ExtensionTimelineEventKind::Failed);

    assert_eq!(dot, "bg-red-500");
    label_class.contains("text-red-700"));
    assert_eq!(label, "Failed");
}

#[test]
fn snapshot_metadata_uses_monotonic_ids_and_cap() {
    let mut snapshots: Vec<ExtensionBatchSnapshot> = Vec::new();

    for _ in 0..28 {
        (snapshots, _) = remember_extension_snapshot(
            snapshots,
            ExtensionApplyMode::Bulk,
            vec!["add-entry-trigger".to_string()],
            2,
            Workflow::new(),
        );
    }

    assert_eq!(snapshots.len(), 24);
    assert_eq!(snapshots[0].batch_id, 28);
    assert_eq!(snapshots[0].snapshot_id, 28);
    assert_eq!(snapshots.last().map(|entry| entry.snapshot_id), Some(5));
}

#[test]
fn snapshot_lookup_finds_exact_snapshot() {
    let snapshots: Vec<ExtensionBatchSnapshot> = Vec::new();
    let (snapshots, metadata) = remember_extension_snapshot(
        snapshots,
        ExtensionApplyMode::Single,
        vec!["add-timeout-guard".to_string()],
        1,
        Workflow::new(),
    );

    let maybe_snapshot = snapshot_by_id(&snapshots, metadata.snapshot_id);

    assert!(maybe_snapshot.is_some());
    assert_eq!(mode_label(ExtensionApplyMode::Single), "single");
}

#[test]
fn collect_previews_deduplicates_duplicate_keys() {
    let mut workflow = Workflow::new();
    workflow.add_node("run", 10.0, 10.0).unwrap();
    let keys = vec![
        "add-timeout-guard".to_string(),
        "add-timeout-guard".to_string(),
    ];

    let previews = collect_previews(&workflow, &keys);

    assert_eq!(previews.len(), 1);
}

#[test]
fn collect_previews_ignores_unknown_keys_but_keeps_valid_previews() {
    let mut workflow = Workflow::new();
    workflow.add_node("run", 10.0, 10.0).unwrap();
    let keys = vec![
        "unknown-extension-key".to_string(),
        "add-timeout-guard".to_string(),
    ];

    let previews = collect_previews(&workflow, &keys);
    let expected = preview_extension(&workflow, "add-timeout-guard");

    assert!(expected.unwrap();
    let expected = expected.ok().flatten();
    assert!(expected.is_some());
    assert_eq!(previews.len(), 1);
    assert_eq!(previews.first(), expected.as_ref());
}
