#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![forbid(unsafe_code)]

use dioxus::prelude::*;
use std::fmt::Write as _;

use super::domain_types::NodeTemplateId;

#[derive(Clone, PartialEq, Eq)]
pub struct SketchNode {
    pub node_type: NodeTemplateId,
    pub label: String,
}

struct PaletteEntry {
    node_type: NodeTemplateId,
    icon: &'static str,
}

const PALETTE_ENTRIES: [PaletteEntry; 9] = [
    PaletteEntry {
        node_type: NodeTemplateId::HttpHandler,
        icon: "🌐",
    },
    PaletteEntry {
        node_type: NodeTemplateId::Run,
        icon: "🛡️",
    },
    PaletteEntry {
        node_type: NodeTemplateId::Sleep,
        icon: "⏱️",
    },
    PaletteEntry {
        node_type: NodeTemplateId::SetState,
        icon: "⬆️",
    },
    PaletteEntry {
        node_type: NodeTemplateId::GetState,
        icon: "⬇️",
    },
    PaletteEntry {
        node_type: NodeTemplateId::SendMessage,
        icon: "📤",
    },
    PaletteEntry {
        node_type: NodeTemplateId::Condition,
        icon: "⑂",
    },
    PaletteEntry {
        node_type: NodeTemplateId::Parallel,
        icon: "⫿",
    },
    PaletteEntry {
        node_type: NodeTemplateId::Condition,
        icon: "⑂",
    },
];

pub fn generate_skeleton(nodes: &[SketchNode]) -> String {
    let mut out = String::from("name: \"prototype-workflow\"\nsteps:\n");

    for (i, node) in nodes.iter().enumerate() {
        let step_id = format!("step-{}", i + 1);
        let _ = writeln!(out, "  - id: {step_id}");
        let _ = writeln!(out, "    type: {}", node.node_type);
        if i > 0 {
            let prev_id = format!("step-{i}");
            let _ = writeln!(out, "    depends_on: [{prev_id}]");
        }
        out.push_str("    config: {}\n");
    }

    out
}

