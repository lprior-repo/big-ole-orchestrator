//! Inline text editor component for workflow definitions.
//!
//! Provides a monospace text area with line numbers and basic syntax
//! hinting (TOML/JSON format indicator).

use dioxus::prelude::*;

use super::types::WorkflowDefinition;
use super::file_upload::{detect_format, parse_content, FormatHint};

/// An inline editor for workflow definition content.
///
/// Shows line numbers, format indicator, and a live preview panel
/// when valid JSON/TOML is entered.
#[component]
pub fn WorkflowEditor(
    value: Signal<String>,
    format: Signal<FormatHint>,
    parse_error: Signal<Option<String>>,
    preview: Signal<Option<WorkflowDefinition>>,
    on_change: EventHandler<String>,
) -> Element {
    let content = value.read().clone();
    let current_format = *format.read();
    let error = parse_error.read().clone();
    let preview_def = preview.read().clone();
    let line_count = content.lines().count();

    let format_label = match current_format {
        FormatHint::Toml => "TOML",
        FormatHint::Json => "JSON",
        FormatHint::Auto => "Auto",
    };

    let format_class = match current_format {
        FormatHint::Json => "bg-blue-100 text-blue-700",
        _ => "bg-green-100 text-green-700",
    };

    let placeholder = match current_format {
        FormatHint::Json => r#"{"name": "my-workflow", "nodes": [{"id": "1", "name": "Step 1", "kind": "pure", "x": 100, "y": 100}], "edges": []}"#,
        FormatHint::Toml => "# Workflow definition\nname = \"my-workflow\"\nguarrantee_class = \"best_effort\"\n\n[[nodes]]\nid = \"1\"\nname = \"Step 1\"\nkind = \"pure\"\nx = 100\ny = 100\n\n[edges]",
        FormatHint::Auto => "{# Enter workflow definition (TOML or JSON)\nname = \"my-workflow\" ...}",
    };

    let line_text = if line_count == 1 { "" } else { "s" };

    let preview_data = preview_def.as_ref().map(|d| {
        let node_text = if d.nodes.len() == 1 { "node" } else { "nodes" };
        let edge_text = if d.edges.len() == 1 { "edge" } else { "edges" };
        let shortened_name = if d.name.len() > 18 {
            d.name[..15].to_string()
        } else {
            d.name.clone()
        };
        (d.nodes.len(), node_text, d.edges.len(), edge_text, shortened_name)
    });

    rsx! {
        div {
            class: "border border-gray-200 rounded-lg overflow-hidden",
            // Editor header with format indicator
            div {
                class: "flex items-center justify-between px-4 py-2 bg-gray-50 border-b border-gray-200",
                div { class: "flex items-center gap-2",
                    span { class: "text-sm font-medium text-gray-700", "Editor" }
                    span {
                        class: "text-xs px-2 py-0.5 rounded-full {format_class}",
                        "{format_label}"
                    }
                }
                div { class: "text-xs text-gray-500",
                    "{line_count} line{line_text}"
                }
            }
            // Line number gutter + textarea
            div { class: "flex",
                // Line numbers gutter
                div {
                    class: "px-3 py-3 bg-gray-50 border-r border-gray-200 select-none",
                    style: "font-family: ui-monospace, SFMono-Regular, monospace",
                    style: "min-width: 2.5rem",
                    for i in 1..=line_count.max(1) {
                        div {
                            class: "text-right text-gray-400 text-xs leading-5",
                            "{i}"
                        }
                    }
                }
                // Editor textarea
                textarea {
                    class: "w-full p-3 text-sm font-mono bg-white resize-none focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-inset leading-5",
                    class: "text-gray-900",
                    value: "{content}",
                    oninput: move |e| {
                        value.set(e.value());
                        on_change.call(e.value());
                    },
                    placeholder: "{placeholder}",
                    spellcheck: "false",
                }
            }
            // Error display
            if let Some(ref err) = error {
                div { class: "px-4 py-2 bg-red-50 border-t border-red-200",
                    p { class: "text-sm text-red-700", "{err}" }
                }
            }
            // Live preview (when valid and non-empty)
            if let Some((pn, nt, pe, et, name)) = &preview_data {
                div { class: "px-4 py-3 bg-blue-50 border-t border-blue-200",
                    div { class: "flex items-center justify-between",
                        span { class: "text-sm font-semibold text-blue-800", "Live Preview" }
                        span { class: "text-xs text-blue-600",
                            "{pn} {nt}, {pe} {et}"
                        }
                    }
                    div { class: "mt-1 text-xs text-blue-600", "Name: {name}" }
                }
            }
        }
    }
}
