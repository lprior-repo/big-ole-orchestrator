use dioxus::prelude::*;
use regex::Regex;
use crate::ui::graph::{Node, NodeCategory, NodeId};

#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    pub node_id: NodeId,
    pub name: String,
    pub kind_name: String,
    pub category: NodeCategory,
    pub match_type: MatchType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MatchType {
    Name,
    NameRegex,
    Kind,
    KindRegex,
}

pub fn filter_nodes_by_query(
    nodes: &[Node],
    query: &str,
) -> Vec<SearchResult> {
    let query = query.trim();
    if query.is_empty() {
        return Vec::new();
    }

    let is_regex = query.starts_with('/') && query.ends_with('/') && query.len() > 2;
    let search_pattern = if is_regex {
        &query[1..query.len()-1]
    } else {
        query
    };

    let case_insensitive = search_pattern.chars().all(|c| c.is_lowercase() || !c.is_alphabetic());

    let regex_result: Option<Regex> = if is_regex {
        Regex::new(search_pattern).ok()
    } else if case_insensitive {
        Regex::new(&format!("(?i){}", regex::escape(search_pattern))).ok()
    } else {
        Regex::new(&regex::escape(search_pattern)).ok()
    };

    nodes
        .iter()
        .filter_map(|node| {
            let (name_matches, name_match_type) = if let Some(ref re) = regex_result {
                if is_regex {
                    if re.is_match(&node.name) {
                        (true, MatchType::NameRegex)
                    } else {
                        (false, MatchType::Name)
                    }
                } else {
                    if re.is_match(&node.name) {
                        (true, MatchType::Name)
                    } else {
                        (false, MatchType::Name)
                    }
                }
            } else {
                (false, MatchType::Name)
            };

            let kind_name = format!("{:?}", node.kind).to_lowercase();
            let (kind_matches, kind_match_type) = if let Some(ref re) = regex_result {
                if re.is_match(&kind_name) || re.is_match(&node.category.to_string()) {
                    (true, if is_regex { MatchType::KindRegex } else { MatchType::Kind })
                } else {
                    (false, MatchType::Kind)
                }
            } else {
                let pattern_lower = search_pattern.to_lowercase();
                if kind_name.contains(&pattern_lower) || node.category.to_string().contains(&pattern_lower) {
                    (true, MatchType::Kind)
                } else {
                    (false, MatchType::Kind)
                }
            };

            if name_matches || kind_matches {
                Some(SearchResult {
                    node_id: node.id.clone(),
                    name: node.name.clone(),
                    kind_name,
                    category: node.category,
                    match_type: if name_matches { name_match_type } else { kind_match_type },
                })
            } else {
                None
            }
        })
        .collect()
}

fn is_escape_key(key: &str) -> bool {
    let key_lower = key.to_lowercase();
    key_lower == "escape" || key_lower == "esc"
}

