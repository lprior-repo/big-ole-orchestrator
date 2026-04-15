#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![forbid(unsafe_code)]

use dioxus::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperatorAction {
    Compensate,
}

impl OperatorAction {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Compensate => "Compensate",
        }
    }

    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Compensate => "Reverse the effects of this operation",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperatorActionError {
    pub message: String,
}

impl std::fmt::Display for OperatorActionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for OperatorActionError {}

const MODAL_STYLE: &str = r#"
    "@keyframes fade-in { from { opacity: 0; } to { opacity: 1; } }
    "@keyframes scale-in { from { transform: scale(0.95); opacity: 0; } to { transform: scale(1); opacity: 1; } }
    ".animate-fade-in { animation: fade-in 0.15s ease-out both; }
    ".animate-scale-in { animation: scale-in 0.15s cubic-bezier(0.16, 1, 0.3, 1) both; }
"#;

#[component]
pub fn ConfirmModal(
    visible: bool,
    title: String,
    message: String,
    confirm_label: String,
    cancel_label: String,
    on_confirm: EventHandler<()>,
    on_cancel: EventHandler<()>,
) -> Element {
    if !visible {
        return rsx! {};
    }

    rsx! {
        style { "{MODAL_STYLE}" }

        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/40 animate-fade-in",
            onclick: move |_| { on_cancel.call(()); },

            div {
                class: "animate-scale-in relative w-full max-w-md rounded-xl bg-white shadow-2xl",
                onclick: move |_| { },

                div { class: "flex flex-col gap-1 px-6 pt-5",
                    h3 { class: "text-[15px] font-semibold text-slate-900", "{title}" }
                    p { class: "text-[13px] text-slate-600 leading-relaxed", "{message}" }
                }

                div { class: "flex items-center gap-2 px-6 py-4 mt-2",
                    button {
                        class: "flex-1 flex items-center justify-center gap-1.5 h-9 rounded-lg border border-slate-300 bg-white text-[13px] font-medium text-slate-700 transition-colors hover:bg-slate-50",
                        onclick: move |_| { on_cancel.call(()); },
                        "{cancel_label}"
                    }
                    button {
                        class: "flex-1 flex items-center justify-center gap-1.5 h-9 rounded-lg border border-red-500/30 bg-red-500 text-[13px] font-medium text-white transition-colors hover:bg-red-600",
                        onclick: move |_| { on_confirm.call(()); },
                        crate::ui::icons::AlertTriangleIcon { class: "h-4 w-4" }
                        "{confirm_label}"
                    }
                }
            }
        }
    }
}

#[component]
pub fn OperatorActionPanel(
    on_action: EventHandler<(OperatorAction, Result<(), OperatorActionError>)>,
) -> Element {
    let mut show_compensate_modal: Signal<bool> = use_signal(bool::default);

    rsx! {
        div {
            class: "flex flex-col gap-3 p-4 bg-white rounded-xl border border-slate-200",

            div { class: "flex items-center gap-2",
                crate::ui::icons::ZapIcon { class: "h-4 w-4 text-amber-500" }
                span { class: "text-[12px] font-semibold uppercase tracking-wide text-slate-500",
                    "Operator Actions"
                }
            }

            button {
                class: "flex items-center gap-2 px-3 py-2 rounded-lg border border-amber-200 bg-amber-50 text-[13px] font-medium text-amber-700 transition-colors hover:bg-amber-100 hover:border-amber-300 w-full",
                onclick: move |_| {
                    show_compensate_modal.set(true);
                },

                crate::ui::icons::AlertTriangleIcon { class: "h-4 w-4 text-amber-500" }
                "Compensate"
            }

            ConfirmModal {
                visible: *show_compensate_modal.read(),
                title: "Confirm Compensation".to_string(),
                message: "This will reverse the effects of this operation. This action cannot be undone.".to_string(),
                confirm_label: "Compensate".to_string(),
                cancel_label: "Cancel".to_string(),
                on_confirm: move |_| {
                    show_compensate_modal.set(false);
                    on_action.call((OperatorAction::Compensate, Ok(())));
                },
                on_cancel: move |_| {
                    show_compensate_modal.set(false);
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operator_action_label_returns_correct_text() {
        assert_eq!(OperatorAction::Compensate.label(), "Compensate");
    }

    #[test]
    fn operator_action_description_returns_non_empty() {
        let desc = OperatorAction::Compensate.description();
        assert!(!desc.is_empty());
    }

    #[test]
    fn operator_action_error_display() {
        let err = OperatorActionError {
            message: "test error".to_string(),
        };
        assert_eq!(err.message, "test error");
        assert_eq!(err.to_string(), "test error");
    }

    #[test]
    fn operator_action_panel_renders_without_panic() {
        let mut vdom =
            VirtualDom::new_with_props(OperatorActionPanel, props! { on_action: |_| {} });
        let _ = vdom.rebuild();
    }

    #[test]
    fn confirm_modal_does_not_render_when_invisible() {
        let mut vdom = VirtualDom::new_with_props(
            ConfirmModal,
            props! {
                visible: false,
                title: "Test".to_string(),
                message: "Test message".to_string(),
                confirm_label: "Confirm".to_string(),
                cancel_label: "Cancel".to_string(),
                on_confirm: |_| {},
                on_cancel: |_| {},
            },
        );
        let _ = vdom.rebuild();
        let mutations = vdom.render_immediate_to_vec();
        assert!(mutations.is_empty() || mutations.iter().all(|m| !matches!(m, VNode { .. })));
    }

    #[test]
    fn operator_action_variants_have_unique_labels() {
        let labels = [OperatorAction::Compensate.label()];
        let unique: std::collections::HashSet<_> = labels.iter().collect();
        assert_eq!(labels.len(), unique.len());
    }

    #[test]
    fn operator_action_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<OperatorActionError>();
    }
}
