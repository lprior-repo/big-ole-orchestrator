#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use crate::ui::panel_types::{CollapseState, PayloadShape, chevron_rotation_class};
use dioxus::prelude::*;
use oya_frontend::graph::{NodeId, Workflow};
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq)]
pub enum PayloadTab {
    Input,
    Output,
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

fn pretty_json(value: &serde_json::Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string())
}

#[component]
fn PayloadItem(payload: serde_json::Value, index: usize, label: String) -> Element {
    let mut expanded = use_signal(|| false);
    let json_str = pretty_json(&payload);
    let lines: Vec<&str> = json_str.lines().collect();
    let max_lines = 8;
    let shape = PayloadShape::from_value(&payload);
    let shape_display = shape.to_display();

    let display_text = if *expanded.read() || lines.len() <= max_lines {
        json_str.clone()
    } else {
        lines[..max_lines].join("\n") + "\n... (truncated)"
    };

    rsx! {
        div { class: "rounded-lg border border-slate-200 bg-slate-50 p-2.5",
            div { class: "mb-1.5 flex items-center justify-between",
                span { class: "text-[10px] font-semibold text-slate-600", "{label}" }
                span { class: "rounded bg-slate-200 px-1.5 py-0.5 text-[9px] text-slate-500 font-mono", "{shape_display}" }
            }
            pre { class: "overflow-x-auto rounded bg-white border border-slate-200 p-2 font-mono text-[10px] leading-relaxed text-slate-700 whitespace-pre-wrap break-words", "{display_text}" }
            if lines.len() > max_lines {
                button {
                    class: "mt-1.5 text-[9px] text-blue-600 hover:text-blue-500",
                    onclick: move |_| expanded.toggle(),
                    if *expanded.read() { "Show less" } else { "Show more" }
                }
            }
        }
    }
}

#[component]
pub fn PayloadPreviewPanel(
    selected_node_id: Signal<Option<NodeId>>,
    nodes_by_id: ReadSignal<HashMap<NodeId, oya_frontend::graph::Node>>,
    workflow: Signal<Workflow>,
) -> Element {
    let mut active_tab: Signal<PayloadTab> = use_signal(|| PayloadTab::Input);

    let node_id = *selected_node_id.read();
    let Some(node_id) = node_id else {
        return rsx! {};
    };

    let node = nodes_by_id.read().get(&node_id).cloned();
    let Some(node) = node else {
        return rsx! {};
    };

    let input_payloads = collect_input_payloads(&workflow.read(), node_id);
    let output_payload = node.last_output.clone();
    let tab = *active_tab.read();

    let input_tab_class = if tab == PayloadTab::Input {
        "border-b-2 border-blue-500 text-blue-600 bg-blue-50/50"
    } else {
        "text-slate-500 hover:bg-slate-50"
    };
    let output_tab_class = if tab == PayloadTab::Output {
        "border-b-2 border-blue-500 text-blue-600 bg-blue-50/50"
    } else {
        "text-slate-500 hover:bg-slate-50"
    };

    rsx! {
        aside { class: "animate-slide-in-right z-30 flex w-[360px] shrink-0 flex-col border-l border-slate-200 bg-white/98 backdrop-blur-sm",
            div { class: "flex items-center justify-between border-b border-slate-200 px-4 py-3",
                div { class: "flex items-center gap-2",
                    h3 { class: "text-[13px] font-semibold text-slate-900", "Payload Preview" }
                    span { class: "rounded bg-slate-100 px-1.5 py-0.5 text-[10px] text-slate-500 font-mono", "{node.name}" }
                }
                button {
                    class: "flex h-6 w-6 items-center justify-center rounded-md text-slate-500 transition-colors hover:bg-slate-100 hover:text-slate-900",
                    onclick: move |_| selected_node_id.set(None),
                    crate::ui::icons::XIcon { class: "h-3.5 w-3.5" }
                }
            }

            div { class: "flex border-b border-slate-200",
                button {
                    class: "flex-1 py-2 text-[11px] font-medium transition-colors {input_tab_class}",
                    onclick: move |_| active_tab.set(PayloadTab::Input),
                    "Input ({input_payloads.len()})"
                }
                button {
                    class: "flex-1 py-2 text-[11px] font-medium transition-colors {output_tab_class}",
                    onclick: move |_| active_tab.set(PayloadTab::Output),
                    "Output"
                }
            }

            div { class: "flex-1 overflow-y-auto p-3",
                match tab {
                    PayloadTab::Input => {
                        if input_payloads.is_empty() {
                            rsx! {
                                div { class: "rounded-lg border border-dashed border-slate-300 bg-slate-50 p-4 text-center",
                                    p { class: "text-[11px] text-slate-500", "No upstream payloads" }
                                    p { class: "mt-1 text-[10px] text-slate-400", "Connect nodes to see input data flow" }
                                }
                            }
                        } else {
                            rsx! {
                                div { class: "flex flex-col gap-2",
                                    for (index, payload) in input_payloads.iter().enumerate() {
                                        PayloadItem {
                                            payload: payload.clone(),
                                            index,
                                            label: format!("Input #{}", index + 1)
                                        }
                                    }
                                }
                            }
                        }
                    }
                    PayloadTab::Output => {
                        if let Some(output) = output_payload {
                            rsx! {
                                PayloadItem {
                                    payload: output,
                                    index: 0,
                                    label: "Output".to_string()
                                }
                            }
                        } else {
                            rsx! {
                                div { class: "rounded-lg border border-dashed border-slate-300 bg-slate-50 p-4 text-center",
                                    p { class: "text-[11px] text-slate-500", "No output yet" }
                                    p { class: "mt-1 text-[10px] text-slate-400", "Run the workflow to generate output" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
