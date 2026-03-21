#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use crate::ui::panel_types::{
    CollapseState, RunOutcome, PayloadShape,
    chevron_rotation_class, outcome_badge_style, outcome_icon_class, panel_height_class,
};
use dioxus::prelude::*;
use oya_frontend::graph::{NodeId, RunRecord};
use std::collections::{HashMap, HashSet};

const fn status_badge_classes(outcome: RunOutcome) -> &'static str {
    match outcome {
        RunOutcome::Success => "bg-emerald-50 text-emerald-700 border-emerald-200",
        RunOutcome::Failure => "bg-red-50 text-red-700 border-red-200",
    }
}

const fn status_icon_classes(outcome: RunOutcome) -> &'static str {
    match outcome {
        RunOutcome::Success => "h-3 w-3 text-emerald-500",
        RunOutcome::Failure => "h-3 w-3 text-red-500",
    }
}

fn format_timestamp(ts: &chrono::DateTime<chrono::Utc>) -> String {
    ts.format("%H:%M:%S").to_string()
}

fn format_elapsed(ts: &chrono::DateTime<chrono::Utc>) -> String {
    let elapsed = chrono::Utc::now().signed_duration_since(*ts);
    if elapsed.num_minutes() < 1 {
        "just now".to_string()
    } else if elapsed.num_hours() < 1 {
        format!("{}m ago", elapsed.num_minutes())
    } else if elapsed.num_days() < 1 {
        format!("{}h ago", elapsed.num_hours())
    } else {
        format!("{}d ago", elapsed.num_days())
    }
}

#[must_use]
pub fn truncate_id(id: &uuid::Uuid) -> String {
    let full = id.to_string();
    full.chars().filter(|c| *c != '-').take(8).collect()
}

#[must_use]
pub const fn format_run_status(outcome: RunOutcome) -> &'static str {
    outcome.display_label()
}

#[must_use]
pub const fn run_status_badge_class(outcome: RunOutcome) -> &'static str {
    match outcome {
        RunOutcome::Success => "inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[10px] font-semibold bg-emerald-50 text-emerald-700 border-emerald-200",
        RunOutcome::Failure => "inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[10px] font-semibold bg-red-50 text-red-700 border-red-200",
    }
}

#[must_use]
pub fn format_run_duration(_run: &RunRecord) -> String {
    "—".to_string()
}

#[must_use]
fn truncate_preview(input: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }

    if input.chars().count() <= max_chars {
        return input.to_string();
    }

    if max_chars <= 3 {
        return ".".repeat(max_chars);
    }

    let keep = max_chars - 3;
    let mut preview: String = input.chars().take(keep).collect();
    preview.push_str("...");
    preview
}

#[must_use]
fn is_failed_step_result(result: &serde_json::Value) -> bool {
    match result {
        serde_json::Value::Null => true,
        serde_json::Value::Object(map) => map.get("error").is_some_and(|error_value| {
            match error_value {
                serde_json::Value::Null => false,
                serde_json::Value::String(message) => !message.is_empty(),
                _ => true,
            }
        }),
        _ => false,
    }
}

#[must_use]
fn derive_step_counts(run: &RunRecord) -> (usize, usize) {
    run.results
        .values()
        .fold((0usize, 0usize), |(ok, failed), result| {
            if is_failed_step_result(result) {
                (ok, failed + 1)
            } else {
                (ok + 1, failed)
            }
        })
}

#[cfg(test)]
mod tests {
    use super::{
        derive_step_counts, format_elapsed, format_run_duration, format_run_status,
        run_status_badge_class, status_badge_classes, truncate_id, truncate_preview,
        RunOutcome,
    };
    use oya_frontend::graph::{NodeId, RunRecord};
    use std::collections::HashMap;
    use uuid::Uuid;

    fn make_run(outcome: RunOutcome) -> RunRecord {
        RunRecord {
            id: Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            results: HashMap::new(),
            success: outcome.is_success(),
        }
    }

    #[test]
    fn given_recent_timestamp_when_formatting_elapsed_then_it_returns_just_now() {
        let timestamp = chrono::Utc::now() - chrono::Duration::seconds(30);
        assert_eq!(format_elapsed(&timestamp), "just now");
    }

    #[test]
    fn given_hour_old_timestamp_when_formatting_elapsed_then_it_returns_hours_ago() {
        let timestamp = chrono::Utc::now() - chrono::Duration::hours(2);
        assert_eq!(format_elapsed(&timestamp), "2h ago");
    }

