#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]

use dioxus::prelude::*;
use itertools::Itertools;
use oya_frontend::flow_extender::{
    apply_extension, extension_presets, preview_extension, resolve_extension_preset,
    suggest_extensions, ExtensionPatchPreview, ExtensionPriority,
};
use oya_frontend::graph::{Node, NodeCategory, NodeId, Workflow};
use std::collections::HashMap;

use crate::ui::NodeConfigEditor;

#[component]
pub fn SelectedNodePanel(
    selection: crate::hooks::use_selection::SelectionState,
    nodes_by_id: ReadSignal<HashMap<NodeId, Node>>,
    workflow_state: crate::hooks::use_workflow_state::WorkflowState,
    preview_patches: Signal<Vec<ExtensionPatchPreview>>,
) -> Element {
    let selected_node_id = selection.selected_id();
    let mut workflow = workflow_state.workflow();
    let mut selected_extension_keys = use_signal(Vec::<String>::new);
    let mut extension_message = use_signal(|| None::<String>);
    let mut extension_timeline = use_signal(Vec::<ExtensionTimelineEvent>::new);
    let mut extension_snapshots = use_signal(Vec::<ExtensionBatchSnapshot>::new);

    use_effect(move || {
        let selected = selected_extension_keys.read().clone();
        let next = collect_previews(&workflow.read(), &selected);
        if *preview_patches.read() != next {
            preview_patches.set(next);
        }
    });

    if let Some(node_id) = *selected_node_id.read() {
        if let Some(selected_node) = nodes_by_id.read().get(&node_id).cloned() {
            let badge_classes = match selected_node.category {
                NodeCategory::Entry => "bg-emerald-50 text-emerald-700 border-emerald-200",
                NodeCategory::Durable => "bg-indigo-50 text-indigo-700 border-indigo-200",
                NodeCategory::State => "bg-orange-50 text-orange-700 border-orange-200",
                NodeCategory::Flow => "bg-amber-50 text-amber-700 border-amber-200",
                NodeCategory::Timing => "bg-pink-50 text-pink-700 border-pink-200",
                NodeCategory::Signal => "bg-blue-50 text-blue-700 border-blue-200",
            };

            return rsx! {
                aside { class: "animate-slide-in-right z-30 flex w-[320px] shrink-0 flex-col border-l border-slate-200 bg-white/95",
                    div { class: "flex items-center justify-between border-b border-slate-200 px-4 py-3",
                        div { class: "flex items-center gap-2.5",
                            div { class: "flex h-7 w-7 items-center justify-center rounded-md border {badge_classes}",
                                {crate::ui::icons::icon_by_name(&selected_node.icon, "h-3.5 w-3.5".to_string())}
                            }
                            div {
                                h3 { class: "text-[13px] font-semibold text-slate-900", "{selected_node.name}" }
                                p { class: "text-[10px] text-slate-500", "{selected_node.description}" }
                            }
                        }
                        button {
                            class: "flex h-6 w-6 items-center justify-center rounded-md text-slate-500 transition-colors hover:bg-slate-100 hover:text-slate-900",
                            onclick: move |_| {
                                selection.clear();
                            },
                            crate::ui::icons::XIcon { class: "h-3.5 w-3.5" }
                        }
                    }

                    div { class: "flex-1 overflow-y-auto p-4",
                        div { class: "mb-4 flex items-center gap-2",
                            span { class: "inline-flex items-center rounded-md border px-2 py-0.5 text-[10px] font-medium capitalize {badge_classes}", "{selected_node.category}" }
                            span { class: "text-[10px] font-mono text-slate-500", "ID: {selected_node.id}" }
                        }
                        div { class: "mb-4 flex flex-col gap-1.5",
                            label { class: "text-[11px] font-medium uppercase tracking-wide text-slate-500", "Node Name" }
                            input {
                                class: "h-8 rounded-md border border-slate-300 bg-white px-3 text-[12px] text-slate-900 outline-none transition-colors focus:border-blue-500/50 focus:ring-1 focus:ring-blue-500/30",
                                value: "{selected_node.name}",
                                oninput: move |evt| {
                                    let mut wf = workflow.write();
                                    if let Some(node) = wf.nodes.iter_mut().find(|node| node.id == node_id) {
                                        node.name = evt.value();
                                    }
                                }
                            }
                        }

                        div { class: "mb-4 flex flex-col gap-1.5",
                            label { class: "text-[11px] font-medium uppercase tracking-wide text-slate-500", "Notes" }
                            textarea {
                                rows: "3",
                                placeholder: "Add notes about this node...",
                                class: "rounded-md border border-slate-300 bg-white px-3 py-2 text-[12px] text-slate-900 placeholder:text-slate-500/70 outline-none transition-colors focus:border-blue-500/50 focus:ring-1 focus:ring-blue-500/30 resize-none",
                                value: "{selected_node.description}",
                                oninput: move |evt| {
                                    let mut wf = workflow.write();
                                    if let Some(node) = wf.nodes.iter_mut().find(|node| node.id == node_id) {
                                        node.description = evt.value();
                                    }
                                }
                            }
                        }

                        div { class: "h-px bg-slate-200" }
                        div { class: "pt-4",
                            NodeConfigEditor {
                                node: selected_node.clone(),
                                input_payloads: collect_input_payloads(&workflow.read(), node_id),
                                on_change: move |new_config| {
                                    let mut wf = workflow.write();
                                    if let Some(node) = wf.nodes.iter_mut().find(|node| node.id == node_id) {
                                        node.apply_config_update(&new_config);
                                    }
                                }
                            }
                        }

                        {
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
                                                    let preview = preview_extension(&workflow.read(), &suggestion.key).ok().flatten();
                                                    let (chip_bg, chip_text) = match suggestion.priority {
                                                        ExtensionPriority::High => ("bg-red-100", "text-red-700"),
                                                        ExtensionPriority::Medium => ("bg-amber-100", "text-amber-700"),
                                                        ExtensionPriority::Low => ("bg-slate-100", "text-slate-700"),
                                                    };
                                                    let key = suggestion.key.clone();
                                                    let key_for_card = key.clone();
                                                    let key_for_checkbox = key.clone();
                                                    let key_for_apply = key.clone();
                                                    let title = suggestion.title.clone();
                                                    let is_selected = selected_extension_keys.read().iter().any(|selected| selected == &key);
                                                    let added_nodes = preview.as_ref().map_or(0, |value| value.nodes.len());
                                                    let added_edges = preview.as_ref().map_or(0, |value| value.connections.len());
                                                    let card_state_class = if is_selected {
                                                        "border-indigo-300 bg-indigo-50"
                                                    } else {
                                                        "border-slate-200 bg-slate-50 hover:border-slate-300"
                                                    };
                                                    rsx! {
                                                        div {
                                                            class: "rounded-lg border p-2.5 transition-colors {card_state_class}",
                                                            onclick: move |_| {
                                                                let mut next = selected_extension_keys.read().clone();
                                                                if next.iter().any(|selected| selected == &key_for_card) {
                                                                    next.retain(|selected| selected != &key_for_card);
                                                                } else {
                                                                    next.push(key_for_card.clone());
                                                                }
                                                                selected_extension_keys.set(next);
                                                            },
                                                            div { class: "mb-1.5 flex items-center justify-between gap-2",
                                                                div { class: "flex items-center gap-2",
                                                                    input {
                                                                        r#type: "checkbox",
                                                                        checked: is_selected,
                                                                        onchange: move |event| {
                                                                            event.stop_propagation();
                                                                            let mut next = selected_extension_keys.read().clone();
                                                                            if next.iter().any(|selected| selected == &key_for_checkbox) {
                                                                                next.retain(|selected| selected != &key_for_checkbox);
                                                                                record_suggestion_decision(
                                                                                    &key_for_checkbox,
                                                                                    false,
                                                                                    "checkbox-toggle",
                                                                                );
                                                                            } else {
                                                                                next.push(key_for_checkbox.clone());
                                                                            }
                                                                            selected_extension_keys.set(next);
                                                                        }
                                                                    }
                                                                    p { class: "text-[11px] font-semibold text-slate-800", "{title}" }
                                                                }
                                                                span { class: "rounded px-2 py-0.5 text-[10px] font-medium {chip_bg} {chip_text}", "{suggestion.priority:?}" }
                                                            }
                                                            p { class: "mb-1.5 text-[10px] leading-relaxed text-slate-600", "{suggestion.rationale}" }
                                                            div { class: "flex items-center justify-between gap-2",
                                                                div { class: "flex items-center gap-2 text-[10px] text-slate-500",
                                                                    span { class: "font-mono", "{suggestion.key}" }
                                                                    span { " +{added_nodes} nodes" }
                                                                    span { " +{added_edges} edges" }
                                                                }
                                                                button {
                                                                    class: "h-6 rounded-md border border-emerald-300 bg-emerald-50 px-2 text-[10px] font-medium text-emerald-700 transition-colors hover:bg-emerald-100",
                                                                    onclick: move |event| {
                                                                        event.stop_propagation();
                                                                        let workflow_before = workflow.read().clone();
                                                                        workflow_state.save_undo_point();

                                                                        let result = {
                                                                            let mut wf = workflow.write();
                                                                            apply_extension(&mut wf, &key_for_apply)
                                                                        };

                                                                        let created_nodes = result
                                                                            .as_ref()
                                                                            .map_or(0, |applied| applied.created_nodes.len());
                                                                        let (new_snapshots, metadata) = remember_extension_snapshot(
                                                                            extension_snapshots.read().clone(),
                                                                            ExtensionApplyMode::Single,
                                                                            vec![key_for_apply.clone()],
                                                                            created_nodes,
                                                                            workflow_before,
                                                                        );
                                                                        extension_snapshots.set(new_snapshots);
                                                                        let history = extension_timeline.read().clone();
                                                                        extension_timeline.set(push_timeline(
                                                                            history,
                                                                            ExtensionTimelineEventKind::Snapshot,
                                                                            format!(
                                                                                "Captured rollback snapshot #{} for batch #{} (single apply).",
                                                                                metadata.snapshot_id,
                                                                                metadata.batch_id
                                                                            ),
                                                                            Some(metadata.clone()),
                                                                        ));

                                                                        match result {
                                                                            Ok(applied) => {
                                                                                record_suggestion_decision(
                                                                                    &key_for_apply,
                                                                                    true,
                                                                                    "single-apply",
                                                                                );
                                                                                let summary = format!(
                                                                                    "Applied '{}' in batch #{}, added {} node(s).",
                                                                                    key_for_apply,
                                                                                    metadata.batch_id,
                                                                                    applied.created_nodes.len()
                                                                                );
                                                                                let history = extension_timeline.read().clone();
                                                                                extension_timeline.set(push_timeline(
                                                                                    history,
                                                                                    ExtensionTimelineEventKind::Applied,
                                                                                    summary.clone(),
                                                                                    Some(metadata),
                                                                                ));
                                                                                extension_message.set(Some(summary));
                                                                                let mut next = selected_extension_keys.read().clone();
                                                                                next.retain(|selected| selected != &key_for_apply);
                                                                                selected_extension_keys.set(next);
                                                                            }
                                                                            Err(err) => {
                                                                                let detail = format!(
                                                                                    "Failed '{}' in batch #{}: {}",
                                                                                    key_for_apply,
                                                                                    metadata.batch_id,
                                                                                    err
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
                                                                        }
                                                                    },
                                                                    "Apply"
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }

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
                                                                                onclick: move |event| {
                                                                                    event.stop_propagation();
                                                                                    if let Some(snapshot) = snapshot_by_id(
                                                                                        &extension_snapshots.read(),
                                                                                        meta.snapshot_id,
                                                                                    ) {
                                                                                        workflow_state.save_undo_point();
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
                        }
                    }

                    div { class: "flex items-center gap-2 border-t border-slate-200 px-4 py-3",
                        button {
                            class: "flex h-8 flex-1 items-center justify-center gap-1.5 rounded-md border border-slate-300 text-[12px] text-slate-700 transition-colors hover:bg-slate-100",
                            onclick: move |_| {
                                workflow_state.save_undo_point();

                                let maybe_clone = workflow
                                    .read()
                                    .nodes
                                    .iter()
                                    .find(|node| node.id == node_id)
                                    .cloned();
                                if let Some(mut clone) = maybe_clone {
                                    clone.id = NodeId::new();
                                    clone.x += 40.0;
                                    clone.y += 40.0;
                                    let cloned_id = clone.id;
                                    workflow.write().nodes.push(clone);
                                    selection.select_single(cloned_id);
                                }
                            },
                            crate::ui::icons::CopyIcon { class: "h-3.5 w-3.5" }
                            "Duplicate"
                        }
                        button {
                            class: "flex h-8 flex-1 items-center justify-center gap-1.5 rounded-md border border-red-500/30 text-[12px] text-red-400 transition-colors hover:bg-red-500/10",
                            onclick: move |_| {
                                workflow_state.save_undo_point();
                                workflow.write().remove_node(node_id);
                                selection.clear();
                            },
                            crate::ui::icons::TrashIcon { class: "h-3.5 w-3.5" }
                            "Delete"
                        }
                    }
                }
            };
        }
    }

    rsx! {}
}

fn collect_previews(workflow: &Workflow, keys: &[String]) -> Vec<ExtensionPatchPreview> {
    keys.iter()
        .unique()
        .filter_map(|key| preview_extension(workflow, key).ok().flatten())
        .collect::<Vec<_>>()
}

fn collect_input_payloads(workflow: &Workflow, node_id: NodeId) -> Vec<serde_json::Value> {
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

#[cfg(not(target_arch = "wasm32"))]
fn record_suggestion_decision(key: &str, accepted: bool, source: &str) {
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
    let _ = store.record_suggestion_decision(metrics);
}

#[cfg(target_arch = "wasm32")]
fn record_suggestion_decision(_key: &str, _accepted: bool, _source: &str) {}

#[derive(Clone, Copy)]
enum ExtensionTimelineEventKind {
    Snapshot,
    Applied,
    Failed,
    Undone,
    Redone,
    RolledBack,
}

#[derive(Clone)]
struct ExtensionTimelineEvent {
    id: usize,
    kind: ExtensionTimelineEventKind,
    message: String,
    metadata: Option<ExtensionTimelineMetadata>,
}

#[derive(Clone)]
struct ExtensionTimelineMetadata {
    batch_id: usize,
    snapshot_id: usize,
    mode: ExtensionApplyMode,
}

#[derive(Clone, Copy)]
enum ExtensionApplyMode {
    Single,
    Bulk,
}

#[derive(Clone)]
struct ExtensionBatchSnapshot {
    batch_id: usize,
    snapshot_id: usize,
    mode: ExtensionApplyMode,
    keys: Vec<String>,
    created_nodes: usize,
    workflow_before: Workflow,
}

fn push_timeline(
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
    new_timeline.extend(
        timeline
            .into_iter()
            .take(11),
    );
    new_timeline
}

fn event_appearance(
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

fn mode_label(mode: ExtensionApplyMode) -> &'static str {
    match mode {
        ExtensionApplyMode::Single => "single",
        ExtensionApplyMode::Bulk => "bulk",
    }
}

fn remember_extension_snapshot(
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
    new_snapshots.extend(
        snapshots
            .into_iter()
            .take(23),
    );

    (
        new_snapshots,
        ExtensionTimelineMetadata {
            batch_id: next_batch_id,
            snapshot_id: next_snapshot_id,
            mode,
        },
    )
}

fn snapshot_by_id(
    snapshots: &[ExtensionBatchSnapshot],
    snapshot_id: usize,
) -> Option<ExtensionBatchSnapshot> {
    snapshots
        .iter()
        .find(|entry| entry.snapshot_id == snapshot_id)
        .cloned()
}

#[cfg(test)]
mod tests {
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
        assert!(label_class.contains("text-red-700"));
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
        let _ = workflow.add_node("run", 10.0, 10.0);
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
        let _ = workflow.add_node("run", 10.0, 10.0);
        let keys = vec![
            "unknown-extension-key".to_string(),
            "add-timeout-guard".to_string(),
        ];

        let previews = collect_previews(&workflow, &keys);
        let expected = preview_extension(&workflow, "add-timeout-guard");

        assert!(expected.is_ok());
        let expected = expected.ok().flatten();
        assert!(expected.is_some());
        assert_eq!(previews.len(), 1);
        assert_eq!(previews.first(), expected.as_ref());
    }
}
