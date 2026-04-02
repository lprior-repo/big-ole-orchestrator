#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![forbid(unsafe_code)]

use crate::ui::panel_types::ValidationResultCategory;
use dioxus::prelude::*;
use oya_frontend::graph::{NodeId, ValidationResult, ValidationSeverity};

use crate::ui::icons::{AlertCircleIcon, AlertTriangleIcon, ChevronDownIcon, ChevronRightIcon};

#[component]
pub fn ValidationPanel(
    validation_result: ReadSignal<ValidationResult>,
    collapsed: Signal<bool>,
    on_select_node: EventHandler<NodeId>,
) -> Element {
    let issue_count = validation_result.read().issues.len();
    let error_count = validation_result.read().error_count();
    let warning_count = validation_result.read().warning_count();
    let category = ValidationResultCategory::from_counts(error_count, warning_count);
    let is_collapsed = *collapsed.read();

    let header_bg = category.header_bg_class();
    let status_text = build_status_text(error_count, warning_count);

    let status_icon: Option<Element> = match category {
        ValidationResultCategory::HasErrors => {
            Some(rsx! { AlertCircleIcon { class: "h-4 w-4 text-red-500" } })
        }
        ValidationResultCategory::HasWarningsOnly => {
            Some(rsx! { AlertTriangleIcon { class: "h-4 w-4 text-amber-500" } })
        }
        ValidationResultCategory::Valid => None,
    };

    let status_text_class = category.status_text_class();

    rsx! {
        div { class: "border-b border-slate-200",
            button {
                class: "flex w-full items-center justify-between px-3 py-2 text-left transition-colors hover:bg-slate-50 {header_bg}",
                onclick: move |_| collapsed.toggle(),
                div { class: "flex items-center gap-2",
                    if is_collapsed {
                        ChevronRightIcon { class: "h-3.5 w-3.5 text-slate-400" }
                    } else {
                        ChevronDownIcon { class: "h-3.5 w-3.5 text-slate-400" }
                    }
                    {status_icon}
                    span {
                        class: "{status_text_class}",
                        "{status_text}"
                    }
                }
                if issue_count > 0 {
                    span {
                        class: "rounded-full px-1.5 py-0.5 text-[10px] font-medium {category.badge_class()}",
                        "{issue_count}"
                    }
                }
            }

            if !is_collapsed && issue_count > 0 {
                div { class: "max-h-[200px] overflow-y-auto border-t border-slate-100 bg-slate-50/50",
                    for issue in validation_result.read().issues.iter() {
                        IssueRow {
                            issue: issue.clone(),
                            on_select_node
                        }
                    }
                }
            }

            if !is_collapsed && issue_count == 0 {
                div { class: "flex items-center justify-center gap-2 border-t border-slate-100 bg-emerald-50/30 px-3 py-4",
                    div { class: "h-2 w-2 rounded-full bg-emerald-400" }
                    span { class: "text-[11px] text-emerald-600", "All checks passed" }
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
struct IssueData {
    node_id: Option<NodeId>,
    severity: ValidationSeverity,
    message: String,
}

#[component]
fn IssueRow(
    issue: oya_frontend::graph::ValidationIssue,
    on_select_node: EventHandler<NodeId>,
) -> Element {
    let node_id = issue.node_id;
    let (border_class, bg_class) = match issue.severity {
        ValidationSeverity::Error => ("border-l-red-400", "bg-red-50/50"),
        ValidationSeverity::Warning => ("border-l-amber-400", "bg-amber-50/50"),
    };

    let icon = match issue.severity {
        ValidationSeverity::Error => {
            rsx! { AlertCircleIcon { class: "h-3.5 w-3.5 text-red-500 shrink-0" } }
        }
        ValidationSeverity::Warning => {
            rsx! { AlertTriangleIcon { class: "h-3.5 w-3.5 text-amber-500 shrink-0" } }
        }
    };

    if let Some(nid) = node_id {
        rsx! {
            button {
                class: "flex w-full items-start gap-2 border-l-2 px-3 py-2 text-left transition-colors hover:bg-white {border_class} {bg_class}",
                onclick: move |_| on_select_node.call(nid),
                {icon}
                span { class: "text-[11px] leading-relaxed text-slate-600", "{issue.message}" }
            }
        }
    } else {
        rsx! {
            div {
                class: "flex w-full items-start gap-2 border-l-2 px-3 py-2 {border_class} {bg_class}",
                {icon}
                span { class: "text-[11px] leading-relaxed text-slate-600", "{issue.message}" }
            }
        }
    }
}

fn build_status_text(error_count: usize, warning_count: usize) -> String {
    match ValidationResultCategory::from_counts(error_count, warning_count) {
        ValidationResultCategory::HasErrors => {
            format!(
                "{} error{}, {} warning{}",
                error_count,
                if error_count == 1 { "" } else { "s" },
                warning_count,
                if warning_count == 1 { "" } else { "s" }
            )
        }
        ValidationResultCategory::HasWarningsOnly => {
            format!(
                "{} warning{}",
                warning_count,
                if warning_count == 1 { "" } else { "s" }
            )
        }
        ValidationResultCategory::Valid => "Workflow valid".to_string(),
    }
}
