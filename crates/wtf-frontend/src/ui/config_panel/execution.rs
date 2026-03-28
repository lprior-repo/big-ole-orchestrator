#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]

use super::get_str_val;
use crate::ui::icons::{icon_by_name, CopyIcon};
use crate::ui::panel_types::{
    invocation_badge_style, ExecutionEventCategory, InvocationStatus, OutputOrigin, PayloadShape,
    StatusBadgeStyle,
};
use dioxus::prelude::*;
use oya_frontend::graph::ExecutionState;
use serde_json::Value;
use wasm_bindgen::JsCast;
use web_sys::window;

const PINNED_OUTPUT_KEY: &str = "pinnedOutputSample";
const DEFAULT_PREVIEW_LINES: usize = 10;

fn copy_to_clipboard(text: &str) -> bool {
    if let Some(window) = window() {
        let navigator = window.navigator();
        if let Ok(clipboard) =
            js_sys::Reflect::get(&navigator, &js_sys::JsString::from("clipboard"))
        {
            if let Ok(write_text) =
                js_sys::Reflect::get(&clipboard, &js_sys::JsString::from("writeText"))
            {
                if let Some(write_text_fn) = write_text.dyn_ref::<js_sys::Function>() {
                    write_text_fn.call1(&clipboard, &js_sys::JsString::from(text)).unwrap();
                    return true;
                }
            }
        }
    }
    false
}

#[component]
fn CopyButton(text: String, compact: bool) -> Element {
    let mut show_feedback = use_signal(|| false);

    let btn_class = if compact {
        "flex h-5 w-5 items-center justify-center rounded text-slate-400 transition-colors hover:bg-slate-700 hover:text-slate-200"
    } else {
        "flex h-6 items-center gap-1 rounded px-2 text-[10px] text-slate-400 transition-colors hover:bg-slate-700 hover:text-slate-200"
    };

    rsx! {
        button {
            class: "{btn_class}",
            onclick: move |_| {
                if copy_to_clipboard(&text) {
                    show_feedback.set(true);
                    let mut feedback = show_feedback;
                    wasm_bindgen_futures::spawn_local(async move {
                        gloo_timers::future::TimeoutFuture::new(1500).await;
                        feedback.set(false);
                    });
                }
            },
            if *show_feedback.read() {
                if compact {
                    {icon_by_name("check", "h-3 w-3 text-emerald-400".to_string())}
                } else {
                    span { class: "text-emerald-400", "Copied!" }
                }
            } else {
                CopyIcon { class: if compact { "h-3 w-3" } else { "h-3.5 w-3.5" } }
                if !compact {
                    span { "Copy" }
                }
            }
        }
    }
}

#[component]
fn PayloadPreview(payload: Value, label: String, shape: PayloadShape, max_lines: usize) -> Element {
    let mut expanded = use_signal(|| false);
    let json_full = match serde_json::to_string_pretty(&payload) {
        Ok(serialized) => serialized,
        Err(_) => payload.to_string(),
    };
    let lines: Vec<&str> = json_full.lines().collect();
    let display_text = if *expanded.read() {
        json_full.clone()
    } else {
        json_preview(&payload, max_lines)
    };
    let shape_display = shape.to_display();

    rsx! {
        div { class: "rounded-lg border border-slate-700 bg-slate-900/65 p-2",
            div { class: "mb-1 flex items-center justify-between",
                span { class: "text-[10px] font-medium text-slate-300", "{label}" }
                div { class: "flex items-center gap-2",
                    span { class: "rounded bg-slate-800 px-1.5 py-0.5 text-[9px] text-slate-400", "{shape_display}" }
                    CopyButton { text: json_full.clone(), compact: true }
                }
            }
            pre { class: "overflow-x-auto rounded bg-slate-950 p-2 font-mono text-[10px] leading-relaxed text-slate-300", "{display_text}" }
            if lines.len() > max_lines {
                button {
                    class: "mt-1.5 text-[9px] text-indigo-400 hover:text-indigo-300",
                    onclick: move |_| expanded.toggle(),
                    if *expanded.read() {
                        "Show less"
                    } else {
                        "Show full payload ({lines.len()} lines)"
                    }
                }
            }
        }
    }
}

#[derive(Clone)]
struct ExecutionTimelineEvent {
    category: ExecutionEventCategory,
    label: String,
    detail: String,
}

