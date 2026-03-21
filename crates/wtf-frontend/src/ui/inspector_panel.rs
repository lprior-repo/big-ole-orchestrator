#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![forbid(unsafe_code)]

use crate::ui::panel_types::{CollapseState, InvocationStatus, invocation_badge_style, chevron_rotation_class};
use dioxus::prelude::*;
use oya_frontend::graph::{ExecutionState, Node};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectorTab {
    Input,
    Output,
}

#[must_use]
pub const fn status_badge_class(state: ExecutionState) -> &'static str {
    match state {
        ExecutionState::Idle | ExecutionState::Queued => {
            "bg-slate-100 text-slate-600 border-slate-200"
        }
        ExecutionState::Running => "bg-blue-100 text-blue-700 border-blue-200",
        ExecutionState::Completed => "bg-green-100 text-green-700 border-green-200",
        ExecutionState::Failed => "bg-red-100 text-red-700 border-red-200",
        ExecutionState::Skipped => "bg-slate-100 text-slate-500 border-slate-200",
    }
}

#[must_use]
pub fn format_duration(ms: Option<i64>) -> String {
    match ms {
        None => "—".to_string(),
        Some(v) if v < 1000 => format!("{v}ms"),
        Some(v) => {
            #[allow(clippy::cast_precision_loss)]
            let secs = v as f64 / 1000.0;
            format!("{secs:.2}s")
        }
    }
}

fn pretty_json(value: &serde_json::Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string())
}

fn filter_lines(text: &str, query: &str) -> String {
    if query.is_empty() {
        return text.to_string();
    }
    let lower_query = query.to_lowercase();
    text.lines()
        .filter(|line| line.to_lowercase().contains(&lower_query))
        .collect::<Vec<_>>()
        .join("\n")
}

#[must_use]
const fn execution_state_label(state: ExecutionState) -> &'static str {
    match state {
        ExecutionState::Idle | ExecutionState::Queued => "pending",
        ExecutionState::Running => "running",
        ExecutionState::Completed => "completed",
        ExecutionState::Failed => "failed",
        ExecutionState::Skipped => "skipped",
    }
}

#[must_use]
fn should_render_failure(state: ExecutionState, error_text: &str) -> bool {
    state == ExecutionState::Failed || !error_text.is_empty()
}

#[component]
fn TabPill(label: &'static str, active: bool, on_click: EventHandler<()>) -> Element {
    let base = "px-3 py-1 rounded-full text-[11px] font-medium transition-colors cursor-pointer";
    let style = if active {
        "bg-slate-800 text-white"
    } else {
        "bg-slate-100 text-slate-600 hover:bg-slate-200"
    };
    rsx! {
        button {
            class: "{base} {style}",
            onclick: move |_| on_click.call(()),
            "{label}"
        }
    }
}

#[component]
fn CopyButton(text: String) -> Element {
    rsx! {
        button {
            class: "flex items-center gap-1 rounded-md border border-slate-200 bg-white px-2 py-1 text-[10px] font-medium text-slate-600 transition-colors hover:bg-slate-50 hover:text-slate-900",
            title: "Copy to clipboard",
            onclick: move |_| {
                let payload = text.clone();
                #[cfg(target_arch = "wasm32")]
                {
                    use web_sys::window;
                    if let Some(w) = window() {
                        let _ = w.navigator().clipboard().write_text(&payload);
                    }
                }
                let _ = payload;
            },
            crate::ui::icons::CopyIcon { class: "h-3 w-3" }
            "Copy"
        }
    }
}

