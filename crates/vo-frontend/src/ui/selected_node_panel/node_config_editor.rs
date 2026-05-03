#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]

use dioxus::prelude::*;
use serde_json::Value;

use crate::ui::graph::Node;

/// Stub node configuration editor component.
///
/// This is a placeholder for the full configuration editor. It displays
/// the node's current config as JSON and accepts config updates.
#[component]
pub fn NodeConfigEditor(
    node: Node,
    input_payloads: Vec<Value>,
    on_change: EventHandler<Value>,
) -> Element {
    let config_json = serde_json::to_string_pretty(&node.config).unwrap_or_else(|_| "{}".to_string());

    rsx! {
        div { class: "flex flex-col gap-2",
            label { class: "text-[11px] font-medium uppercase tracking-wide text-slate-500", "Configuration" }
            if input_payloads.is_empty() {
                p { class: "text-[10px] text-slate-400", "No input payloads" }
            } else {
                p { class: "text-[10px] text-slate-400", "{input_payloads.len()} input payload(s)" }
            }
            pre { class: "rounded bg-slate-50 border border-slate-200 p-2 font-mono text-[10px] text-slate-600 overflow-auto max-h-48 whitespace-pre-wrap break-words",
                "{config_json}"
            }
        }
    }
}
