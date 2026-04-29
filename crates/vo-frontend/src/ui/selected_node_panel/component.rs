#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]

use dioxus::prelude::*;
use oya_frontend::flow_extender::ExtensionPatchPreview;
use std::collections::HashMap;

use crate::ui::graph::NodeCategory;
use crate::ui::graph::{Node, NodeId, Workflow};
use crate::ui::NodeGuaranteeBadge;

use crate::ui::NodeConfigEditor;

use super::extend_flow::ExtendFlowSection;
use super::types::{
    collect_input_payloads, collect_previews, ExtensionBatchSnapshot, ExtensionTimelineEvent,
};

#[component]
pub fn SelectedNodePanel(
    nodes_by_id: ReadSignal<HashMap<NodeId, Node>>,
    preview_patches: Signal<Vec<ExtensionPatchPreview>>,
) -> Element {
    let (selection, selected_node_id) = crate::hooks::use_selection::use_selection();
    let (workflow_state, workflow) = crate::hooks::use_workflow_state::use_workflow_state(Workflow::new("".to_string(), vo_types::GuaranteeClass::BestEffort));
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
            let badge_classes = selected_node.category.badge_class();
            let workflow_guarantee = workflow.read().guarantee_class;

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
                                selection.0.clear(selected_node_id.clone());
                            },
                            crate::ui::icons::XIcon { class: "h-3.5 w-3.5" }
                        }
                    }

                    div { class: "flex-1 overflow-y-auto p-4",
                        div { class: "mb-4 flex items-center gap-2",
                            span { class: "inline-flex items-center rounded-md border px-2 py-0.5 text-[10px] font-medium capitalize {badge_classes}", "{selected_node.category}" }
                            NodeGuaranteeBadge {
                                node_kind: selected_node.kind,
                                workflow_guarantee,
                            }
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

                        ExtendFlowSection {
                            node_id,
                            workflow,
                            workflow_state,
                            selected_extension_keys,
                            extension_message,
                            extension_timeline,
                            extension_snapshots,
                            preview_patches,
                        }
                    }

                    div { class: "flex items-center gap-2 border-t border-slate-200 px-4 py-3",
                        button {
                            class: "flex h-8 flex-1 items-center justify-center gap-1.5 rounded-md border border-slate-300 text-[12px] text-slate-700 transition-colors hover:bg-slate-100",
                            onclick: move |_| {
                                workflow_state.0.save_undo_point();

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
                                    selection.0.select_single(cloned_id, selected_node_id.clone());
                                }
                            },
                            crate::ui::icons::CopyIcon { class: "h-3.5 w-3.5" }
                            "Duplicate"
                        }
                        button {
                            class: "flex h-8 flex-1 items-center justify-center gap-1.5 rounded-md border border-red-500/30 text-[12px] text-red-400 transition-colors hover:bg-red-500/10",
                            onclick: move |_| {
                                workflow_state.0.save_undo_point();
                                workflow.write().remove_node(node_id);
                                selection.0.clear(selected_node_id.clone());
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