#[component]
pub(super) fn ExecutionTab(
    config: Value,
    execution_state: ExecutionState,
    execution_data: Value,
    last_output: Option<Value>,
    input_payloads: Vec<Value>,
    on_pin_sample: EventHandler<Option<Value>>,
) -> Element {
    let invocation_status = resolve_invocation_status(execution_state, &execution_data, &config);
    let journal_idx =
        read_u64_with_legacy_fallback(&execution_data, &config, "journal_index", "journalIndex");
    let retry_count =
        read_u64_with_legacy_fallback(&execution_data, &config, "retry_count", "retryCount");
    let timeline = build_execution_timeline(invocation_status, journal_idx, retry_count);
    let pinned_output = get_pinned_output(&config);
    let output_payload = last_output.clone().or_else(|| pinned_output.clone());
    let output_origin = OutputOrigin::from_flags(last_output.is_some(), pinned_output.is_some());

    rsx! {
        div { class: "flex flex-col gap-4",
            div { class: "flex flex-col gap-2",
                label { class: "text-[11px] font-medium uppercase tracking-wide text-slate-500", "Invocation Status" }
                if let Some(status) = invocation_status {
                    StatusBadge { status }
                } else {
                    span { class: "text-[11px] text-slate-500", "Not yet executed" }
                }
            }

            if let Some(idx) = journal_idx {
                div { class: "flex flex-col gap-1",
                    label { class: "text-[11px] font-medium uppercase tracking-wide text-slate-500", "Journal Entry" }
                    div { class: "flex items-center gap-2",
                        span { class: "rounded bg-slate-800 px-2 py-0.5 font-mono text-[11px] text-slate-300", "#{idx}" }
                        span { class: "text-[10px] text-slate-500", "Position in durable execution log" }
                    }
                }
            }

            if let Some(count) = retry_count {
                if count > 0 {
                    div { class: "flex flex-col gap-1",
                        label { class: "text-[11px] font-medium uppercase tracking-wide text-slate-500", "Retry Attempts" }
                        div { class: "flex items-center gap-2",
                            span { class: "rounded bg-red-500/10 px-2 py-0.5 font-mono text-[11px] text-red-400", "{count}" }
                            span { class: "text-[10px] text-slate-500", "Times retried before success/failure" }
                        }
                    }
                }
            }

            div { class: "h-px bg-slate-800" }

            div { class: "flex flex-col gap-2",
                div { class: "flex items-center justify-between",
                    h4 { class: "text-[11px] font-semibold uppercase tracking-wide text-slate-300", "Input Payloads" }
                    span { class: "rounded bg-slate-800 px-1.5 py-0.5 text-[10px] text-slate-400", "{input_payloads.len()}" }
                }
                if input_payloads.is_empty() {
                    p { class: "rounded-lg border border-dashed border-slate-700 bg-slate-800/50 px-3 py-2 text-[11px] text-slate-500", "No upstream payloads available yet." }
                } else {
                    div { class: "flex flex-col gap-2",
                        for (index, payload) in input_payloads.iter().enumerate() {
                            PayloadPreview {
                                payload: payload.clone(),
                                label: format!("Input #{}", index + 1),
                                shape: PayloadShape::from_value(payload),
                                max_lines: DEFAULT_PREVIEW_LINES,
                            }
                        }
                    }
                }
            }

            div { class: "h-px bg-slate-800" }

            div { class: "flex flex-col gap-2",
                div { class: "flex items-center justify-between",
                    h4 { class: "text-[11px] font-semibold uppercase tracking-wide text-slate-300", "Output Payload" }
                    span { class: "rounded bg-slate-800 px-1.5 py-0.5 text-[10px] text-slate-400", "{output_origin.display_label()}" }
                }

                if let Some(output) = output_payload.as_ref() {
                    PayloadPreview {
                        payload: output.clone(),
                        label: "Payload".to_string(),
                        shape: PayloadShape::from_value(output),
                        max_lines: 14,
                    }
                } else {
                    p { class: "rounded-lg border border-dashed border-slate-700 bg-slate-800/50 px-3 py-2 text-[11px] text-slate-500", "Run the workflow to inspect output for this node." }
                }

                div { class: "flex items-center gap-2",
                    button {
                        class: "h-7 rounded-md border border-indigo-500/40 bg-indigo-500/10 px-2.5 text-[10px] font-medium text-indigo-300 transition-colors hover:bg-indigo-500/20 disabled:cursor-not-allowed disabled:opacity-50",
                        disabled: last_output.is_none(),
                        onclick: move |_| {
                            if let Some(output) = last_output.clone() {
                                on_pin_sample.call(Some(output));
                            }
                        },
                        "Pin latest output"
                    }
                    if pinned_output.is_some() {
                        button {
                            class: "h-7 rounded-md border border-slate-600 bg-slate-800/60 px-2.5 text-[10px] font-medium text-slate-300 transition-colors hover:bg-slate-700/60",
                            onclick: move |_| on_pin_sample.call(None),
                            "Unpin"
                        }
                    }
                }
            }

            div { class: "h-px bg-slate-800" }

            div { class: "rounded-lg border border-slate-700 bg-slate-900/65 p-3",
                div { class: "mb-2 flex items-center justify-between",
                    h4 { class: "text-[11px] font-semibold uppercase tracking-wide text-slate-300", "Execution Timeline" }
                    span { class: "rounded bg-slate-800 px-1.5 py-0.5 text-[10px] text-slate-400", "{timeline.len()}" }
                }
                if timeline.is_empty() {
                    p { class: "text-[11px] text-slate-500", "No execution telemetry yet." }
                } else {
                    div { class: "flex flex-col gap-1.5",
                        for event in timeline.iter() {
                            TimelineEventItem { event: event.clone() }
                        }
                    }
                }
            }

            div { class: "rounded-lg border border-dashed border-slate-700 bg-slate-800/50 p-3",
                p { class: "text-[11px] leading-relaxed text-slate-400", "Restate persists each step in a durable journal. On failure, execution replays from the journal, skipping already-completed steps. This ensures exactly-once semantics." }
            }
        }
    }
}

