#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![forbid(unsafe_code)]

use crate::ui::icons::icon_by_name;
use crate::ui::InlineConfigPanel;
use dioxus::prelude::*;
use oya_frontend::graph::{ExecutionState, Node, NodeCategory};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Pure helpers
// ---------------------------------------------------------------------------

/// Returns left-border + ring Tailwind classes driven by execution state.
/// Idle and Waiting are neutral (no extra border treatment beyond category).
#[must_use]
pub const fn node_border_class(state: ExecutionState) -> &'static str {
    match state {
        ExecutionState::Idle | ExecutionState::Queued | ExecutionState::Skipped => "",
        ExecutionState::Running => "border-l-4 border-blue-500 ring-1 ring-blue-300",
        ExecutionState::Completed => "border-l-4 border-green-500 ring-1 ring-green-200",
        ExecutionState::Failed => {
            "border-l-4 border-red-500 ring-1 ring-red-300 shadow-red-500/20 shadow-lg"
        }
    }
}

/// Returns Tailwind classes for the status badge pill background / text / border.
#[must_use]
pub const fn status_badge_class(state: ExecutionState) -> &'static str {
    match state {
        ExecutionState::Idle => "",
        ExecutionState::Queued => {
            "inline-flex items-center gap-1 rounded-full border px-1.5 py-px text-[9px] font-medium leading-none bg-amber-500/15 text-amber-400 border-amber-500/30"
        }
        ExecutionState::Running => {
            "inline-flex items-center gap-1 rounded-full border px-1.5 py-px text-[9px] font-medium leading-none bg-blue-500/15 text-blue-400 border-blue-500/30"
        }
        ExecutionState::Completed => {
            "inline-flex items-center gap-1 rounded-full border px-1.5 py-px text-[9px] font-medium leading-none bg-emerald-500/15 text-emerald-400 border-emerald-500/30"
        }
        ExecutionState::Failed => {
            "inline-flex items-center gap-1 rounded-full border px-1.5 py-px text-[9px] font-medium leading-none bg-red-500/15 text-red-400 border-red-500/30"
        }
        ExecutionState::Skipped => {
            "inline-flex items-center gap-1 rounded-full border px-1.5 py-px text-[9px] font-medium leading-none bg-slate-500/15 text-slate-400 border-slate-500/30 opacity-60"
        }
    }
}

/// Short human-readable label for the status badge.
#[must_use]
pub const fn status_badge_label(state: ExecutionState) -> &'static str {
    match state {
        ExecutionState::Idle => "",
        ExecutionState::Queued => "Waiting",
        ExecutionState::Running => "Running",
        ExecutionState::Completed => "Done",
        ExecutionState::Failed => "Failed",
        ExecutionState::Skipped => "Skipped",
    }
}