#[component]
pub fn PrototypePalette(
    open: ReadSignal<bool>,
    on_close: EventHandler<()>,
    on_add_node: EventHandler<NodeTemplateId>,
) -> Element {
    if !*open.read() {
        return rsx! {};
    }

    let mut sketch_nodes: Signal<Vec<SketchNode>> = use_signal(Vec::new);
    let mut generated_skeleton: Signal<Option<String>> = use_signal(|| None);

    let nodes_snapshot = sketch_nodes.read().clone();
    let skeleton_snapshot = generated_skeleton.read().clone();

    rsx! {
        div {
            class: "fixed inset-0 z-50 bg-black/40 backdrop-blur-sm flex items-start justify-center overflow-y-auto",
            onclick: move |_| on_close.call(()),

            div {
                class: "relative w-full max-w-2xl mx-auto mt-24 mb-12 bg-white rounded-xl shadow-2xl overflow-hidden",
                onclick: move |evt| evt.stop_propagation(),

                div {
                    class: "flex items-start justify-between px-6 py-5 border-b border-slate-200",
                    div {
                        h2 { class: "text-lg font-bold text-slate-900", "Prototype Mode" }
                        p {
                            class: "mt-0.5 text-sm text-slate-500",
                            "Sketch your workflow, then generate a code skeleton"
                        }
                    }
                    button {
                        class: "rounded-lg border border-slate-200 px-3 py-1.5 text-xs font-medium text-slate-500 transition-colors hover:border-slate-400 hover:text-slate-700",
                        onclick: move |_| on_close.call(()),
                        "Close"
                    }
                }

                div { class: "px-6 py-4 border-b border-slate-100",
                    h3 { class: "mb-3 text-xs font-semibold uppercase tracking-wide text-slate-400", "Node Types" }
                    div { class: "grid grid-cols-3 gap-2",
                        for entry in PALETTE_ENTRIES {
                            button {
                                key: "{entry.node_type}",
                                class: "flex flex-col items-center gap-1.5 rounded-lg border border-slate-200 bg-slate-50 px-3 py-3 text-center transition-colors hover:border-indigo-300 hover:bg-indigo-50 hover:text-indigo-700 active:scale-95",
                                onclick: move |_| {
                                    let new_node = SketchNode {
                                        node_type: entry.node_type,
                                        label: entry.node_type.label().to_string(),
                                    };
                                    sketch_nodes.write().push(new_node);
                                    *generated_skeleton.write() = None;
                                    on_add_node.call(entry.node_type);
                                },
                                span { class: "text-2xl leading-none", "{entry.icon}" }
                                span { class: "font-mono text-[10px] text-slate-500", "{entry.node_type}" }
                                span { class: "text-[11px] font-medium text-slate-700", "{entry.node_type.label()}" }
                            }
                        }
                    }
                }

                div { class: "px-6 py-4 border-b border-slate-100 min-h-[64px]",
                    h3 { class: "mb-2 text-xs font-semibold uppercase tracking-wide text-slate-400", "Sketch" }
                    if nodes_snapshot.is_empty() {
                        p { class: "text-sm text-slate-400 italic", "Click node types above to add them." }
                    } else {
                        div { class: "flex flex-wrap gap-2",
                            for (idx, node) in nodes_snapshot.iter().enumerate() {
                                div {
                                    key: "{idx}",
                                    class: "inline-flex items-center gap-1 rounded-full bg-indigo-100 px-3 py-1 text-xs font-medium text-indigo-800",
                                    span { "{node.node_type}" }
                                    button {
                                        class: "ml-1 rounded-full text-indigo-500 hover:text-indigo-900 leading-none",
                                        "aria-label": "Remove {node.node_type}",
                                        onclick: move |_| {
                                            let mut nodes = sketch_nodes.write();
                                            if idx < nodes.len() {
                                                nodes.remove(idx);
                                            }
                                            *generated_skeleton.write() = None;
                                        },
                                        "×"
                                    }
                                }
                            }
                        }
                    }
                }

                div { class: "px-6 py-4",
                    if !nodes_snapshot.is_empty() {
                        button {
                            class: "w-full rounded-lg bg-indigo-600 px-4 py-2.5 text-sm font-semibold text-white shadow-sm transition-colors hover:bg-indigo-700 active:bg-indigo-800",
                            onclick: move |_| {
                                let nodes = sketch_nodes.read().clone();
                                *generated_skeleton.write() = Some(generate_skeleton(&nodes));
                            },
                            "Generate Code Skeleton"
                        }
                    }

                    if let Some(ref skeleton) = skeleton_snapshot {
                        div { class: "mt-4",
                            div { class: "mb-2 flex items-center justify-between",
                                h3 { class: "text-xs font-semibold uppercase tracking-wide text-slate-400", "Generated Skeleton" }
                                button {
                                    class: "rounded border border-slate-200 px-2.5 py-1 text-xs font-medium text-slate-500 transition-colors hover:border-slate-400 hover:text-slate-700",
                                    onclick: {
                                        let skeleton_to_copy = skeleton.clone();
                                        move |_| {
                                            let js = format!(
                                                "navigator.clipboard.writeText({skeleton_to_copy:?}).catch(()=>{{}})"
                                            );
                                            document::eval(&js);
                                        }
                                    },
                                    "Copy"
                                }
                            }
                            pre {
                                class: "overflow-x-auto rounded-lg bg-slate-900 p-4 font-mono text-xs text-green-400 whitespace-pre",
                                "{skeleton}"
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{generate_skeleton, SketchNode, NodeTemplateId};

    fn node(node_type: NodeTemplateId) -> SketchNode {
        SketchNode {
            node_type,
            label: node_type.label().to_string(),
        }
    }

    #[test]
    fn given_empty_nodes_when_generating_skeleton_then_produces_header_only() {
        let result = generate_skeleton(&[]);
        assert!(result.contains("name: \"prototype-workflow\""));
        assert!(result.contains("steps:"));
        assert!(!result.contains("step-1"));
    }

    #[test]
    fn given_two_nodes_when_generating_skeleton_then_second_has_depends_on() {
        let nodes = vec![node(NodeTemplateId::HttpHandler), node(NodeTemplateId::Run)];
        let result = generate_skeleton(&nodes);

        assert!(result.contains("id: step-1"));
        assert!(result.contains("type: http-handler"));
        assert!(result.contains("id: step-2"));
        assert!(result.contains("type: run"));
        assert!(!result
            .lines()
            .take_while(|l| !l.contains("step-2"))
            .any(|l| l.contains("depends_on")));
        assert!(result.contains("depends_on: [step-1]"));
    }

    #[test]
    fn given_three_nodes_when_generating_skeleton_then_linear_chain_is_correct() {
        let nodes = vec![node(NodeTemplateId::HttpHandler), node(NodeTemplateId::Run), node(NodeTemplateId::Sleep)];
        let result = generate_skeleton(&nodes);

        let lines: Vec<&str> = result.lines().collect();

        let step1_block: Vec<&str> = lines
            .iter()
            .skip_while(|l| !l.contains("id: step-1"))
            .take_while(|l| !l.contains("id: step-2"))
            .copied()
            .collect();
        assert!(!step1_block.iter().any(|l| l.contains("depends_on")));

        let step2_block: Vec<&str> = lines
            .iter()
            .skip_while(|l| !l.contains("id: step-2"))
            .take_while(|l| !l.contains("id: step-3"))
            .copied()
            .collect();
        assert!(step2_block
            .iter()
            .any(|l| l.contains("depends_on: [step-1]")));

        let step3_block: Vec<&str> = lines
            .iter()
            .skip_while(|l| !l.contains("id: step-3"))
            .copied()
            .collect();
        assert!(step3_block
            .iter()
            .any(|l| l.contains("depends_on: [step-2]")));
        assert!(step3_block.iter().any(|l| l.contains("type: sleep")));
    }
}
