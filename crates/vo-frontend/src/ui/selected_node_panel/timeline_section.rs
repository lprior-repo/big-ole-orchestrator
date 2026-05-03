#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]

use crate::flow_extender::ExtensionPatchPreview;
use crate::ui::graph::Workflow;
use dioxus::prelude::*;

use super::types::{
    event_appearance, mode_label, push_timeline, snapshot_by_id, ExtensionBatchSnapshot,
    ExtensionTimelineEvent, ExtensionTimelineEventKind, ExtensionTimelineMetadata,
};

#[component]
pub(crate) fn TimelineSection(
    extension_timeline: Signal<Vec<ExtensionTimelineEvent>>,
    extension_snapshots: Signal<Vec<ExtensionBatchSnapshot>>,
    workflow: Signal<Workflow>,
    workflow_state: crate::hooks::use_workflow_state::WorkflowState,
    preview_patches: Signal<Vec<ExtensionPatchPreview>>,
    extension_message: Signal<Option<String>>,
) -> Element {
    rsx! {
        div { class: "mt-3 rounded-lg border border-slate-200 bg-slate-50/80 p-2.5",
            div { class: "mb-2 flex items-center justify-between",
                h5 { class: "text-[10px] font-semibold uppercase tracking-wide text-slate-600", "Extension Timeline" }
                span { class: "rounded bg-white px-1.5 py-0.5 text-[10px] text-slate-500", "{extension_timeline.read().len()}" }
            }
            if extension_timeline.read().is_empty() {
                p { class: "text-[10px] text-slate-500", "No extension operations yet." }
            } else {
                div { class: "flex flex-col gap-1.5",
                    for (idx, event) in extension_timeline.read().iter().enumerate() {
                        {
                            let (dot_class, label_class, label) = event_appearance(event.kind);
                            let metadata = event.metadata.clone();
                            rsx! {
                                div {
                                    key: "timeline-{idx}",
                                    class: "flex gap-2 rounded-md border border-slate-200 bg-white px-2 py-1.5",
                                    div { class: "flex flex-col items-center",
                                        span { class: "mt-[2px] h-2 w-2 rounded-full {dot_class}" }
                                        span { class: "mt-1 text-[9px] font-mono text-slate-400", "#{event.id}" }
                                    }
                                    div { class: "min-w-0",
                                        p { class: "text-[10px] leading-relaxed text-slate-700", "{event.message}" }
                                        if let Some(meta) = metadata.clone() {
                                            div { class: "mt-0.5 flex flex-wrap items-center gap-1 text-[9px] text-slate-500",
                                                span { class: "rounded bg-slate-100 px-1.5 py-0.5 font-mono", "B#{meta.batch_id}" }
                                                span { class: "rounded bg-slate-100 px-1.5 py-0.5 font-mono", "S#{meta.snapshot_id}" }
                                                span { class: "rounded bg-slate-100 px-1.5 py-0.5", "{mode_label(meta.mode)}" }
                                            }
                                        }
                                        span { class: "mt-0.5 inline-flex rounded px-1.5 py-0.5 text-[9px] font-medium {label_class}", "{label}" }
                                        if matches!(event.kind, ExtensionTimelineEventKind::Snapshot) {
                                            if let Some(meta) = metadata {
                                                button {
                                                    class: "mt-1 inline-flex h-5 items-center rounded border border-cyan-300 bg-cyan-50 px-1.5 text-[9px] font-medium text-cyan-700 transition-colors hover:bg-cyan-100",
                                                    onclick: {
                                                        let meta = meta.clone();
                                                        let ws = workflow_state.clone();
                                                        move |event| {
                                                        event.stop_propagation();
                                                        if let Some(snapshot) = snapshot_by_id(
                                                            &extension_snapshots.read(),
                                                            meta.snapshot_id,
                                                        ) {
                                                            ws.save_undo_point();
                                                            workflow.set(snapshot.workflow_before.clone());
                                                            let detail = format!(
                                                                "Rolled back to snapshot #{} from batch #{} ({} keys, {} node(s)).",
                                                                snapshot.snapshot_id,
                                                                snapshot.batch_id,
                                                                snapshot.keys.len(),
                                                                snapshot.created_nodes
                                                            );
                                                            let history = extension_timeline.read().clone();
                                                            extension_timeline.set(push_timeline(
                                                                history,
                                                                ExtensionTimelineEventKind::RolledBack,
                                                                detail.clone(),
                                                                Some(ExtensionTimelineMetadata {
                                                                    batch_id: snapshot.batch_id,
                                                                    snapshot_id: snapshot.snapshot_id,
                                                                    mode: snapshot.mode,
                                                                }),
                                                            ));
                                                            extension_message.set(Some(detail));
                                                            preview_patches.set(Vec::new());
                                                        }
                                                    }
                                                    },
                                                    "Rollback"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
