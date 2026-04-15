#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![forbid(unsafe_code)]

use dioxus::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActionType {
    Compensate,
}

#[component]
pub fn OperatorActionPanel(on_action: EventHandler<ActionType>) -> Element {
    let mut show_confirmation: Signal<bool> = use_signal(|| false);

    rsx! {
        div {
            class: "flex items-center gap-2 p-3 border-t border-slate-200 bg-white",

            button {
                class: "flex items-center gap-2 rounded-md bg-amber-50 px-3 py-1.5 text-[12px] font-medium text-amber-700 border border-amber-200 hover:bg-amber-100 transition-colors",
                onclick: move |_| {
                    show_confirmation.set(true);
                },
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "h-3.5 w-3.5",
                    path { d: "M13 2L3 14h9l-1 8 10-12h-9l1-8z" }
                }
                "Compensate"
            }

            if *show_confirmation.read() {
                ConfirmationModal {
                    action_type: ActionType::Compensate,
                    on_confirm: {
                        let on_action = on_action;
                        move |action| {
                            on_action.call(action);
                            show_confirmation.set(false);
                        }
                    },
                    on_cancel: move |()| {
                        show_confirmation.set(false);
                    }
                }
            }
        }
    }
}

#[component]
fn ConfirmationModal(
    action_type: ActionType,
    on_confirm: EventHandler<ActionType>,
    on_cancel: EventHandler<()>,
) -> Element {
    let action_label = match action_type {
        ActionType::Compensate => "Compensate",
    };

    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/40",
            onclick: move |_| {
                on_cancel.call(());
            },

            div {
                class: "w-[320px] rounded-lg bg-white shadow-xl border border-slate-200",
                onclick: move |evt| {
                    evt.stop_propagation();
                },

                div { class: "flex items-center justify-between border-b border-slate-100 px-4 py-3",
                    h3 { class: "text-[14px] font-semibold text-slate-900", "Confirm Action" }
                    button {
                        class: "flex h-6 w-6 items-center justify-center rounded-md text-slate-400 hover:bg-slate-100 hover:text-slate-600 transition-colors",
                        onclick: move |_| on_cancel.call(()),
                        svg {
                            xmlns: "http://www.w3.org/2000/svg",
                            view_box: "0 0 24 24",
                            fill: "none",
                            stroke: "currentColor",
                            stroke_width: "2",
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            class: "h-4 w-4",
                            line { x1: "18", y1: "6", x2: "6", y2: "18" }
                            line { x1: "6", y1: "6", x2: "18", y2: "18" }
                        }
                    }
                }

                div { class: "p-4",
                    p { class: "text-[13px] text-slate-600",
                        "Are you sure you want to "
                        span { class: "font-medium text-slate-900", "{action_label}" }
                        "?"
                    }
                    p { class: "mt-2 text-[11px] text-slate-500",
                        "This action cannot be undone."
                    }
                }

                div { class: "flex items-center justify-end gap-2 border-t border-slate-100 px-4 py-3",
                    button {
                        class: "rounded-md px-3 py-1.5 text-[12px] font-medium text-slate-600 hover:bg-slate-100 transition-colors",
                        onclick: move |_| on_cancel.call(()),
                        "Cancel"
                    }
                    button {
                        class: "flex items-center gap-1.5 rounded-md bg-amber-500 px-3 py-1.5 text-[12px] font-medium text-white hover:bg-amber-600 transition-colors",
                        onclick: move |_| on_confirm.call(action_type),
                        svg {
                            xmlns: "http://www.w3.org/2000/svg",
                            view_box: "0 0 24 24",
                            fill: "none",
                            stroke: "currentColor",
                            stroke_width: "2",
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            class: "h-3.5 w-3.5",
                            path { d: "M22 11.08V12a10 10 0 1 1-5.93-9.14" }
                            polyline { points: "22 4 12 14.01 9 11.01" }
                        }
                        "{action_label}"
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_type_compensate_display() {
        assert_eq!(format!("{:?}", ActionType::Compensate), "Compensate");
    }

    #[test]
    fn action_type_eq() {
        assert_eq!(ActionType::Compensate, ActionType::Compensate);
    }

    #[test]
    fn action_type_hash() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h1 = DefaultHasher::new();
        let mut h2 = DefaultHasher::new();
        ActionType::Compensate.hash(&mut h1);
        ActionType::Compensate.hash(&mut h2);
        assert_eq!(h1.finish(), h2.finish());
    }
}
