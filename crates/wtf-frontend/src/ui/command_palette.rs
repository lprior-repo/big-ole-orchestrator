use dioxus::prelude::*;

use super::domain_types::NodeTemplateId;

#[inline]
pub fn is_escape_key(key: &str) -> bool {
    let key_lower = key.to_lowercase();
    key_lower == "escape" || key_lower == "esc"
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct CommandTemplate {
    pub node_type: NodeTemplateId,
}

pub fn filtered_templates(query: &str) -> Vec<CommandTemplate> {
    let normalized_query = query.trim().to_lowercase();

    if normalized_query.is_empty() {
        return NodeTemplateId::all()
            .into_iter()
            .map(|id| CommandTemplate { node_type: id })
            .collect();
    }

    NodeTemplateId::all()
        .into_iter()
        .filter(|id| {
            id.as_str().contains(&normalized_query)
                || id.label().to_lowercase().contains(&normalized_query)
                || id.hint().to_lowercase().contains(&normalized_query)
        })
        .map(|id| CommandTemplate { node_type: id })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{filtered_templates, is_escape_key};

    #[test]
    fn given_empty_query_when_filtering_templates_then_all_templates_are_returned() {
        let templates = filtered_templates("");
        assert_eq!(templates.len(), 14);
    }

    #[test]
    fn given_case_insensitive_query_when_filtering_then_label_hint_and_type_are_matched() {
        let by_label = filtered_templates("HTTP");
        let by_hint = filtered_templates("durably");
        let by_type = filtered_templates("kafka-handler");

        assert!(by_label
            .iter()
            .any(|t| t.node_type == NodeTemplateId::HttpHandler));
        assert!(by_hint.iter().any(|t| t.node_type == NodeTemplateId::Sleep));
        assert!(by_type
            .iter()
            .any(|t| t.node_type == NodeTemplateId::KafkaHandler));
    }

    #[test]
    fn given_non_matching_query_when_filtering_templates_then_empty_vec_is_returned() {
        let templates = filtered_templates("zz-no-match-zz");
        assert!(templates.is_empty());
    }

    #[test]
    fn given_query_with_leading_and_trailing_whitespace_then_query_is_trimmed() {
        let templates = filtered_templates("  HTTP  ");
        assert!(!templates.is_empty());
        assert!(templates
            .iter()
            .any(|t| t.node_type == NodeTemplateId::HttpHandler));
    }

    #[test]
    fn when_key_is_escape_then_returns_true() {
        assert!(is_escape_key("Escape"));
        assert!(is_escape_key("escape"));
        assert!(is_escape_key("ESCAPE"));
    }

    #[test]
    fn when_key_is_esc_then_returns_true() {
        assert!(is_escape_key("Esc"));
        assert!(is_escape_key("esc"));
        assert!(is_escape_key("ESC"));
    }

    #[test]
    fn when_key_is_not_escape_then_returns_false() {
        assert!(!is_escape_key("Enter"));
        assert!(!is_escape_key("Tab"));
        assert!(!is_escape_key("a"));
        assert!(!is_escape_key(""));
    }
}

#[component]
pub fn NodeCommandPalette(
    open: ReadSignal<bool>,
    query: ReadSignal<String>,
    on_query_change: EventHandler<String>,
    on_close: EventHandler<()>,
    on_pick: EventHandler<NodeTemplateId>,
) -> Element {
    if !*open.read() {
        return rsx! {};
    }

    let query_value = query.read().to_string();
    let templates = filtered_templates(&query_value);

    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-slate-950/45 p-4 backdrop-blur-sm",
            onclick: move |_| on_close.call(()),
            div {
                class: "w-full max-w-xl overflow-hidden rounded-xl border border-slate-700/70 bg-slate-900/95 shadow-2xl",
                onclick: move |evt| evt.stop_propagation(),
                div {
                    class: "flex items-center justify-between border-b border-slate-800 px-4 py-3",
                    h2 { class: "text-[14px] font-semibold text-slate-100", "Quick Add Node" }
                    button {
                        class: "rounded-md border border-slate-700 px-2 py-1 text-[11px] font-medium text-slate-300 transition-colors hover:border-slate-500 hover:text-white",
                        onclick: move |_| on_close.call(()),
                        "Close"
                    }
                }

                div { class: "border-b border-slate-800 px-4 py-3",
                    input {
                        r#type: "text",
                        autofocus: true,
                        placeholder: "Search commands...",
                        value: "{query_value}",
                        class: "h-10 w-full rounded-md border border-slate-700 bg-slate-950 px-3 text-[13px] text-slate-100 placeholder:text-slate-500 outline-none transition-colors focus:border-indigo-500/60 focus:ring-1 focus:ring-indigo-500/30",
                        oninput: move |evt| on_query_change.call(evt.value()),
                        onkeydown: move |evt| {
                            if is_escape_key(&evt.key().to_string()) {
                                evt.prevent_default();
                                on_close.call(());
                            }
                        }
                    }
                }

                div { class: "max-h-[320px] overflow-y-auto p-2",
                    if templates.is_empty() {
                        div { class: "px-3 py-8 text-center text-[12px] text-slate-500", "No matching commands" }
                    } else {
                        for template in templates {
                            button {
                                key: "{template.node_type}",
                                class: "mb-1 flex w-full items-center justify-between rounded-md px-3 py-2 text-left transition-colors hover:bg-slate-800",
                                onclick: move |_| on_pick.call(template.node_type),
                                div { class: "flex min-w-0 flex-col",
                                    span { class: "truncate text-[13px] font-medium text-slate-100", "{template.node_type.label()}" }
                                    span { class: "truncate text-[11px] text-slate-500", "{template.node_type.hint()}" }
                                }
                                span { class: "rounded bg-slate-800 px-2 py-0.5 font-mono text-[10px] text-slate-400", "{template.node_type}" }
                            }
                        }
                    }
                }

                div { class: "border-t border-slate-800 px-4 py-2 text-right text-[11px] text-slate-500",
                    "Press Esc to close"
                }
            }
        }
    }
}