#[component]
fn StatusBadge(status: InvocationStatus) -> Element {
    let style = invocation_badge_style(status);
    let icon_class = if status.is_spinning() {
        "h-3 w-3 animate-spin".to_string()
    } else {
        "h-3 w-3".to_string()
    };

    rsx! {
        div {
            class: "inline-flex self-start items-center gap-1.5 rounded-md border px-2.5 py-1 text-[11px] font-medium {style.bg} {style.text} {style.border}",
            {icon_by_name(status.icon_name(), icon_class)}
            "{status.display_label()}"
        }
    }
}

#[component]
fn TimelineEventItem(event: ExecutionTimelineEvent) -> Element {
    let dot_class = event.category.dot_class();
    let pill_class = event.category.pill_class();
    let label_text = event.category.display_label();

    rsx! {
        div { class: "flex gap-2 rounded-md border border-slate-700 bg-slate-900/80 px-2 py-1.5",
            div { class: "mt-[2px] h-2 w-2 rounded-full {dot_class}" }
            div {
                p { class: "text-[10px] font-medium text-slate-200", "{event.label}" }
                p { class: "text-[10px] text-slate-400", "{event.detail}" }
            }
            span { class: "ml-auto inline-flex h-fit rounded px-1.5 py-0.5 text-[9px] font-medium {pill_class}", "{label_text}" }
        }
    }
}

fn build_execution_timeline(
    status: Option<InvocationStatus>,
    journal_idx: Option<u64>,
    retry_count: Option<u64>,
) -> Vec<ExecutionTimelineEvent> {
    let status_event = status.map(|s| ExecutionTimelineEvent {
        category: ExecutionEventCategory::Status,
        label: "Invocation status updated".to_string(),
        detail: s.as_str().to_string(),
    });

    let journal_event = journal_idx.map(|index| ExecutionTimelineEvent {
        category: ExecutionEventCategory::Journal,
        label: "Durable journal checkpoint".to_string(),
        detail: format!("journal #{index}"),
    });

    let retry_event = retry_count
        .filter(|&retry| retry > 0)
        .map(|retry| ExecutionTimelineEvent {
            category: ExecutionEventCategory::Retry,
            label: "Retry attempts recorded".to_string(),
            detail: format!("{retry} retries"),
        });

    [status_event, journal_event, retry_event]
        .into_iter()
        .flatten()
        .collect()
}

fn get_pinned_output(config: &Value) -> Option<Value> {
    config.get(PINNED_OUTPUT_KEY).cloned()
}