    #[test]
    fn given_success_outcome_when_requesting_badge_classes_then_success_classes_are_returned() {
        assert_eq!(
            status_badge_classes(RunOutcome::Success),
            "bg-emerald-50 text-emerald-700 border-emerald-200"
        );
    }

    #[test]
    fn given_uuid_when_truncating_then_first_8_hex_chars_are_returned() {
        let id =
            Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap_or_else(|_| Uuid::nil());
        assert_eq!(truncate_id(&id), "550e8400");
    }

    #[test]
    fn given_nil_uuid_when_truncating_then_eight_zeros_are_returned() {
        let id = Uuid::nil();
        assert_eq!(truncate_id(&id), "00000000");
    }

    #[test]
    fn given_success_outcome_when_formatting_status_then_success_label_is_returned() {
        assert_eq!(format_run_status(RunOutcome::Success), "Success");
    }

    #[test]
    fn given_failure_outcome_when_formatting_status_then_failed_label_is_returned() {
        assert_eq!(format_run_status(RunOutcome::Failure), "Failed");
    }

    #[test]
    fn given_success_outcome_when_requesting_badge_class_then_emerald_classes_are_returned() {
        assert!(run_status_badge_class(RunOutcome::Success).contains("emerald"));
    }

    #[test]
    fn given_failure_outcome_when_requesting_badge_class_then_red_classes_are_returned() {
        assert!(run_status_badge_class(RunOutcome::Failure).contains("red"));
    }

    #[test]
    fn given_run_record_when_formatting_duration_then_placeholder_is_returned() {
        let run = make_run(RunOutcome::Success);
        assert_eq!(format_run_duration(&run), "—");
    }

    #[test]
    fn given_multibyte_preview_when_truncating_then_utf8_boundaries_are_preserved() {
        let input = "alpha🙂beta🙂gamma";
        assert_eq!(truncate_preview(input, 10), "alpha🙂b...");
    }

    #[test]
    fn given_short_preview_when_truncating_then_value_is_unchanged() {
        let input = r#"{"ok":true}"#;
        assert_eq!(truncate_preview(input, 30), input);
    }

    #[test]
    fn given_failed_run_without_step_errors_when_deriving_counts_then_steps_are_not_all_failed() {
        let mut results = HashMap::new();
        let _ = results.insert(NodeId::new(), serde_json::json!({"ok": true}));

        let run = RunRecord {
            id: Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            results,
            success: false,
        };

        assert_eq!(derive_step_counts(&run), (1, 0));
    }

    #[test]
    fn given_mixed_step_results_when_deriving_counts_then_ok_and_failed_are_counted_per_result() {
        let mut results = HashMap::new();
        let _ = results.insert(NodeId::new(), serde_json::json!({"value": 1}));
        let _ = results.insert(NodeId::new(), serde_json::json!({"error": "boom"}));
        let _ = results.insert(NodeId::new(), serde_json::json!(null));

        let run = RunRecord {
            id: Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            results,
            success: false,
        };

        assert_eq!(derive_step_counts(&run), (1, 2));
    }
}