#[allow(clippy::too_many_arguments)]
#[component]
pub fn InspectorPanel(
    node: ReadSignal<Option<Node>>,
    step_input: ReadSignal<Option<serde_json::Value>>,
    step_output: ReadSignal<Option<serde_json::Value>>,
    step_error: ReadSignal<Option<String>>,
    step_stack_trace: ReadSignal<Option<String>>,
    step_start_time: ReadSignal<Option<String>>,
    step_end_time: ReadSignal<Option<String>>,
    step_duration_ms: ReadSignal<Option<i64>>,
    step_attempt: ReadSignal<u32>,
    on_close: EventHandler<()>,
) -> Element {
    const SLIDE_STYLE: &str = "@keyframes slide-in-right { from { transform: translateX(100%); opacity: 0; } to { transform: translateX(0); opacity: 1; } } .animate-slide-in-right { animation: slide-in-right 0.22s cubic-bezier(0.16, 1, 0.3, 1) both; }";

    let mut active_tab: Signal<InspectorTab> = use_signal(|| InspectorTab::Input);
    let mut search_query: Signal<String> = use_signal(String::new);

    let current_node = node.read();
    let Some(ref selected) = *current_node else {
        return rsx! {};
    };

    let exec_state = selected.execution_state;
    let badge_class = status_badge_class(exec_state);
    let state_label = execution_state_label(exec_state);
    let node_name = selected.name.clone();
    let node_type = selected.node_type.clone();

    let start_str = step_start_time
        .read()
        .clone()
        .unwrap_or_else(|| "—".to_string());
    let end_str = step_end_time
        .read()
        .clone()
        .unwrap_or_else(|| "—".to_string());
    let duration_str = format_duration(*step_duration_ms.read());
    let attempt = *step_attempt.read();

    let tab = *active_tab.read();
    let query = search_query.read().clone();

    let input_json_text = step_input
        .read()
        .as_ref()
        .map_or_else(|| "null".to_string(), pretty_json);

    let output_json_text = step_output
        .read()
        .as_ref()
        .map_or_else(|| "null".to_string(), pretty_json);

    let error_text = step_error.read().clone().unwrap_or_default();
    let stack_text = step_stack_trace.read().clone().unwrap_or_default();
    let is_failed = should_render_failure(exec_state, &error_text);

    let copy_text_for_tab = match tab {
        InspectorTab::Input => input_json_text.clone(),
        InspectorTab::Output => {
            if is_failed {
                format!("{error_text}\n\n{stack_text}")
            } else {
                output_json_text.clone()
            }
        }
    };

    let input_display = filter_lines(&input_json_text, &query);
    let output_display = filter_lines(&output_json_text, &query);
    let stack_display = filter_lines(&stack_text, &query);
    let query_display = query;

    rsx! {
        style { "{SLIDE_STYLE}" }

        aside {
            class: "animate-slide-in-right fixed right-0 top-0 z-30 flex h-full w-[420px] flex-col border-l border-slate-200 bg-white shadow-xl",

            div { class: "shrink-0 border-b border-slate-200 px-4 py-3",

                div { class: "mb-2 flex items-center justify-between gap-2",
                    h2 { class: "text-[15px] font-bold leading-tight text-slate-900 truncate",
                        "{node_name}"
                    }
                    button {
                        class: "flex h-7 w-7 shrink-0 items-center justify-center rounded-md text-slate-400 transition-colors hover:bg-slate-100 hover:text-slate-700",
                        title: "Close inspector",
                        onclick: move |_| on_close.call(()),
                        crate::ui::icons::XIcon { class: "h-4 w-4" }
                    }
                }

                div { class: "mb-3 flex items-center gap-2",
                    span {
                        class: "inline-flex items-center rounded-full border px-2.5 py-0.5 text-[11px] font-semibold capitalize {badge_class}",
                        "{state_label}"
                    }
                    span {
                        class: "inline-flex items-center rounded-full border border-slate-200 bg-slate-50 px-2.5 py-0.5 text-[11px] font-mono text-slate-600",
                        "{node_type}"
                    }
                }

                div { class: "grid grid-cols-4 gap-2",
                    div { class: "flex flex-col gap-0.5",
                        span { class: "text-[9px] font-semibold uppercase tracking-wide text-slate-400", "Start" }
                        span { class: "text-[11px] font-mono text-slate-700 truncate", "{start_str}" }
                    }
                    div { class: "flex flex-col gap-0.5",
                        span { class: "text-[9px] font-semibold uppercase tracking-wide text-slate-400", "End" }
                        span { class: "text-[11px] font-mono text-slate-700 truncate", "{end_str}" }
                    }
                    div { class: "flex flex-col gap-0.5",
                        span { class: "text-[9px] font-semibold uppercase tracking-wide text-slate-400", "Duration" }
                        span { class: "text-[11px] font-mono text-slate-700", "{duration_str}" }
                    }
                    div { class: "flex flex-col gap-0.5",
                        span { class: "text-[9px] font-semibold uppercase tracking-wide text-slate-400", "Attempt" }
                        span { class: "text-[11px] font-mono text-slate-700", "#{attempt}" }
                    }
                }
            }

            div { class: "shrink-0 flex items-center gap-2 border-b border-slate-100 px-4 py-2",
                TabPill {
                    label: "Input",
                    active: tab == InspectorTab::Input,
                    on_click: move |()| {
                        active_tab.set(InspectorTab::Input);
                        search_query.set(String::new());
                    },
                }
                TabPill {
                    label: "Output",
                    active: tab == InspectorTab::Output,
                    on_click: move |()| {
                        active_tab.set(InspectorTab::Output);
                        search_query.set(String::new());
                    },
                }
            }

            div { class: "shrink-0 flex items-center gap-2 border-b border-slate-100 px-4 py-2",
                div { class: "relative flex-1",
                    crate::ui::icons::SearchIcon {
                        class: "absolute left-2 top-1/2 h-3 w-3 -translate-y-1/2 text-slate-400 pointer-events-none"
                    }
                    input {
                        class: "h-7 w-full rounded-md border border-slate-200 bg-slate-50 pl-7 pr-3 text-[11px] text-slate-800 outline-none placeholder:text-slate-400 focus:border-blue-400 focus:ring-1 focus:ring-blue-300",
                        r#type: "text",
                        placeholder: "Search payload...",
                        value: "{query_display}",
                        oninput: move |evt| search_query.set(evt.value()),
                    }
                }
                CopyButton { text: copy_text_for_tab }
            }

            div { class: "flex-1 overflow-y-auto p-4",
                match tab {
                    InspectorTab::Input => rsx! {
                        pre {
                            class: "font-mono text-[11px] leading-relaxed overflow-auto whitespace-pre-wrap break-words rounded-md bg-slate-50 p-3 text-slate-800",
                            "{input_display}"
                        }
                    },
                    InspectorTab::Output => {
                        if is_failed {
                            rsx! {
                                if !error_text.is_empty() {
                                    div { class: "mb-3 rounded-md border border-red-200 bg-red-50 p-3",
                                        p { class: "mb-1 text-[11px] font-semibold text-red-700", "Error" }
                                        p { class: "text-[11px] leading-relaxed text-red-800", "{error_text}" }
                                    }
                                }
                                if !stack_text.is_empty() {
                                    div { class: "mb-1 text-[10px] font-semibold uppercase tracking-wide text-slate-500",
                                        "Stack Trace"
                                    }
                                    pre {
                                        class: "font-mono text-[10px] leading-relaxed overflow-auto whitespace-pre-wrap break-words rounded-md bg-slate-50 p-3 text-slate-700",
                                        "{stack_display}"
                                    }
                                }
                                if error_text.is_empty() && stack_text.is_empty() {
                                    p { class: "text-[12px] text-slate-400", "No error details available." }
                                }
                            }
                        } else {
                            rsx! {
                                pre {
                                    class: "font-mono text-[11px] leading-relaxed overflow-auto whitespace-pre-wrap break-words rounded-md bg-slate-50 p-3 text-slate-800",
                                    "{output_display}"
                                }
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
    use super::{
        execution_state_label, filter_lines, format_duration, should_render_failure,
        status_badge_class,
    };
    use oya_frontend::graph::ExecutionState;

    #[test]
    fn given_idle_state_when_getting_badge_class_then_returns_slate() {
        let class = status_badge_class(ExecutionState::Idle);
        assert!(
            class.contains("slate"),
            "expected slate class, got: {class}"
        );
    }

    #[test]
    fn given_queued_state_when_getting_badge_class_then_returns_slate() {
        let class = status_badge_class(ExecutionState::Queued);
        assert!(
            class.contains("slate"),
            "expected slate class, got: {class}"
        );
    }

    #[test]
    fn given_running_state_when_getting_badge_class_then_returns_blue() {
        let class = status_badge_class(ExecutionState::Running);
        assert!(class.contains("blue"), "expected blue class, got: {class}");
    }

    #[test]
    fn given_completed_state_when_getting_badge_class_then_returns_green() {
        let class = status_badge_class(ExecutionState::Completed);
        assert!(
            class.contains("green"),
            "expected green class, got: {class}"
        );
    }

    #[test]
    fn given_failed_state_when_getting_badge_class_then_returns_red() {
        let class = status_badge_class(ExecutionState::Failed);
        assert!(class.contains("red"), "expected red class, got: {class}");
    }

    #[test]
    fn given_skipped_state_when_getting_badge_class_then_returns_slate() {
        let class = status_badge_class(ExecutionState::Skipped);
        assert!(
            class.contains("slate"),
            "expected slate class, got: {class}"
        );
    }

    #[test]
    fn given_queued_state_when_getting_status_label_then_returns_pending() {
        assert_eq!(execution_state_label(ExecutionState::Queued), "pending");
    }

    #[test]
    fn given_completed_state_with_error_text_when_checking_failure_then_true() {
        assert!(should_render_failure(ExecutionState::Completed, "boom"));
    }

    #[test]
    fn given_completed_state_without_error_text_when_checking_failure_then_false() {
        assert!(!should_render_failure(ExecutionState::Completed, ""));
    }

    #[test]
    fn given_none_duration_when_formatting_then_returns_dash() {
        assert_eq!(format_duration(None), "—");
    }

    #[test]
    fn given_zero_ms_when_formatting_then_returns_zero_ms() {
        assert_eq!(format_duration(Some(0)), "0ms");
    }

    #[test]
    fn given_sub_second_duration_when_formatting_then_returns_ms() {
        assert_eq!(format_duration(Some(234)), "234ms");
    }

    #[test]
    fn given_999_ms_when_formatting_then_returns_ms_not_seconds() {
        assert_eq!(format_duration(Some(999)), "999ms");
    }

    #[test]
    fn given_exactly_1000_ms_when_formatting_then_returns_seconds() {
        assert_eq!(format_duration(Some(1000)), "1.00s");
    }

    #[test]
    fn given_1230_ms_when_formatting_then_returns_two_decimal_seconds() {
        assert_eq!(format_duration(Some(1230)), "1.23s");
    }

    #[test]
    fn given_large_duration_when_formatting_then_returns_seconds() {
        assert_eq!(format_duration(Some(60_000)), "60.00s");
    }

    #[test]
    fn given_empty_query_when_filtering_then_returns_all_lines() {
        let text = "foo\nbar\nbaz";
        assert_eq!(filter_lines(text, ""), text);
    }

    #[test]
    fn given_matching_query_when_filtering_then_returns_only_matching_lines() {
        let text = "foo\nbar\nbaz";
        assert_eq!(filter_lines(text, "ba"), "bar\nbaz");
    }

    #[test]
    fn given_case_insensitive_query_when_filtering_then_matches_regardless_of_case() {
        let text = "FooBar\nfoobar\nQUX";
        let result = filter_lines(text, "foobar");
        assert!(result.contains("FooBar"), "should match FooBar");
        assert!(result.contains("foobar"), "should match foobar");
        assert!(!result.contains("QUX"), "should not match QUX");
    }

    #[test]
    fn given_non_matching_query_when_filtering_then_returns_empty_string() {
        let text = "foo\nbar";
        assert_eq!(filter_lines(text, "zzz"), "");
    }
}
