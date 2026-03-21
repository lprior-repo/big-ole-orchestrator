#![allow(dead_code)]

use dioxus::prelude::*;
use serde_json::Value;

#[derive(Clone, PartialEq)]
pub struct NodeInfo {
    pub name: String,
    pub last_output: Option<Value>,
}

#[component]
pub fn ExpressionInput(
    value: String,
    on_change: EventHandler<String>,
    nodes: Vec<NodeInfo>,
    placeholder: Option<String>,
    class: Option<String>,
) -> Element {
    let mut show_dropdown = use_signal(|| false);
    let mut selected_index = use_signal(|| 0usize);
    let mut cached_suggestions = use_signal(Vec::<Suggestion>::new);

    let input_class = class.unwrap_or_else(|| {
        "h-8 w-full rounded-md border border-slate-700 bg-slate-950 px-3 font-mono text-[12px] text-slate-100 outline-none transition-colors focus:border-indigo-500/50 focus:ring-1 focus:ring-indigo-500/30".to_string()
    });

    let trigger_info = parse_trigger(&value);
    let new_phase = match &trigger_info {
        Some(Trigger::NodeStart) => CompletionPhase::NodeName {
            prefix: String::new(),
        },
        Some(Trigger::NodePrefix(prefix)) => CompletionPhase::NodeName {
            prefix: prefix.clone(),
        },
        Some(Trigger::PathStart { node_name }) => CompletionPhase::Path {
            node_name: node_name.clone(),
            path_prefix: String::new(),
        },
        Some(Trigger::PathPrefix { node_name, path }) => CompletionPhase::Path {
            node_name: node_name.clone(),
            path_prefix: path.clone(),
        },
        None => CompletionPhase::None,
    };

    let is_none = matches!(new_phase, CompletionPhase::None);

    let suggestions: Vec<Suggestion> = match &new_phase {
        CompletionPhase::None => vec![],
        CompletionPhase::NodeName { prefix } => nodes
            .iter()
            .filter(|n| n.name.to_lowercase().starts_with(&prefix.to_lowercase()))
            .map(|n| Suggestion::Node(n.clone()))
            .collect(),
        CompletionPhase::Path {
            node_name,
            path_prefix,
        } => nodes
            .iter()
            .find(|n| &n.name == node_name)
            .and_then(|n| n.last_output.as_ref())
            .map(|output| extract_paths(output, path_prefix))
            .unwrap_or_default(),
    };

    let should_show = !is_none && !suggestions.is_empty();

    cached_suggestions.set(suggestions.clone());
    show_dropdown.set(should_show);

    rsx! {
        div { class: "relative",
            input {
                r#type: "text",
                class: "{input_class}",
                placeholder: placeholder.as_deref().unwrap_or("Enter value or $node[\"name\"].path"),
                value: "{value}",
                oninput: move |e| {
                    on_change.call(e.value());
                },
                onkeydown: move |e| {
                    let key = e.key().to_string();
                    if *show_dropdown.read() {
                        match key.as_str() {
                            "ArrowDown" => {
                                e.prevent_default();
                                let suggs = cached_suggestions.read();
                                let max = suggs.len().saturating_sub(1);
                                let current = *selected_index.read();
                                selected_index.set(if current >= max { 0 } else { current + 1 });
                            }
                            "ArrowUp" => {
                                e.prevent_default();
                                let suggs = cached_suggestions.read();
                                let max = suggs.len().saturating_sub(1);
                                let current = *selected_index.read();
                                selected_index.set(if current == 0 { max } else { current - 1 });
                            }
                            "Enter" | "Tab" => {
                                e.prevent_default();
                                let idx = *selected_index.read();
                                let suggs = cached_suggestions.read();
                                if let Some(suggestion) = suggs.get(idx).cloned() {
                                    let current_value = value.clone();
                                    let new_val = apply_suggestion(&current_value, &suggestion);
                                    show_dropdown.set(false);
                                    on_change.call(new_val);
                                }
                            }
                            "Escape" => {
                                e.prevent_default();
                                show_dropdown.set(false);
                            }
                            _ => {}
                        }
                    }
                }
            }

            if *show_dropdown.read() {
                SuggestionDropdown {
                    value: value.clone(),
                    on_change: on_change,
                    show_dropdown,
                    selected_index,
                    suggestions,
                }
            }
        }
    }
}

