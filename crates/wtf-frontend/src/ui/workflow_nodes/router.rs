use crate::ui::workflow_nodes::schema::{RouterBranch, RouterConfig};
use dioxus::prelude::*;

fn add_branch(branches: &[RouterBranch]) -> Vec<RouterBranch> {
    let branch_name = format!("Path {}", branches.len() + 1);
    branches
        .iter()
        .cloned()
        .chain(std::iter::once(RouterBranch::new(branch_name)))
        .collect()
}

fn remove_branch_by_id(branches: &[RouterBranch], branch_id: &str) -> Vec<RouterBranch> {
    if branches.len() <= 1 {
        branches.to_vec()
    } else {
        branches
            .iter()
            .filter(|branch| branch.id != branch_id)
            .cloned()
            .collect()
    }
}

fn update_branch_name_by_id(branches: &[RouterBranch], branch_id: &str, name: String) -> Vec<RouterBranch> {
    branches
        .iter()
        .map(|branch| {
            if branch.id == branch_id {
                RouterBranch {
                    id: branch.id.clone(),
                    name: name.clone(),
                    condition: branch.condition.clone(),
                    next_node_id: branch.next_node_id.clone(),
                }
            } else {
                branch.clone()
            }
        })
        .collect()
}

fn update_branch_condition_by_id(branches: &[RouterBranch], branch_id: &str, condition: String) -> Vec<RouterBranch> {
    branches
        .iter()
        .map(|branch| {
            if branch.id == branch_id {
                RouterBranch {
                    id: branch.id.clone(),
                    name: branch.name.clone(),
                    condition: condition.clone(),
                    next_node_id: branch.next_node_id.clone(),
                }
            } else {
                branch.clone()
            }
        })
        .collect()
}

#[derive(Clone)]
pub struct RouterNode {
    pub config: Signal<RouterConfig>,
}

impl RouterNode {
    pub fn new() -> Self {
        Self {
            config: use_signal(|| RouterConfig {
                branches: vec![RouterBranch::new("Path 1".to_string())],
            }),
        }
    }

    pub fn from_config(config: RouterConfig) -> Self {
        Self {
            config: use_signal(|| config),
        }
    }
}

impl Default for RouterNode {
    fn default() -> Self {
        Self::new()
    }
}

#[component]
pub fn RouterForm(config: Signal<RouterConfig>) -> Element {
    let branches = config.read().branches.clone();
    let branch_count = branches.len();
    let can_remove_branch = branch_count > 1;

    rsx! {
        div {
            class: "space-y-4",
            div {
                class: "bg-pink-50 border border-pink-200 rounded-lg p-3 mb-4",
                p {
                    class: "text-sm text-pink-800",
                    "🔀 ",
                    strong { "Split" },
                    " - Go to different steps based on a condition."
                }
            }

            for (idx, branch) in branches.iter().enumerate() {
                let branch_id_for_remove = branch.id.clone();
                let branch_id_for_name = branch.id.clone();
                let branch_id_for_condition = branch.id.clone();
                div {
                    key: "{branch.id}",
                    class: "border border-gray-200 rounded-lg p-3 mb-2",
                    div {
                        class: "flex items-center justify-between mb-2",
                        h4 { class: "font-medium text-gray-700", "Path {idx + 1}" }
                        button {
                            class: if can_remove_branch { "text-red-500 text-sm hover:underline" } else { "text-red-300 text-sm cursor-not-allowed" },
                            disabled: !can_remove_branch,
                            onclick: move |_| {
                                config.write().branches = remove_branch_by_id(&config.read().branches, branch_id_for_remove.as_str());
                            },
                            "Remove"
                        }
                    }

                    div {
                        class: "form-field",
                        label { class: "block text-sm font-medium text-gray-700 mb-1", "Name this path" }
                        input {
                            r#type: "text",
                            class: "w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-pink-500",
                            placeholder: "e.g., If approved, If rejected",
                            value: "{branch.name}",
                            oninput: move |e| {
                                config.write().branches = update_branch_name_by_id(&config.read().branches, branch_id_for_name.as_str(), e.value().clone());
                            }
                        }
                    }

                    div {
                        class: "form-field",
                        label { class: "block text-sm font-medium text-gray-700 mb-1", "When to take this path?" }
                        input {
                            r#type: "text",
                            class: "w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-pink-500",
                            placeholder: "{{ steps.approve.status }} == 'approved'",
                            value: "{branch.condition}",
                            oninput: move |e| {
                                config.write().branches = update_branch_condition_by_id(&config.read().branches, branch_id_for_condition.as_str(), e.value().clone());
                            }
                        }
                        p { class: "text-xs text-gray-500 mt-1", "Use {{ step.field }} to check values" }
                    }
                }
            }

            button {
                class: "w-full py-2 border-2 border-dashed border-gray-300 rounded-lg text-gray-500 hover:border-gray-400 hover:text-gray-600",
                onclick: move |_| config.write().branches = add_branch(&config.read().branches),
                "+ Add Another Path"
            }

            if !can_remove_branch {
                p { class: "text-xs text-gray-500 mt-1", "At least one path is required." }
            }

            if branch_count == 0 {
                div {
                    class: "bg-gray-50 p-3 rounded-lg mt-2",
                    p { class: "text-sm text-gray-600", "💡 Use this to check conditions and go different directions, like an if/else." }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{add_branch, remove_branch_by_id, update_branch_condition_by_id, update_branch_name_by_id};
    use crate::ui::workflow_nodes::schema::RouterBranch;

    fn branch(id: &str, name: &str, condition: &str) -> RouterBranch {
        RouterBranch {
            id: id.to_string(),
            name: name.to_string(),
            condition: condition.to_string(),
            next_node_id: None,
        }
    }

    #[test]
    fn remove_branch_keeps_one_minimum() {
        let branches = vec![branch("a", "Path 1", "x")];
        let result = remove_branch_by_id(&branches, "a");
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn update_branch_name_uses_id() {
        let branches = vec![branch("a", "Path 1", "x"), branch("b", "Path 2", "y")];
        let result = update_branch_name_by_id(&branches, "b", "Updated".to_string());
        assert_eq!(result[0].name, "Path 1");
        assert_eq!(result[1].name, "Updated");
    }

    #[test]
    fn update_branch_condition_uses_id() {
        let branches = vec![branch("a", "Path 1", "x")];
        let result = update_branch_condition_by_id(&branches, "a", "z".to_string());
        assert_eq!(result[0].condition, "z");
    }

    #[test]
    fn add_branch_appends_new_branch() {
        let branches = vec![branch("a", "Path 1", "x")];
        let result = add_branch(&branches);
        assert_eq!(result.len(), 2);
        assert_eq!(result[1].name, "Path 2");
    }
}

#[component]
pub fn RouterNodeCard() -> Element {
    rsx! {
        div {
            class: "flex items-center gap-3 p-3 bg-white border border-gray-200 rounded-lg shadow-sm hover:shadow-md transition-shadow",
            div { class: "w-10 h-10 bg-pink-100 rounded-full flex items-center justify-center", span { class: "text-xl", "🔀" } },
            div {
                class: "flex-1",
                h3 { class: "font-medium text-gray-900", "Router" }
                p { class: "text-sm text-gray-500", "Go different ways based on conditions" }
            }
        }
    }
}
