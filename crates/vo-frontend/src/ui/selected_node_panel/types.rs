#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]

use dioxus::prelude::*;
use itertools::Itertools;
use oya_frontend::flow_extender::{preview_extension, ExtensionPatchPreview};
use oya_frontend::graph::{NodeId, Workflow};

// ---------------------------------------------------------------------------
// Extension timeline types
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub(crate) enum ExtensionTimelineEventKind {
    Snapshot,
    Applied,
    Failed,
    Undone,
    Redone,
    RolledBack,
}

#[derive(Clone)]
pub(crate) struct ExtensionTimelineEvent {
    pub id: usize,
    pub kind: ExtensionTimelineEventKind,
    pub message: String,
    pub metadata: Option<ExtensionTimelineMetadata>,
}

#[derive(Clone)]
pub(crate) struct ExtensionTimelineMetadata {
    pub batch_id: usize,
    pub snapshot_id: usize,
    pub mode: ExtensionApplyMode,
}

#[derive(Clone, Copy)]
pub(crate) enum ExtensionApplyMode {
    Single,
    Bulk,
}

#[derive(Clone)]
pub(crate) struct ExtensionBatchSnapshot {
    pub batch_id: usize,
    pub snapshot_id: usize,
    pub mode: ExtensionApplyMode,
    pub keys: Vec<String>,
    pub created_nodes: usize,
    pub workflow_before: Workflow,
}

// ---------------------------------------------------------------------------
// Pure helpers – timeline
// ---------------------------------------------------------------------------

pub(crate) fn push_timeline(
    timeline: Vec<ExtensionTimelineEvent>,
    kind: ExtensionTimelineEventKind,
    message: String,
    metadata: Option<ExtensionTimelineMetadata>,
) -> Vec<ExtensionTimelineEvent> {
    let next_id = timeline.first().map_or(1, |entry| entry.id + 1);
    let mut new_timeline = vec![ExtensionTimelineEvent {
        id: next_id,
        kind,
        message,
        metadata,
    }];
    new_timeline.extend(timeline.into_iter().take(11));
    new_timeline
}

pub(crate) fn event_appearance(
    kind: ExtensionTimelineEventKind,
) -> (&'static str, &'static str, &'static str) {
    match kind {
        ExtensionTimelineEventKind::Snapshot => {
            ("bg-slate-500", "bg-slate-100 text-slate-700", "Snapshot")
        }
        ExtensionTimelineEventKind::Applied => (
            "bg-emerald-500",
            "bg-emerald-100 text-emerald-700",
            "Applied",
        ),
        ExtensionTimelineEventKind::Failed => ("bg-red-500", "bg-red-100 text-red-700", "Failed"),
        ExtensionTimelineEventKind::Undone => {
            ("bg-amber-500", "bg-amber-100 text-amber-700", "Undo")
        }
        ExtensionTimelineEventKind::Redone => {
            ("bg-indigo-500", "bg-indigo-100 text-indigo-700", "Redo")
        }
        ExtensionTimelineEventKind::RolledBack => {
            ("bg-cyan-500", "bg-cyan-100 text-cyan-700", "Rollback")
        }
    }
}

pub(crate) fn mode_label(mode: ExtensionApplyMode) -> &'static str {
    match mode {
        ExtensionApplyMode::Single => "single",
        ExtensionApplyMode::Bulk => "bulk",
    }
}

// ---------------------------------------------------------------------------
// Pure helpers – snapshots
// ---------------------------------------------------------------------------

pub(crate) fn remember_extension_snapshot(
    snapshots: Vec<ExtensionBatchSnapshot>,
    mode: ExtensionApplyMode,
    keys: Vec<String>,
    created_nodes: usize,
    workflow_before: Workflow,
) -> (Vec<ExtensionBatchSnapshot>, ExtensionTimelineMetadata) {
    let next_snapshot_id = snapshots.first().map_or(1, |entry| entry.snapshot_id + 1);
    let next_batch_id = snapshots.first().map_or(1, |entry| entry.batch_id + 1);
    let snapshot = ExtensionBatchSnapshot {
        batch_id: next_batch_id,
        snapshot_id: next_snapshot_id,
        mode,
        keys,
        created_nodes,
        workflow_before,
    };
    let mut new_snapshots = vec![snapshot];
    new_snapshots.extend(snapshots.into_iter().take(23));

    (
        new_snapshots,
        ExtensionTimelineMetadata {
            batch_id: next_batch_id,
            snapshot_id: next_snapshot_id,
            mode,
        },
    )
}

pub(crate) fn snapshot_by_id(
    snapshots: &[ExtensionBatchSnapshot],
    snapshot_id: usize,
) -> Option<ExtensionBatchSnapshot> {
    snapshots
        .iter()
        .find(|entry| entry.snapshot_id == snapshot_id)
        .cloned()
}

// ---------------------------------------------------------------------------
// Pure helpers – previews & payloads
// ---------------------------------------------------------------------------

pub(crate) fn collect_previews(workflow: &Workflow, keys: &[String]) -> Vec<ExtensionPatchPreview> {
    keys.iter()
        .unique()
        .filter_map(|key| preview_extension(workflow, key).ok().flatten())
        .collect::<Vec<_>>()
}

pub(crate) fn collect_input_payloads(
    workflow: &Workflow,
    node_id: NodeId,
) -> Vec<serde_json::Value> {
    workflow
        .connections
        .iter()
        .filter(|edge| edge.target == node_id)
        .filter_map(|edge| {
            workflow
                .nodes
                .iter()
                .find(|node| node.id == edge.source)
                .and_then(|node| node.last_output.clone())
        })
        .collect::<Vec<_>>()
}

// ---------------------------------------------------------------------------
// Platform-conditional metrics recording
// ---------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn record_suggestion_decision(key: &str, accepted: bool, source: &str) {
    use chrono::Utc;
    use oya_frontend::metrics::{SuggestionDecision, SuggestionDecisionMetrics};
    use oya_frontend::MetricsStore;
    use std::path::Path;

    let decision = if accepted {
        SuggestionDecision::Accepted
    } else {
        SuggestionDecision::Rejected
    };
    let metrics = SuggestionDecisionMetrics {
        timestamp: Utc::now(),
        suggestion_key: key.to_string(),
        decision,
        source: source.to_string(),
    };

    let store = MetricsStore::new(Path::new("."));
    store.record_suggestion_decision(metrics).unwrap();
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn record_suggestion_decision(_key: &str, _accepted: bool, _source: &str) {}