fn runtime_status(
    execution_state: ExecutionState,
    execution_data: &Value,
) -> Option<InvocationStatus> {
    let runtime_status = execution_data
        .get("status")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|status| !status.is_empty())
        .and_then(InvocationStatus::parse);

    match runtime_status {
        Some(status) => Some(status),
        None => match execution_state {
            ExecutionState::Idle => None,
            ExecutionState::Queued => Some(InvocationStatus::Queued),
            ExecutionState::Running => Some(InvocationStatus::Running),
            ExecutionState::Completed => Some(InvocationStatus::Completed),
            ExecutionState::Failed => Some(InvocationStatus::Failed),
            ExecutionState::Skipped => Some(InvocationStatus::Skipped),
        },
    }
}

fn resolve_invocation_status(
    execution_state: ExecutionState,
    execution_data: &Value,
    config: &Value,
) -> Option<InvocationStatus> {
    if let Some(status) = runtime_status(execution_state, execution_data) {
        return Some(status);
    }
    let legacy_status = get_str_val(config, "status");
    InvocationStatus::parse(&legacy_status)
}

fn read_u64_with_legacy_fallback(
    execution_data: &Value,
    config: &Value,
    key: &str,
    legacy_key: &str,
) -> Option<u64> {
    execution_data
        .get(key)
        .and_then(Value::as_u64)
        .or_else(|| execution_data.get(legacy_key).and_then(Value::as_u64))
        .or_else(|| config.get(key).and_then(Value::as_u64))
        .or_else(|| config.get(legacy_key).and_then(Value::as_u64))
}

fn json_preview(payload: &Value, max_lines: usize) -> String {
    let pretty = match serde_json::to_string_pretty(payload) {
        Ok(serialized) => serialized,
        Err(_) => payload.to_string(),
    };
    let lines: Vec<&str> = pretty.lines().collect();
    if lines.len() <= max_lines {
        return pretty;
    }

    format!("{}\n... (truncated)", lines[..max_lines].join("\n"))
}

#[cfg(test)]
mod tests {
    use super::{
        build_execution_timeline, get_pinned_output, json_preview, resolve_invocation_status,
        ExecutionEventCategory, InvocationStatus,
    };
    use oya_frontend::graph::ExecutionState;
    use serde_json::json;

    #[test]
    fn timeline_includes_status_journal_and_retry_when_present() {
        let timeline = build_execution_timeline(Some(InvocationStatus::Retrying), Some(7), Some(2));

        assert_eq!(timeline.len(), 3);
        assert!(matches!(
            timeline[0].category,
            ExecutionEventCategory::Status
        ));
        assert!(timeline[1].detail.contains("#7"));
        assert!(matches!(
            timeline[2].category,
            ExecutionEventCategory::Retry
        ));
    }

    #[test]
    fn timeline_skips_retry_when_zero() {
        let timeline = build_execution_timeline(Some(InvocationStatus::Running), Some(1), Some(0));

        assert_eq!(timeline.len(), 2);
        assert!(!timeline
            .iter()
            .any(|entry| matches!(entry.category, ExecutionEventCategory::Retry)));
    }

    #[test]
    fn pinned_output_is_read_from_config() {
        let config = json!({"pinnedOutputSample": {"ok": true}});
        assert_eq!(get_pinned_output(&config), Some(json!({"ok": true})));
    }

    #[test]
    fn json_preview_truncates_large_payloads() {
        let payload = json!({
            "a": 1, "b": 2, "c": 3, "d": 4, "e": 5,
            "f": 6, "g": 7, "h": 8, "i": 9, "j": 10,
            "k": 11, "l": 12, "m": 13
        });

        let preview = json_preview(&payload, 6);
        assert!(preview.contains("... (truncated)"));
    }

    #[test]
    fn status_prefers_runtime_data_over_other_sources() {
        let status = resolve_invocation_status(
            ExecutionState::Completed,
            &json!({"status": "retrying"}),
            &json!({"status": "failed"}),
        );

        assert_eq!(status, Some(InvocationStatus::Retrying));
    }

    #[test]
    fn status_falls_back_to_execution_state_and_then_legacy_config() {
        let from_state = resolve_invocation_status(
            ExecutionState::Running,
            &json!({}),
            &json!({"status": "failed"}),
        );
        let from_config = resolve_invocation_status(
            ExecutionState::Idle,
            &json!({}),
            &json!({"status": "suspended"}),
        );

        assert_eq!(from_state, Some(InvocationStatus::Running));
        assert_eq!(from_config, Some(InvocationStatus::Suspended));
    }
}
