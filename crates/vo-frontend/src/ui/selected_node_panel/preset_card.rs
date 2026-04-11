#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]

use dioxus::prelude::*;
use oya_frontend::flow_extender::{
    apply_extension, resolve_extension_preset, ExtensionPatchPreview, ExtensionPreset,
};
use oya_frontend::graph::Workflow;

use super::types::{
    push_timeline, record_suggestion_decision, remember_extension_snapshot, ExtensionApplyMode,
    ExtensionBatchSnapshot, ExtensionTimelineEvent, ExtensionTimelineEventKind,
};

#[component]
pub(crate) fn PresetCard(
    preset: ExtensionPreset,
    workflow: Signal<Workflow>,
    workflow_state: crate::hooks::use_workflow_state::WorkflowState,
    selected_extension_keys: Signal<Vec<String>>,
    extension_message: Signal<Option<String>>,
    extension_timeline: Signal<Vec<ExtensionTimelineEvent>>,
    extension_snapshots: Signal<Vec<ExtensionBatchSnapshot>>,
    preview_patches: Signal<Vec<ExtensionPatchPreview>>,
) -> Element {
    let preset_key_for_preview = preset.key.clone();
    let preset_key_for_apply = preset.key.clone();
    let preset_title_for_preview = preset.title.clone();
    let preset_title_for_apply = preset.title.clone();

    rsx! {
        div { class: "rounded-md border border-slate-200 bg-white px-2.5 py-2",
            div { class: "mb-1 flex items-start justify-between gap-2",
                div {
                    p { class: "text-[11px] font-semibold text-slate-800", "{preset.title}" }
                    p { class: "text-[10px] leading-relaxed text-slate-600", "{preset.description}" }
                }
                span { class: "rounded bg-slate-100 px-1.5 py-0.5 text-[9px] font-mono text-slate-600", "{preset.key}" }
            }
            div { class: "mb-2 flex flex-wrap items-center gap-1 text-[9px] text-slate-500",
                for key in preset.extension_keys.clone() {
                    span { class: "rounded border border-slate-200 bg-slate-50 px-1.5 py-0.5 font-mono", "{key}" }
                }
            }
            div { class: "flex items-center gap-1.5",
                button {
                    class: "h-6 rounded-md border border-slate-300 bg-white px-2 text-[10px] font-medium text-slate-700 transition-colors hover:bg-slate-100",
                    onclick: move |_| {
                        match resolve_extension_preset(&workflow.read(), &preset_key_for_preview) {
                            Ok(resolved) => {
                                if resolved.conflicts.is_empty() {
                                    let count = resolved.ordered_keys.len();
                                    selected_extension_keys.set(resolved.ordered_keys.clone());
                                    extension_message.set(Some(format!(
                                        "Previewing preset '{preset_title_for_preview}' ({count} extension rules).",
                                    )));
                                } else {
                                    let conflict_count = resolved.conflicts.len();
                                    let detail = format!(
                                        "Preset '{preset_title_for_preview}' has {conflict_count} conflict(s). Resolve conflicts before apply.",
                                    );
                                    let history = extension_timeline.read().clone();
                                    extension_timeline.set(push_timeline(
                                        history,
                                        ExtensionTimelineEventKind::Failed,
                                        detail.clone(),
                                        None,
                                    ));
                                    extension_message.set(Some(detail));
                                    selected_extension_keys.set(Vec::new());
                                    preview_patches.set(Vec::new());
                                }
                            }
                             Err(err) => {
                                  let detail = format!(
                                      "Failed preset preview '{preset_key_for_preview}': {err}",
                                  );
                                 let history = extension_timeline.read().clone();
                                 extension_timeline.set(push_timeline(
                                     history,
                                     ExtensionTimelineEventKind::Failed,
                                     detail.clone(),
                                     None,
                                 ));
                                 extension_message.set(Some(detail));
                             }
                        }
                    },
                    "Preview"
                }
                button {
                    class: "h-6 rounded-md border border-blue-300 bg-blue-50 px-2 text-[10px] font-medium text-blue-700 transition-colors hover:bg-blue-100",
                    onclick: move |_| {
                        let resolved = resolve_extension_preset(&workflow.read(), &preset_key_for_apply);
                        let resolved = match resolved {
                            Ok(value) => value,
                             Err(err) => {
                                  let detail = format!(
                                      "Failed preset apply '{preset_key_for_apply}': {err}",
                                  );
                                 let history = extension_timeline.read().clone();
                                 extension_timeline.set(push_timeline(
                                     history,
                                     ExtensionTimelineEventKind::Failed,
                                     detail.clone(),
                                     None,
                                 ));
                                 extension_message.set(Some(detail));
                                 return;
                             }
                        };

                        if !resolved.conflicts.is_empty() {
                            let detail = format!(
                                "Preset '{}' blocked by {} conflict(s).",
                                preset_title_for_apply,
                                resolved.conflicts.len(),
                            );
                            let history = extension_timeline.read().clone();
                            extension_timeline.set(push_timeline(
                                history,
                                ExtensionTimelineEventKind::Failed,
                                detail.clone(),
                                None,
                            ));
                            extension_message.set(Some(detail));
                            return;
                        }

                        let workflow_before = workflow.read().clone();
                        workflow_state.save_undo_point();

                        let mut total_created = 0usize;
                        let mut applied_count = 0usize;
                        let mut failures = Vec::new();
                        {
                            let mut wf = workflow.write();
                            resolved.ordered_keys.iter().for_each(|key| {
                                match apply_extension(&mut wf, key) {
                                    Ok(applied) => {
                                        total_created += applied.created_nodes.len();
                                        applied_count += 1;
                                        record_suggestion_decision(
                                            key,
                                            true,
                                            "preset-apply",
                                        );
                                    }
                                    Err(err) => failures.push(format!("{key}: {err}")),
                                }
                            });
                        }

                        let (new_snapshots, metadata) = remember_extension_snapshot(
                            extension_snapshots.read().clone(),
                            ExtensionApplyMode::Bulk,
                            resolved.ordered_keys.clone(),
                            total_created,
                            workflow_before,
                        );
                        extension_snapshots.set(new_snapshots);
                        let history = extension_timeline.read().clone();
                        extension_timeline.set(push_timeline(
                            history,
                            ExtensionTimelineEventKind::Snapshot,
                            format!(
                                "Captured rollback snapshot #{} for batch #{} (preset apply).",
                                metadata.snapshot_id,
                                metadata.batch_id,
                            ),
                            Some(metadata.clone()),
                        ));

                        if failures.is_empty() {
                            let summary = format!(
                                "Applied preset '{}' in batch #{} with {} extension(s), added {} node(s).",
                                preset_title_for_apply,
                                metadata.batch_id,
                                applied_count,
                                total_created,
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
                                "Preset '{}' batch #{} completed with {} error(s): {}",
                                preset_title_for_apply,
                                metadata.batch_id,
                                failures.len(),
                                failures.join(" | "),
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
                    "Apply preset"
                }
            }
        }
    }
}