/// Returns the first `max_lines` lines of pretty-printed JSON for the given
/// output value, or `None` when there is no output.  Pure - no side effects.
#[must_use]
pub fn output_preview(output: Option<&Value>, max_lines: usize) -> Option<String> {
    let value = output?;
    let pretty = serde_json::to_string_pretty(value).ok()?;
    let lines: Vec<&str> = pretty.lines().take(max_lines).collect();
    if lines.is_empty() {
        return None;
    }
    let truncated = lines.len() < pretty.lines().count();
    let mut result = lines.join("\n");
    if truncated {
        result.push_str("\n...");
    }
    Some(result)
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

#[component]
pub fn FlowNodeComponent(
    node: Node,
    selected: bool,
    inline_open: bool,
    on_mouse_down: EventHandler<MouseEvent>,
    on_click: EventHandler<MouseEvent>,
    on_double_click: EventHandler<MouseEvent>,
    on_handle_mouse_down: EventHandler<(MouseEvent, String)>,
    on_handle_mouse_enter: EventHandler<String>,
    on_handle_mouse_leave: EventHandler<()>,
    on_inline_change: EventHandler<Value>,
    on_inline_close: EventHandler<()>,
) -> Element {
    let category = node.category;
    let icon = node.icon.clone();
    let exec_state = node.execution_state;

    let category_border = match category {
        NodeCategory::Entry => "border-emerald-500/40",
        NodeCategory::Durable => "border-indigo-500/40",
        NodeCategory::State => "border-orange-500/40",
        NodeCategory::Flow => "border-amber-500/40",
        NodeCategory::Timing => "border-pink-500/40",
        NodeCategory::Signal => "border-blue-500/40",
    };

    let category_icon_bg = match category {
        NodeCategory::Entry => "bg-emerald-500/15 text-emerald-400",
        NodeCategory::Durable => "bg-indigo-500/15 text-indigo-500",
        NodeCategory::State => "bg-orange-500/15 text-orange-400",
        NodeCategory::Flow => "bg-amber-500/15 text-amber-400",
        NodeCategory::Timing => "bg-pink-500/15 text-pink-400",
        NodeCategory::Signal => "bg-blue-500/15 text-blue-400",
    };

    let category_accent_bar = match category {
        NodeCategory::Entry => "bg-emerald-500/40",
        NodeCategory::Durable => "bg-indigo-500/40",
        NodeCategory::State => "bg-orange-500/40",
        NodeCategory::Flow => "bg-amber-500/40",
        NodeCategory::Timing => "bg-pink-500/40",
        NodeCategory::Signal => "bg-blue-500/40",
    };

    let exec_border = node_border_class(exec_state);

    let selected_classes = if selected {
        "ring-2 ring-cyan-500/55 border-cyan-500/40 shadow-xl shadow-cyan-500/20"
    } else {
        "hover:border-slate-300"
    };

    // Running nodes retain the subtle outer glow even when not selected.
    let running_glow = if matches!(exec_state, ExecutionState::Running) {
        "shadow-[0_0_0_2px_rgba(6,182,212,0.2)]"
    } else {
        ""
    };

    let z_index = if selected || inline_open { 10 } else { 1 };

    // Output preview: up to 3 lines of pretty JSON.
    let preview = output_preview(node.last_output.as_ref(), 3);

    // Show an inspect hint when the node carries execution data.
    let has_execution_data = node.last_output.is_some()
        || matches!(
            exec_state,
            ExecutionState::Completed | ExecutionState::Failed
        );

    rsx! {
        div {
            "data-node-id": "{node.id}",
            class: "absolute select-none",
            style: "left: {node.x}px; top: {node.y}px; z-index: {z_index};",

            div {
                class: "group relative rounded-xl border bg-gradient-to-b from-white to-slate-50/70 transition-all duration-150 cursor-grab active:cursor-grabbing {category_border} {exec_border} {selected_classes} {running_glow}",
                style: "width: 220px;",
                onmousedown: move |e| {
                    on_mouse_down.call(e);
                },
                onclick: move |e| {
                    on_click.call(e);
                },
                ondoubleclick: move |e| {
                    e.stop_propagation();
                    on_double_click.call(e);
                },

                // ── Input handle (left) ──────────────────────────────────
                div {
                    class: "absolute -left-[5px] top-1/2 -translate-y-1/2 h-[10px] w-[10px] rounded-full border-2 border-slate-300 bg-white hover:bg-blue-500 hover:border-blue-500 hover:scale-125 transition-all duration-150 cursor-ew-resize z-10",
                    onmousedown: move |e| {
                        e.stop_propagation();
                        on_handle_mouse_down.call((e, "target".to_string()));
                    },
                    onmouseenter: move |_| on_handle_mouse_enter.call("target".to_string()),
                    onmouseleave: move |_| on_handle_mouse_leave.call(())
                }

                // ── Header row ───────────────────────────────────────────
                div {
                    class: "absolute inset-x-0 top-0 h-[1px] rounded-t-xl opacity-70",
                    style: "background: linear-gradient(90deg, rgba(14,165,233,0.55), rgba(45,212,191,0.45), rgba(251,191,36,0.45));"
                }

                div { class: "flex items-center gap-3 px-3.5 py-3",
                    div { class: "flex h-8 w-8 shrink-0 items-center justify-center rounded-md {category_icon_bg} shadow-sm",
                        {icon_by_name(&icon, "h-4 w-4".to_string())}
                    }

                    div { class: "flex flex-col gap-0.5 min-w-0 flex-1",
                        span { class: "text-[13px] font-semibold leading-tight text-slate-900 truncate", "{node.name}" }
                        span { class: "text-[11px] leading-tight text-slate-500 truncate", "{node.description}" }
                    }

                    // ── Status badge (top-right) ─────────────────────────
                    div { class: "ml-auto shrink-0",
                        {
                            match exec_state {
                                ExecutionState::Idle => rsx! { div {} },
                                ExecutionState::Queued => rsx! {
                                    span {
                                        class: "{status_badge_class(ExecutionState::Queued)}",
                                        {icon_by_name("clock", "h-2.5 w-2.5".to_string())}
                                        "{status_badge_label(ExecutionState::Queued)}"
                                    }
                                },
                                ExecutionState::Running => rsx! {
                                    span {
                                        class: "{status_badge_class(ExecutionState::Running)}",
                                        // Animated pulse dot
                                        span {
                                            class: "relative flex h-2 w-2",
                                            span {
                                                class: "animate-ping absolute inline-flex h-full w-full rounded-full bg-blue-400 opacity-75"
                                            }
                                            span {
                                                class: "relative inline-flex rounded-full h-2 w-2 bg-blue-500"
                                            }
                                        }
                                        "{status_badge_label(ExecutionState::Running)}"
                                    }
                                },
                                ExecutionState::Completed => rsx! {
                                    span {
                                        class: "{status_badge_class(ExecutionState::Completed)}",
                                        {icon_by_name("check-circle", "h-2.5 w-2.5".to_string())}
                                        "{status_badge_label(ExecutionState::Completed)}"
                                    }
                                },
                                ExecutionState::Failed => rsx! {
                                    span {
                                        class: "{status_badge_class(ExecutionState::Failed)}",
                                        title: "Execution failed",
                                        {icon_by_name("x", "h-2.5 w-2.5".to_string())}
                                        "{status_badge_label(ExecutionState::Failed)}"
                                    }
                                },
                                ExecutionState::Skipped => rsx! {
                                    span {
                                        class: "{status_badge_class(ExecutionState::Skipped)}",
                                        {icon_by_name("x", "h-2.5 w-2.5".to_string())}
                                        "{status_badge_label(ExecutionState::Skipped)}"
                                    }
                                },
                            }
                        }
                    }
                }

                div { class: "-mt-1 flex items-center gap-1.5 px-3.5 pb-2 text-[9px] uppercase tracking-wide text-slate-400",
                    span { class: "rounded bg-white px-1.5 py-px", "{category}" }
                    if inline_open {
                        span { class: "rounded border border-cyan-200 bg-cyan-50 px-1.5 py-px text-cyan-700", "Editing" }
                    }
                }

                // ── Config hint row ──────────────────────────────────────
                if node.execution_data.get("journalIndex").is_some() || node.execution_data.get("retryCount").and_then(serde_json::Value::as_u64) > Some(0) {
                    div { class: "flex items-center gap-2 px-3 pb-2 text-[9px] font-mono text-slate-500",
                        if let Some(idx) = node.execution_data.get("journalIndex").and_then(serde_json::Value::as_u64) {
                            span { class: "rounded bg-slate-100 px-1 py-px", "journal #{idx}" }
                        }
                        if let Some(retries) = node.execution_data.get("retryCount").and_then(serde_json::Value::as_u64) {
                            if retries > 0 {
                                span { class: "rounded bg-red-500/10 text-red-400/70 px-1 py-px", "{retries} retries" }
                            }
                        }
                        if let Some(key) = node.execution_data.get("idempotencyKey").and_then(|i| i.as_str()) {
                            span { class: "rounded bg-slate-100 px-1 py-px truncate max-w-[80px]", title: "{key}", "key: {key}" }
                        }
                    }
                }

                // ── Output preview ───────────────────────────────────────
                if let Some(ref text) = preview {
                    div { class: "px-3 pb-2",
                        pre {
                            class: "text-[9px] font-mono text-slate-400 truncate whitespace-pre-wrap break-all leading-tight",
                            "{text}"
                        }
                    }
                }

                // ── Category accent bar ──────────────────────────────────
                div { class: "h-[2px] rounded-b-lg {category_accent_bar}" }

                // ── Click-to-inspect hint ────────────────────────────────
                if has_execution_data {
                    div {
                        class: "absolute bottom-1 right-2 opacity-0 group-hover:opacity-100 transition-opacity duration-150",
                        title: "Click to inspect execution data",
                        {icon_by_name("info", "h-2.5 w-2.5 text-slate-400".to_string())}
                    }
                }

                // ── Output handle (right) ────────────────────────────────
                div {
                    class: "absolute -right-[5px] top-1/2 -translate-y-1/2 h-[10px] w-[10px] rounded-full border-2 border-slate-300 bg-white hover:bg-blue-500 hover:border-blue-500 hover:scale-125 transition-all duration-150 cursor-ew-resize z-10",
                    onmousedown: move |e| {
                        e.stop_propagation();
                        on_handle_mouse_down.call((e, "source".to_string()));
                    },
                    onmouseenter: move |_| on_handle_mouse_enter.call("source".to_string()),
                    onmouseleave: move |_| on_handle_mouse_leave.call(())
                }
            }

            if inline_open {
                div {
                    class: "mt-1",
                    onclick: move |e| e.stop_propagation(),
                    InlineConfigPanel {
                        node: node.clone(),
                        on_change: on_inline_change,
                        on_close: on_inline_close,
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // -- node_border_class ---------------------------------------------------

    #[test]
    fn given_idle_state_when_border_class_queried_then_empty() {
        assert_eq!(node_border_class(ExecutionState::Idle), "");
    }

    #[test]
    fn given_waiting_state_when_border_class_queried_then_empty() {
        assert_eq!(node_border_class(ExecutionState::Queued), "");
    }

    #[test]
    fn given_skipped_state_when_border_class_queried_then_empty() {
        assert_eq!(node_border_class(ExecutionState::Skipped), "");
    }

    #[test]
    fn given_running_state_when_border_class_queried_then_contains_blue() {
        let class = node_border_class(ExecutionState::Running);
        assert!(class.contains("border-blue-500"), "got: {class}");
        assert!(class.contains("ring-blue-300"), "got: {class}");
    }

    #[test]
    fn given_succeeded_state_when_border_class_queried_then_contains_green() {
        let class = node_border_class(ExecutionState::Completed);
        assert!(class.contains("border-green-500"), "got: {class}");
        assert!(class.contains("ring-green-200"), "got: {class}");
    }

    #[test]
    fn given_failed_state_when_border_class_queried_then_contains_red_and_shadow() {
        let class = node_border_class(ExecutionState::Failed);
        assert!(class.contains("border-red-500"), "got: {class}");
        assert!(class.contains("ring-red-300"), "got: {class}");
        assert!(class.contains("shadow"), "got: {class}");
    }

    // -- status_badge_class --------------------------------------------------

    #[test]
    fn given_idle_state_when_badge_class_queried_then_empty() {
        assert_eq!(status_badge_class(ExecutionState::Idle), "");
    }

    #[test]
    fn given_running_state_when_badge_class_queried_then_contains_blue() {
        let class = status_badge_class(ExecutionState::Running);
        assert!(class.contains("blue"), "got: {class}");
        assert!(class.contains("rounded-full"), "got: {class}");
    }

    #[test]
    fn given_succeeded_state_when_badge_class_queried_then_contains_emerald() {
        let class = status_badge_class(ExecutionState::Completed);
        assert!(class.contains("emerald"), "got: {class}");
    }

    #[test]
    fn given_failed_state_when_badge_class_queried_then_contains_red() {
        let class = status_badge_class(ExecutionState::Failed);
        assert!(class.contains("red"), "got: {class}");
    }

    #[test]
    fn given_waiting_state_when_badge_class_queried_then_contains_amber() {
        let class = status_badge_class(ExecutionState::Queued);
        assert!(class.contains("amber"), "got: {class}");
    }

    #[test]
    fn given_skipped_state_when_badge_class_queried_then_contains_slate_and_opacity() {
        let class = status_badge_class(ExecutionState::Skipped);
        assert!(class.contains("slate"), "got: {class}");
        assert!(class.contains("opacity-60"), "got: {class}");
    }

    // -- status_badge_label --------------------------------------------------

    #[test]
    fn given_idle_state_when_label_queried_then_empty() {
        assert_eq!(status_badge_label(ExecutionState::Idle), "");
    }

    #[test]
    fn given_running_state_when_label_queried_then_running() {
        assert_eq!(status_badge_label(ExecutionState::Running), "Running");
    }

    #[test]
    fn given_succeeded_state_when_label_queried_then_done() {
        assert_eq!(status_badge_label(ExecutionState::Completed), "Done");
    }

    #[test]
    fn given_failed_state_when_label_queried_then_failed() {
        assert_eq!(status_badge_label(ExecutionState::Failed), "Failed");
    }

    #[test]
    fn given_waiting_state_when_label_queried_then_waiting() {
        assert_eq!(status_badge_label(ExecutionState::Queued), "Waiting");
    }

    #[test]
    fn given_skipped_state_when_label_queried_then_skipped() {
        assert_eq!(status_badge_label(ExecutionState::Skipped), "Skipped");
    }

    // -- output_preview ------------------------------------------------------

    #[test]
    fn given_none_output_when_preview_requested_then_returns_none() {
        assert!(output_preview(None, 3).is_none());
    }

    #[test]
    fn given_simple_json_when_preview_requested_then_returns_some() {
        let val = json!({"key": "value"});
        let result = output_preview(Some(&val), 3);
        assert!(result.is_some());
    }

    #[test]
    fn given_multiline_json_when_max_lines_is_3_then_at_most_3_lines_plus_ellipsis() {
        let val = json!({"a": 1, "b": 2, "c": 3, "d": 4, "e": 5});
        let result = output_preview(Some(&val), 3).expect("should produce preview");
        let line_count = result.lines().count();
        // 3 data lines + 1 "..." line = 4
        assert!(line_count <= 4, "too many lines: {line_count}\n{result}");
        assert!(
            result.ends_with("..."),
            "should end with ellipsis\n{result}"
        );
    }

    #[test]
    fn given_short_json_when_max_lines_exceeds_total_then_no_ellipsis() {
        let val = json!({"x": 1});
        let result = output_preview(Some(&val), 10).expect("should produce preview");
        assert!(
            !result.ends_with("..."),
            "should not have ellipsis\n{result}"
        );
    }

    #[test]
    fn given_exactly_max_lines_json_when_preview_requested_then_no_ellipsis() {
        // A flat object serializes to exactly 3 lines: `{`, `  "k": v`, `}`
        let val = json!({"k": 1});
        let result = output_preview(Some(&val), 3).expect("should produce preview");
        assert!(
            !result.ends_with("..."),
            "should not have ellipsis\n{result}"
        );
    }

    #[test]
    fn given_null_json_value_when_preview_requested_then_returns_some() {
        let val = json!(null);
        let result = output_preview(Some(&val), 3);
        assert!(result.is_some());
    }

    #[test]
    fn given_array_json_when_max_lines_is_1_then_single_line_returned() {
        let val = json!([1, 2, 3, 4, 5]);
        let result = output_preview(Some(&val), 1).expect("should produce preview");
        let first_line = result.lines().next().expect("has a line");
        assert_eq!(first_line, "[");
    }
}
