#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![forbid(unsafe_code)]

use dioxus::prelude::*;
use oya_frontend::graph::ExecutionState;

/// Returns the Tailwind background + border classes for the bar.
#[must_use]
pub const fn bar_bg_class(status: ExecutionState, frozen: bool) -> &'static str {
    if frozen {
        return "bg-amber-50 border-b border-amber-200";
    }
    match status {
        ExecutionState::Running | ExecutionState::Queued => "bg-blue-50 border-b border-blue-200",
        ExecutionState::Completed => "bg-green-50 border-b border-green-200",
        ExecutionState::Failed => "bg-red-50 border-b border-red-200",
        ExecutionState::Idle | ExecutionState::Skipped => "bg-slate-50 border-b border-slate-200",
    }
}

/// Returns the human-readable status text (non-frozen states only).
#[must_use]
pub fn status_text(status: ExecutionState, step: usize, total: usize, name: &str) -> String {
    match status {
        ExecutionState::Running | ExecutionState::Queued => {
            format!("Running step {step} of {total} \u{2014} {name}")
        }
        ExecutionState::Completed => format!("Completed {total} steps"),
        ExecutionState::Failed => format!("Failed at step {step} \u{2014} {name}"),
        ExecutionState::Idle | ExecutionState::Skipped => "Ready".to_string(),
    }
}

#[component]
pub fn RunStatusBar(
    current_step: ReadSignal<usize>,
    total_steps: ReadSignal<usize>,
    current_step_name: ReadSignal<String>,
    overall_status: ReadSignal<ExecutionState>,
    is_frozen_mode: ReadSignal<bool>,
    frozen_run_id: ReadSignal<Option<String>>,
    on_exit_frozen: EventHandler<()>,
) -> Element {
    let frozen = *is_frozen_mode.read();
    let status = *overall_status.read();
    let step = *current_step.read();
    let total = *total_steps.read();
    let name = current_step_name.read();
    let bg = bar_bg_class(status, frozen);

    rsx! {
        div {
            class: "flex h-8 w-full items-center px-4 text-[12px] {bg}",

            if frozen {
                // Frozen mode: show snowflake, run ID, and exit button
                span { class: "mr-2 text-amber-600", "\u{2744}" }
                span { class: "text-amber-700 font-medium",
                    {
                        let run_id = frozen_run_id.read();
                    run_id.as_deref().map_or_else(
                        || "Viewing historical run".to_string(),
                        |id| format!("Viewing historical run {id}"),
                    )
                    }
                }
                button {
                    class: "ml-3 rounded px-2 py-0.5 text-[11px] font-semibold bg-amber-200 text-amber-800 hover:bg-amber-300 transition-colors",
                    onclick: move |_| on_exit_frozen.call(()),
                    "Exit"
                }
            } else {
                match status {
                    ExecutionState::Running | ExecutionState::Queued => {
                        rsx! {
                            span { class: "mr-2 inline-block h-2 w-2 rounded-full bg-blue-500 animate-pulse" }
                            span { class: "text-blue-700 font-medium",
                                { status_text(status, step, total, &name) }
                            }
                        }
                    }
                    ExecutionState::Completed => {
                        rsx! {
                            span { class: "mr-2 text-green-600 font-bold", "\u{2713}" }
                            span { class: "text-green-700 font-medium",
                                { status_text(status, step, total, &name) }
                            }
                        }
                    }
                    ExecutionState::Failed => {
                        rsx! {
                            span { class: "mr-2 text-red-600 font-bold", "\u{2717}" }
                            span { class: "text-red-700 font-medium",
                                { status_text(status, step, total, &name) }
                            }
                        }
                    }
                    ExecutionState::Idle | ExecutionState::Skipped => {
                        rsx! {
                            span { class: "mr-1.5 text-slate-400", "\u{25cf}" }
                            span { class: "text-slate-500",
                                { status_text(status, step, total, &name) }
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
    use super::*;

    // --- bar_bg_class ---

    #[test]
    fn given_idle_status_not_frozen_when_bg_class_then_slate() {
        assert_eq!(
            bar_bg_class(ExecutionState::Idle, false),
            "bg-slate-50 border-b border-slate-200"
        );
    }

    #[test]
    fn given_running_status_not_frozen_when_bg_class_then_blue() {
        assert_eq!(
            bar_bg_class(ExecutionState::Running, false),
            "bg-blue-50 border-b border-blue-200"
        );
    }

    #[test]
    fn given_waiting_status_not_frozen_when_bg_class_then_blue() {
        assert_eq!(
            bar_bg_class(ExecutionState::Queued, false),
            "bg-blue-50 border-b border-blue-200"
        );
    }

    #[test]
    fn given_succeeded_status_not_frozen_when_bg_class_then_green() {
        assert_eq!(
            bar_bg_class(ExecutionState::Completed, false),
            "bg-green-50 border-b border-green-200"
        );
    }

    #[test]
    fn given_failed_status_not_frozen_when_bg_class_then_red() {
        assert_eq!(
            bar_bg_class(ExecutionState::Failed, false),
            "bg-red-50 border-b border-red-200"
        );
    }

    #[test]
    fn given_skipped_status_not_frozen_when_bg_class_then_slate() {
        assert_eq!(
            bar_bg_class(ExecutionState::Skipped, false),
            "bg-slate-50 border-b border-slate-200"
        );
    }

    #[test]
    fn given_any_status_frozen_true_when_bg_class_then_amber() {
        for status in [
            ExecutionState::Idle,
            ExecutionState::Running,
            ExecutionState::Completed,
            ExecutionState::Failed,
            ExecutionState::Skipped,
            ExecutionState::Queued,
        ] {
            assert_eq!(
                bar_bg_class(status, true),
                "bg-amber-50 border-b border-amber-200",
                "frozen=true should always return amber for status {status:?}"
            );
        }
    }

    // --- status_text ---

    #[test]
    fn given_idle_when_status_text_then_ready() {
        assert_eq!(status_text(ExecutionState::Idle, 0, 0, ""), "Ready");
    }

    #[test]
    fn given_skipped_when_status_text_then_ready() {
        assert_eq!(status_text(ExecutionState::Skipped, 1, 5, "foo"), "Ready");
    }

    #[test]
    fn given_running_when_status_text_then_running_format() {
        assert_eq!(
            status_text(ExecutionState::Running, 2, 5, "fetch-data"),
            "Running step 2 of 5 \u{2014} fetch-data"
        );
    }

    #[test]
    fn given_waiting_when_status_text_then_running_format() {
        assert_eq!(
            status_text(ExecutionState::Queued, 1, 3, "init"),
            "Running step 1 of 3 \u{2014} init"
        );
    }

    #[test]
    fn given_succeeded_when_status_text_then_completed_format() {
        assert_eq!(
            status_text(ExecutionState::Completed, 5, 5, "done"),
            "Completed 5 steps"
        );
    }

    #[test]
    fn given_failed_when_status_text_then_failed_format() {
        assert_eq!(
            status_text(ExecutionState::Failed, 3, 7, "transform"),
            "Failed at step 3 \u{2014} transform"
        );
    }

    #[test]
    fn given_step_name_with_spaces_when_running_then_name_preserved() {
        assert_eq!(
            status_text(ExecutionState::Running, 1, 1, "My Complex Step"),
            "Running step 1 of 1 \u{2014} My Complex Step"
        );
    }

    #[test]
    fn given_step_name_empty_when_failed_then_dash_separator_present() {
        let text = status_text(ExecutionState::Failed, 2, 4, "");
        assert!(text.starts_with("Failed at step 2"));
        assert!(text.contains('\u{2014}'));
    }
}