#[component]
pub fn ExecutionHistoryTable(
    history: ReadSignal<Vec<RunRecord>>,
    active_run_id: ReadSignal<Option<uuid::Uuid>>,
    on_run_select: EventHandler<uuid::Uuid>,
) -> Element {
    rsx! {
        div { class: "w-full overflow-x-auto",
            table { class: "w-full border-collapse text-left",
                thead {
                    tr { class: "bg-slate-50 border-b border-slate-200",
                        th { class: "text-[11px] font-semibold text-slate-500 uppercase tracking-wide px-3 py-2 border-b border-slate-200", "ID" }
                        th { class: "text-[11px] font-semibold text-slate-500 uppercase tracking-wide px-3 py-2 border-b border-slate-200", "Status" }
                        th { class: "text-[11px] font-semibold text-slate-500 uppercase tracking-wide px-3 py-2 border-b border-slate-200", "Start Time" }
                        th { class: "text-[11px] font-semibold text-slate-500 uppercase tracking-wide px-3 py-2 border-b border-slate-200", "Duration" }
                        th { class: "text-[11px] font-semibold text-slate-500 uppercase tracking-wide px-3 py-2 border-b border-slate-200", "Steps OK" }
                        th { class: "text-[11px] font-semibold text-slate-500 uppercase tracking-wide px-3 py-2 border-b border-slate-200", "Steps Failed" }
                    }
                }
                tbody {
                    for run in history.read().iter().rev() {
                        {
                            let run_id = run.id;
                            let outcome = RunOutcome::from(run.success);
                            let is_active = active_run_id.read().is_some_and(|a| a == run_id);
                            let short_id = truncate_id(&run_id);
                            let status_label = format_run_status(outcome);
                            let badge_class = run_status_badge_class(outcome);
                            let start_time = format_timestamp(&run.timestamp);
                            let duration = format_run_duration(run);
                            let (steps_ok, steps_failed) = derive_step_counts(run);

                            let row_base = "cursor-pointer transition-colors border-b border-slate-100 last:border-b-0";
                            let row_class = if is_active {
                                format!("{row_base} bg-indigo-50 border-l-2 border-indigo-500")
                            } else {
                                format!("{row_base} hover:bg-slate-50")
                            };

                            rsx! {
                                tr {
                                    class: "{row_class}",
                                    key: "{run_id}",
                                    onclick: move |_| { on_run_select.call(run_id); },

                                    td { class: "text-[12px] text-slate-700 px-3 py-2",
                                        span { class: "font-mono", "{short_id}" }
                                    }
                                    td { class: "text-[12px] text-slate-700 px-3 py-2",
                                        span { class: "{badge_class}",
                                            if outcome.is_success() {
                                                crate::ui::icons::CheckIcon { class: "h-2.5 w-2.5" }
                                            } else {
                                                crate::ui::icons::XCircleIcon { class: "h-2.5 w-2.5" }
                                            }
                                            "{status_label}"
                                        }
                                    }
                                    td { class: "text-[12px] text-slate-700 px-3 py-2 font-mono", "{start_time}" }
                                    td { class: "text-[12px] text-slate-700 px-3 py-2", "{duration}" }
                                    td { class: "text-[12px] text-slate-700 px-3 py-2", "{steps_ok}" }
                                    td { class: "text-[12px] text-slate-700 px-3 py-2", "{steps_failed}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn FrozenModeBanner(
    active_run_id: ReadSignal<Option<uuid::Uuid>>,
    on_exit_frozen: EventHandler<()>,
) -> Element {
    let Some(id) = *active_run_id.read() else {
        return rsx! {};
    };
    let short_id = truncate_id(&id);

    rsx! {
        div { class: "flex items-center justify-between px-3 py-2 bg-indigo-50 border-b border-indigo-200 text-[11px]",
            div { class: "flex items-center gap-2",
                div { class: "w-2 h-2 rounded-full bg-indigo-500 animate-pulse" }
                span { class: "text-indigo-700 font-medium",
                    "Viewing historical run "
                    span { class: "font-mono", "{short_id}" }
                    " — Frozen mode"
                }
            }
            button {
                class: "text-indigo-600 hover:text-indigo-800 font-semibold border border-indigo-300 rounded px-2 py-0.5 hover:bg-indigo-100 transition-colors",
                onclick: move |_| { on_exit_frozen.call(()); },
                "Exit frozen mode"
            }
        }
    }
}

#[component]
pub fn ExecutionHistoryPanel(
    history: Memo<Vec<RunRecord>>,
    nodes_by_id: ReadSignal<HashMap<NodeId, oya_frontend::graph::Node>>,
    on_select_node: EventHandler<NodeId>,
    collapsed: Signal<bool>,
    active_run_id: ReadSignal<Option<uuid::Uuid>>,
    on_run_select: EventHandler<uuid::Uuid>,
    on_exit_frozen: EventHandler<()>,
) -> Element {
    let mut expanded_runs: Signal<HashSet<uuid::Uuid>> =
        use_signal(HashSet::new);
    let history_len = history.read().len();
    let collapse_state = CollapseState::from_bool(*collapsed.read());
    let height_class = panel_height_class(collapse_state);
    let chevron_class = chevron_rotation_class(collapse_state);

    let history_signal: ReadSignal<Vec<RunRecord>> = ReadSignal::from(history);

    rsx! {
        aside {
            class: "flex flex-col border-t border-slate-200 bg-white/95 transition-all duration-200 {height_class}",

            div {
                class: "flex items-center justify-between px-3 py-2 border-b border-slate-100",
                button {
                    class: "flex items-center gap-2 text-slate-700 hover:text-slate-900 transition-colors",
                    onclick: move |_| {
                        let _ = collapsed.try_write().map(|mut c| *c = !*c);
                    },
                    crate::ui::icons::ClockIcon { class: "h-4 w-4 text-slate-500" }
                    span { class: "text-[12px] font-semibold", "Execution History" }
                    span { class: "rounded bg-slate-100 px-1.5 py-0.5 text-[10px] text-slate-600", "{history_len}" }
                    div { class: "transition-transform {chevron_class}",
                        crate::ui::icons::ChevronDownIcon { class: "h-3 w-3 text-slate-400" }
                    }
                }
            }

            if !collapse_state.is_collapsed() {
                FrozenModeBanner {
                    active_run_id,
                    on_exit_frozen,
                }

                div { class: "flex-1 overflow-y-auto",
                    if history.read().is_empty() {
                        div { class: "flex flex-col items-center justify-center h-full text-center px-4",
                            crate::ui::icons::ClockIcon { class: "h-8 w-8 text-slate-300 mb-2" }
                            p { class: "text-[12px] text-slate-500", "No executions yet" }
                            p { class: "text-[10px] text-slate-400 mt-1", "Run the workflow to see history" }
                        }
                    } else {
                        ExecutionHistoryTable {
                            history: history_signal,
                            active_run_id,
                            on_run_select,
                        }

                        div { class: "flex flex-col border-t border-slate-100 mt-1",
                            for run in history.read().iter().rev() {
                                {
                                    let run_id = run.id;
                                    let outcome = RunOutcome::from(run.success);
                                    let is_expanded = expanded_runs.read().contains(&run_id);
                                    let status_class = status_badge_classes(outcome);
                                    let icon_class = status_icon_classes(outcome);
                                    let timestamp_str = format_timestamp(&run.timestamp);
                                    let elapsed_str = format_elapsed(&run.timestamp);
                                    let node_count = run.results.len();
                                    let item_chevron_class = chevron_rotation_class(CollapseState::from_bool(!is_expanded));

                                    rsx! {
                                        div {
                                            class: "border-b border-slate-100 last:border-b-0",
                                            key: "{run_id}",

                                            button {
                                                class: "flex w-full items-center gap-2 px-3 py-2 hover:bg-slate-50 transition-colors",
                                                onclick: move |_| {
                                                    let _ = expanded_runs.try_write().map(|mut set| {
                                                        if set.contains(&run_id) {
                                                            set.remove(&run_id);
                                                        } else {
                                                            set.insert(run_id);
                                                        }
                                                    });
                                                },

                                                div { class: "transition-transform {item_chevron_class}",
                                                    crate::ui::icons::ChevronDownIcon { class: "h-3 w-3 text-slate-400" }
                                                }

                                                div { class: "flex-1 flex items-center gap-2",
                                                    span { class: "font-mono text-[11px] text-slate-600", "{timestamp_str}" }
                                                    span { class: "text-[10px] text-slate-400", "{elapsed_str}" }
                                                }

                                                span { class: "text-[10px] text-slate-500", "{node_count} nodes" }

                                                div { class: "flex items-center gap-1 px-1.5 py-0.5 rounded border {status_class}",
                                                    if outcome.is_success() {
                                                        crate::ui::icons::CheckIcon { class: "{icon_class}" }
                                                    } else {
                                                        crate::ui::icons::XCircleIcon { class: "{icon_class}" }
                                                    }
                                                    span { class: "text-[10px] font-medium", "{outcome.display_label()}" }
                                                }
                                            }

                                            if is_expanded {
                                                div { class: "bg-slate-50/50 px-3 pb-2",
                                                    for (node_id, result) in run.results.iter() {
                                                        {
                                                            let node_name = nodes_by_id
                                                                .read()
                                                                .get(node_id)
                                                                .map_or_else(
                                                                    || "Unknown".to_string(),
                                                                    |n| n.name.clone(),
                                                                );
                                                            let node_id_for_click = *node_id;
                                                            let result_preview = match serde_json::to_string(result) {
                                                                Ok(serialized) => serialized,
                                                                Err(_) => "{}".to_string(),
                                                            };
                                                            let truncated_result = truncate_preview(&result_preview, 30);

                                                            rsx! {
                                                                button {
                                                                    class: "flex w-full items-center gap-2 px-2 py-1.5 rounded hover:bg-white transition-colors text-left",
                                                                    key: "{node_id}",
                                                                    onclick: move |_| {
                                                                        on_select_node.call(node_id_for_click);
                                                                    },

                                                                    div { class: "w-1.5 h-1.5 rounded-full bg-indigo-400 shrink-0" }
                                                                    span { class: "text-[11px] text-slate-700 flex-1 truncate", "{node_name}" }
                                                                    span { class: "text-[10px] font-mono text-slate-400 shrink-0",
                                                                        "{truncated_result}"
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
