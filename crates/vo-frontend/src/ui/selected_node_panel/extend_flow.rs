#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]

use crate::flow_extender::{
    apply_extension, extension_presets, suggest_extensions, ExtensionPatchPreview,
    ExtensionPriority,
};
use crate::ui::graph::{NodeId, Workflow};
use dioxus::prelude::*;

use super::preset_card::PresetCard;
use super::suggestion_card::SuggestionCard;
use super::timeline_section::TimelineSection;
use super::types::{
    mode_label, push_timeline, record_suggestion_decision, remember_extension_snapshot,
    ExtensionApplyMode, ExtensionBatchSnapshot, ExtensionTimelineEvent, ExtensionTimelineEventKind,
};

#[component]
pub(crate) fn ExtendFlowSection(
    node_id: NodeId,
    workflow: Signal<Workflow>,
    workflow_state: crate::hooks::use_workflow_state::WorkflowState,
    selected_extension_keys: Signal<Vec<String>>,
    extension_message: Signal<Option<String>>,
    extension_timeline: Signal<Vec<ExtensionTimelineEvent>>,
    extension_snapshots: Signal<Vec<ExtensionBatchSnapshot>>,
    preview_patches: Signal<Vec<ExtensionPatchPreview>>,
) -> Element {
    let suggestions = suggest_extensions(&workflow.read());
    let presets = extension_presets();
    let suggestions_for_all = suggestions.clone();
    let suggestions_for_high = suggestions.clone();
    let selected_count = selected_extension_keys.read().len();
    let can_undo = workflow_state.can_undo();
    let can_redo = workflow_state.can_redo();

    rsx! {
        div { class: "mt-5 border-t border-slate-200 pt-4",
            div { class: "mb-3 flex items-center justify-between",
                h4 { class: "text-[11px] font-semibold uppercase tracking-wide text-slate-600", "Extend Flow" }
                span { class: "rounded bg-slate-100 px-2 py-0.5 text-[10px] text-slate-600", "{suggestions.len()}" }
            }

            div { class: "mb-2 flex flex-wrap items-center gap-1.5",
                button {
                    class: "h-7 rounded-md border border-slate-300 bg-white px-2.5 text-[10px] font-medium text-slate-700 transition-colors hover:bg-slate-100",
                    onclick: move |_| {
                        let all = suggestions_for_all
                            .iter()
                            .map(|entry| entry.key.clone())
                            .collect::<Vec<_>>();
                        selected_extension_keys.set(all);
                    },
                    "Select all"
                }
                button {
                    class: "h-7 rounded-md border border-slate-300 bg-white px-2.5 text-[10px] font-medium text-slate-700 transition-colors hover:bg-slate-100",
                    onclick: move |_| {
                        let high_priority = suggestions_for_high
                            .iter()
                            .filter(|entry| matches!(entry.priority, ExtensionPriority::High))
                            .map(|entry| entry.key.clone())
                            .collect::<Vec<_>>();
                        selected_extension_keys.set(high_priority);
                    },
                    "Select high"
                }
                button {
                    class: "h-7 rounded-md border border-slate-300 bg-white px-2.5 text-[10px] font-medium text-slate-700 transition-colors hover:bg-slate-100 disabled:cursor-not-allowed disabled:opacity-45",
                    disabled: !can_undo,
                    onclick: move |_| {
                        if workflow_state.undo() {
                            let history = extension_timeline.read().clone();
                            extension_timeline.set(push_timeline(
                                history,
                                ExtensionTimelineEventKind::Undone,
                                "Extension changes reverted via undo.".to_string(),
                                None,
                            ));
                            extension_message.set(Some("Undid most recent graph change.".to_string()));
                        }
                    },
                    "Undo"
                }
                button {
                    class: "h-7 rounded-md border border-slate-300 bg-white px-2.5 text-[10px] font-medium text-slate-700 transition-colors hover:bg-slate-100 disabled:cursor-not-allowed disabled:opacity-45",
                    disabled: !can_redo,
                    onclick: move |_| {
                        if workflow_state.redo() {
                            let history = extension_timeline.read().clone();
                            extension_timeline.set(push_timeline(
                                history,
                                ExtensionTimelineEventKind::Redone,
                                "Extension changes restored via redo.".to_string(),
                                None,
                            ));
                            extension_message.set(Some("Redid most recent graph change.".to_string()));
                        }
                    },
                    "Redo"
                }
            }

            if !presets.is_empty() {
                div { class: "mb-3 rounded-lg border border-slate-200 bg-slate-50/80 p-2.5",
                    div { class: "mb-2 flex items-center justify-between",
                        h5 { class: "text-[10px] font-semibold uppercase tracking-wide text-slate-600", "Presets" }
                        span { class: "rounded bg-white px-1.5 py-0.5 text-[10px] text-slate-500", "{presets.len()}" }
                    }
                    div { class: "flex flex-col gap-2",
                        for preset in presets {
                            {
                                let wf = workflow;
                                let ws = workflow_state;
                                let sek = selected_extension_keys;
                                let em = extension_message;
                                let et = extension_timeline;
                                let es = extension_snapshots;
                                let pp = preview_patches;
                                rsx! {
                                    PresetCard {
                                        preset,
                                        workflow: wf,
                                        workflow_state: ws,
                                        selected_extension_keys: sek,
                                        extension_message: em,
                                        extension_timeline: et,
                                        extension_snapshots: es,
                                        preview_patches: pp,
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if selected_count > 0 {
                div { class: "mb-2 flex items-center gap-2",
                    button {
                        class: "h-7 rounded-md border border-blue-300 bg-blue-50 px-2.5 text-[11px] font-medium text-blue-700 transition-colors hover:bg-blue-100",
                        onclick: move |_| {
                            let keys = selected_extension_keys.read().clone();
                            if keys.is_empty() {
                                extension_message.set(Some("Select at least one extension to apply.".to_string()));
                                return;
                            }

                            let workflow_before = workflow.read().clone();
                            workflow_state.save_undo_point();

                            let mut total_created = 0usize;
                            let mut applied_count = 0usize;
                            let mut failures = Vec::new();
                            {
                                let mut wf = workflow.write();
                                for key in &keys {
                                                     match apply_extension(&mut wf, key) {
                                                         Ok(applied) => {
                                                             total_created += applied.created_nodes.len();
                                                             applied_count += 1;
                                                             record_suggestion_decision(
                                                                 key,
                                                                 true,
                                                                 "bulk-apply",
                                                             );
                                                         }
                                                         Err(err) => failures.push(format!("{key}: {err}")),
                                                     }
                                                 }
                            }

                            let (new_snapshots, metadata) = remember_extension_snapshot(
                                extension_snapshots.read().clone(),
                                ExtensionApplyMode::Bulk,
                                keys.clone(),
                                total_created,
                                workflow_before,
                            );
                            extension_snapshots.set(new_snapshots);
                            let history = extension_timeline.read().clone();
                            extension_timeline.set(push_timeline(
                                history,
                                ExtensionTimelineEventKind::Snapshot,
                                format!(
                                    "Captured rollback snapshot #{} for batch #{} ({} apply).",
                                    metadata.snapshot_id,
                                    metadata.batch_id,
                                    mode_label(metadata.mode)
                                ),
                                Some(metadata.clone()),
                            ));

                            if failures.is_empty() {
                                let summary = format!(
                                    "Applied {} extension(s), added {} node(s) in batch #{}.",
                                    applied_count,
                                    total_created,
                                    metadata.batch_id,
                                );
                                let history = extension_timeline.read().clone();
                                extension_timeline.set(push_timeline(
                                    history,
                                    ExtensionTimelineEventKind::Applied,
                                    summary.clone(),
                                    Some(metadata),
                                ));
                                extension_message.set(Some(summary));
                                selected_extension_keys.set(Vec::new());
                                preview_patches.set(Vec::new());
                            } else {
                                let detail = format!(
                                    "Batch #{} completed with {} error(s): {}",
                                    metadata.batch_id,
                                    failures.len(),
                                    failures.join(" | ")
                                );
                                let history = extension_timeline.read().clone();
                                extension_timeline.set(push_timeline(
                                    history,
                                    ExtensionTimelineEventKind::Failed,
                                    detail.clone(),
                                    Some(metadata),
                                ));
                                extension_message.set(Some(detail));
                            }
                        },
                        "Apply Selected ({selected_count})"
                    }
                    button {
                        class: "h-7 rounded-md border border-slate-300 bg-white px-2.5 text-[11px] text-slate-700 transition-colors hover:bg-slate-100",
                        onclick: move |_| {
                            selected_extension_keys.read().iter().for_each(|key| {
                                record_suggestion_decision(key, false, "bulk-clear");
                            });
                            selected_extension_keys.set(Vec::new());
                            preview_patches.set(Vec::new());
                        },
                        "Clear"
                    }
                }
            }

            if let Some(message) = extension_message.read().as_ref() {
                p { class: "mb-2 rounded-md border border-slate-200 bg-white px-2.5 py-1.5 text-[10px] text-slate-600", "{message}" }
            }

            if suggestions.is_empty() {
                p { class: "rounded-md border border-slate-200 bg-slate-50 px-3 py-2 text-[11px] text-slate-500",
                    "No extension recommendations right now."
                }
            } else {
                div { class: "flex flex-col gap-2",
                    for suggestion in suggestions {
                        {
                            let wf = workflow;
                            let ws = workflow_state;
                            let sek = selected_extension_keys;
                            let em = extension_message;
                            let et = extension_timeline;
                            let es = extension_snapshots;
                            rsx! {
                                SuggestionCard {
                                    suggestion,
                                    workflow: wf,
                                    workflow_state: ws,
                                    selected_extension_keys: sek,
                                    extension_message: em,
                                    extension_timeline: et,
                                    extension_snapshots: es,
                                }
                            }
                        }
                    }
                }
            }

            TimelineSection {
                extension_timeline,
                extension_snapshots,
                workflow,
                workflow_state,
                preview_patches,
                extension_message,
            }
        }
    }
}
