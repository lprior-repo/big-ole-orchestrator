#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]

use dioxus::prelude::*;
use oya_frontend::flow_extender::{
    apply_extension, preview_extension, ExtensionPriority, FlowExtension,
};
use oya_frontend::graph::Workflow;

use super::types::{
    push_timeline, record_suggestion_decision, remember_extension_snapshot, ExtensionApplyMode,
    ExtensionBatchSnapshot, ExtensionTimelineEvent, ExtensionTimelineEventKind,
};

#[component]
pub(crate) fn SuggestionCard(
    suggestion: FlowExtension,
    workflow: Signal<Workflow>,
    workflow_state: crate::hooks::use_workflow_state::WorkflowState,
    selected_extension_keys: Signal<Vec<String>>,
    extension_message: Signal<Option<String>>,
    extension_timeline: Signal<Vec<ExtensionTimelineEvent>>,
    extension_snapshots: Signal<Vec<ExtensionBatchSnapshot>>,
) -> Element {
    let preview = preview_extension(&workflow.read(), &suggestion.key)
        .ok()
        .flatten();
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
    let is_selected = selected_extension_keys
        .read()
        .iter()
        .any(|selected| selected == &key);
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