#[component]
fn SuggestionDropdown(
    value: String,
    on_change: EventHandler<String>,
    mut show_dropdown: Signal<bool>,
    mut selected_index: Signal<usize>,
    suggestions: Vec<Suggestion>,
) -> Element {
    rsx! {
        div {
            class: "absolute left-0 right-0 top-full z-50 mt-1 max-h-48 overflow-y-auto rounded-md border border-slate-700 bg-slate-900 shadow-xl",
            for (idx, suggestion) in suggestions.into_iter().enumerate() {
                {
                    let val = value.clone();
                    rsx! {
                        button {
                            key: "{idx}",
                            class: if idx == *selected_index.read() {
                                "flex w-full items-center gap-2 bg-indigo-500/20 px-3 py-2 text-left text-[12px] text-slate-100"
                            } else {
                                "flex w-full items-center gap-2 px-3 py-2 text-left text-[12px] text-slate-100 hover:bg-slate-800"
                            },
                            onclick: move |_| {
                                let new_val = apply_suggestion(&val, &suggestion);
                                show_dropdown.set(false);
                                on_change.call(new_val);
                            },
                            match &suggestion {
                                Suggestion::Node(node) => rsx! {
                                    span { class: "font-mono text-indigo-300", "$node" }
                                    span { class: "text-slate-400", "[\"" }
                                    span { class: "font-medium text-slate-100", "{node.name}" }
                                    span { class: "text-slate-400", "\"]" }
                                },
                                Suggestion::Path(path) => rsx! {
                                    span { class: "text-slate-400", ".json." }
                                    span { class: "font-mono text-emerald-300", "{path}" }
                                },
                            }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Clone, PartialEq)]
enum CompletionPhase {
    None,
    NodeName {
        prefix: String,
    },
    Path {
        node_name: String,
        path_prefix: String,
    },
}

#[derive(Clone, PartialEq)]
enum Suggestion {
    Node(NodeInfo),
    Path(String),
}

#[derive(Clone)]
enum Trigger {
    NodeStart,
    NodePrefix(String),
    PathStart { node_name: String },
    PathPrefix { node_name: String, path: String },
}

fn parse_trigger(value: &str) -> Option<Trigger> {
    let node_pattern = "$node[\"";
    if let Some(start) = value.rfind(node_pattern) {
        let after_trigger = &value[start + node_pattern.len()..];

        if let Some(quote_pos) = after_trigger.find("\"]") {
            let node_name = &after_trigger[..quote_pos];
            let after_bracket = &after_trigger[quote_pos + 2..];

            if after_bracket.is_empty() {
                return Some(Trigger::PathStart {
                    node_name: node_name.to_string(),
                });
            } else if after_bracket.starts_with(".json.") || after_bracket.starts_with('.') {
                let path_part = after_bracket
                    .strip_prefix(".json.")
                    .unwrap_or_else(|| after_bracket.strip_prefix('.').unwrap_or(""));
                return Some(Trigger::PathPrefix {
                    node_name: node_name.to_string(),
                    path: path_part.to_string(),
                });
            }
            return None;
        }

        if after_trigger.is_empty() {
            return Some(Trigger::NodeStart);
        }
        if !after_trigger.contains('"') && !after_trigger.contains(']') {
            return Some(Trigger::NodePrefix(after_trigger.to_string()));
        }
    }

    None
}

fn apply_suggestion(value: &str, suggestion: &Suggestion) -> String {
    let node_pattern = "$node[\"";

    match suggestion {
        Suggestion::Node(node) => {
            if let Some(start) = value.rfind(node_pattern) {
                let after_trigger = &value[start + node_pattern.len()..];
                if after_trigger.starts_with('"') || after_trigger.is_empty() {
                    let before = &value[..start + node_pattern.len()];
                    return format!("{before}{name}\"]", name = node.name);
                }
                if !after_trigger.contains(']') {
                    let before = &value[..start + node_pattern.len()];
                    return format!("{before}{name}\"]", name = node.name);
                }
            }
            value.to_string()
        }
        Suggestion::Path(path) => {
            if let Some(start) = value.rfind(node_pattern) {
                let after_trigger = &value[start + node_pattern.len()..];
                if let Some(quote_pos) = after_trigger.find("\"]") {
                    let before_bracket = &value[..start + node_pattern.len() + quote_pos + 2];
                    return format!("{before_bracket}.json.{path}");
                }
            }
            value.to_string()
        }
    }
}

fn extract_paths(value: &Value, prefix: &str) -> Vec<Suggestion> {
    let mut paths = Vec::new();
    collect_paths(value, "", prefix, &mut paths);
    paths
}

fn collect_paths(value: &Value, current_path: &str, prefix: &str, paths: &mut Vec<Suggestion>) {
    if let Value::Object(map) = value {
        for (key, child) in map {
            let full_path = if current_path.is_empty() {
                key.clone()
            } else {
                format!("{current_path}.{key}")
            };

            if full_path.to_lowercase().starts_with(&prefix.to_lowercase()) {
                paths.push(Suggestion::Path(full_path.clone()));
            }

            if prefix.is_empty() || full_path.to_lowercase().starts_with(&prefix.to_lowercase()) {
                collect_paths(child, &full_path, "", paths);
            } else {
                collect_paths(child, &full_path, prefix, paths);
            }
        }
    } else if let Value::Array(arr) = value {
        for (idx, child) in arr.iter().enumerate() {
            let full_path = format!("{current_path}[{idx}]");
            if full_path.to_lowercase().starts_with(&prefix.to_lowercase()) {
                paths.push(Suggestion::Path(full_path.clone()));
            }
            collect_paths(child, &full_path, prefix, paths);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{apply_suggestion, extract_paths, parse_trigger, Suggestion, Trigger};
    use serde_json::json;

    #[test]
    fn given_node_prefix_when_parsing_trigger_then_node_prefix_phase_is_returned() {
        let trigger = parse_trigger("hello $node[\"Use");
        assert!(matches!(trigger, Some(Trigger::NodePrefix(prefix)) if prefix == "Use"));
    }

    #[test]
    fn given_node_and_path_prefix_when_parsing_trigger_then_path_prefix_phase_is_returned() {
        let trigger = parse_trigger("$node[\"User\"].json.em");
        assert!(matches!(
            trigger,
            Some(Trigger::PathPrefix { node_name, path }) if node_name == "User" && path == "em"
        ));
    }

    #[test]
    fn given_node_suggestion_when_applying_then_incomplete_node_token_is_replaced() {
        let suggestion = Suggestion::Node(super::NodeInfo {
            name: "Fetcher".to_string(),
            last_output: None,
        });

        let output = apply_suggestion("$node[\"Fet", &suggestion);
        assert_eq!(output, "$node[\"Fetcher\"]");
    }

    #[test]
    fn given_path_suggestion_when_applying_then_json_path_is_appended_after_node_token() {
        let suggestion = Suggestion::Path("user.email".to_string());

        let output = apply_suggestion("$node[\"Fetcher\"]", &suggestion);
        assert_eq!(output, "$node[\"Fetcher\"].json.user.email");
    }

    #[test]
    fn given_nested_object_and_array_when_extracting_paths_then_matching_paths_are_emitted() {
        let value = json!({
            "user": {
                "email": "a@b.dev",
                "phones": [
                    {"kind": "home"}
                ]
            }
        });

        let suggestions = extract_paths(&value, "user");
        let paths: Vec<String> = suggestions
            .into_iter()
            .filter_map(|s| match s {
                Suggestion::Path(path) => Some(path),
                Suggestion::Node(_) => None,
            })
            .collect();

        assert!(paths.iter().any(|p| p == "user.email"));
        assert!(paths.iter().any(|p| p == "user.phones[0].kind"));
    }
}