#[component]
pub fn NodeSearchPanel(
    open: ReadSignal<bool>,
    query: ReadSignal<String>,
    nodes: ReadSignal<Vec<Node>>,
    on_query_change: EventHandler<String>,
    on_close: EventHandler<()>,
    on_select: EventHandler<NodeId>,
) -> Element {
    if !*open.read() {
        return rsx! {};
    }

    let query_value = query.read().to_string();
    let results = use_memo(move || {
        filter_nodes_by_query(nodes.read().as_slice(), &query_value)
    });

    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-start justify-center bg-slate-950/45 p-4 backdrop-blur-sm pt-20",
            onclick: move |_| on_close.call(()),
            div {
                class: "w-full max-w-md overflow-hidden rounded-xl border border-slate-700/70 bg-slate-900/95 shadow-2xl",
                onclick: move |evt| evt.stop_propagation(),
                div {
                    class: "flex items-center justify-between border-b border-slate-800 px-4 py-3",
                    h2 { class: "text-[14px] font-semibold text-slate-100", "Search Nodes" }
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
                        placeholder: "Search by name or type (use /regex/ for patterns)",
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
                    if query_value.trim().is_empty() {
                        div { class: "px-3 py-8 text-center text-[12px] text-slate-500", "Start typing to search nodes..." }
                    } else if results.read().is_empty() {
                        div { class: "px-3 py-8 text-center text-[12px] text-slate-500", "No matching nodes found" }
                    } else {
                        for result in results.read().iter() {
                            button {
                                key: "{result.node_id}",
                                class: "mb-1 flex w-full items-center justify-between rounded-md px-3 py-2 text-left transition-colors hover:bg-slate-800",
                                onclick: move |_| on_select.call(result.node_id.clone()),
                                div { class: "flex min-w-0 flex-col",
                                    span { class: "truncate text-[13px] font-medium text-slate-100", "{result.name}" }
                                    span { class: "truncate text-[11px] text-slate-500", "kind: {result.kind_name} · {result.category}" }
                                }
                                span { class: "rounded bg-slate-800 px-2 py-0.5 font-mono text-[10px] text-slate-400",
                                    match result.match_type {
                                        MatchType::NameRegex | MatchType::KindRegex => "regex",
                                        _ => ""
                                    }
                                }
                            }
                        }
                    }
                }

                div { class: "border-t border-slate-800 px-4 py-2 text-right text-[11px] text-slate-500",
                    "Press Esc to close · Click to select"
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::graph::{Node, NodeCategory, NodeId, NodeKind};

    fn make_node(name: &str, kind: NodeKind) -> Node {
        Node::new(NodeId::new(), name.to_string(), kind)
    }

    #[test]
    fn given_empty_query_when_filtering_then_returns_empty() {
        let nodes = vec![
            make_node("HTTP Handler", NodeKind::Pure),
            make_node("DB Query", NodeKind::ManagedEffect),
        ];
        let results = filter_nodes_by_query(&nodes, "");
        assert!(results.is_empty());
    }

    #[test]
    fn given_whitespace_query_when_filtering_then_returns_empty() {
        let nodes = vec![
            make_node("HTTP Handler", NodeKind::Pure),
        ];
        let results = filter_nodes_by_query(&nodes, "   ");
        assert!(results.is_empty());
    }

    #[test]
    fn given_name_match_when_filtering_then_returns_matching_nodes() {
        let nodes = vec![
            make_node("HTTP Handler", NodeKind::Pure),
            make_node("DB Query", NodeKind::ManagedEffect),
            make_node("HTTP POST", NodeKind::Pure),
        ];
        let results = filter_nodes_by_query(&nodes, "HTTP");
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.name.contains("HTTP")));
    }

    #[test]
    fn given_kind_match_when_filtering_then_returns_matching_nodes() {
        let nodes = vec![
            make_node("HTTP Handler", NodeKind::Pure),
            make_node("DB Query", NodeKind::ManagedEffect),
            make_node("Timer", NodeKind::Wait),
        ];
        let results = filter_nodes_by_query(&nodes, "managed");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "DB Query");
    }

    #[test]
    fn given_case_insensitive_name_match_when_filtering_then_returns_matching_nodes() {
        let nodes = vec![
            make_node("http handler", NodeKind::Pure),
            make_node("HTTP Handler", NodeKind::Pure),
        ];
        let results = filter_nodes_by_query(&nodes, "HTTP");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn given_regex_pattern_when_filtering_then_returns_matching_nodes() {
        let nodes = vec![
            make_node("node-001", NodeKind::Pure),
            make_node("node-002", NodeKind::Pure),
            make_node("data-001", NodeKind::ManagedEffect),
        ];
        let results = filter_nodes_by_query(&nodes, "/node-\\d+/");
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.name.starts_with("node-")));
    }

    #[test]
    fn given_invalid_regex_when_filtering_then_returns_empty() {
        let nodes = vec![
            make_node("node-001", NodeKind::Pure),
        ];
        let results = filter_nodes_by_query(&nodes, "/[invalid/");
        assert!(results.is_empty());
    }

    #[test]
    fn given_category_match_when_filtering_then_returns_matching_nodes() {
        let nodes = vec![
            make_node("Entry Node", NodeKind::Pure),
            make_node("Timer", NodeKind::Wait),
        ];
        let mut entry_node = nodes[0].clone();
        entry_node.category = NodeCategory::Entry;

        let all_nodes = vec![entry_node, nodes[1].clone()];
        let results = filter_nodes_by_query(&all_nodes, "entry");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Entry Node");
    }

    #[test]
    fn given_no_match_when_filtering_then_returns_empty() {
        let nodes = vec![
            make_node("HTTP Handler", NodeKind::Pure),
            make_node("DB Query", NodeKind::ManagedEffect),
        ];
        let results = filter_nodes_by_query(&nodes, "xyz123");
        assert!(results.is_empty());
    }
}