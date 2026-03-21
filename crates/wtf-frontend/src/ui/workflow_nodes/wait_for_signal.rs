use crate::ui::workflow_nodes::schema::WaitForSignalConfig;
use dioxus::prelude::*;

#[component]
pub fn WaitForSignalForm(config: Signal<WaitForSignalConfig>) -> Element {
    rsx! {
        div {
            class: "space-y-4",

            div {
                class: "bg-rose-50 border border-rose-200 rounded-lg p-3 mb-4",
                p {
                    class: "text-sm text-rose-800",
                    "📡 ",
                    strong { "Wait for Signal" },
                    " - Pause until another workflow sends a signal."
                }
            }

            div {
                class: "form-field",
                label {
                    class: "block text-sm font-medium text-gray-700 mb-1",
                    "Promise Name"
                }
                input {
                    r#type: "text",
                    class: "w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-rose-500",
                    placeholder: "e.g., payment_complete, order_shipped",
                    value: "{config.read().promise_name}",
                    oninput: move |e| {
                        config.write().promise_name = e.value().clone();
                    }
                }
                p {
                    class: "text-xs text-gray-500 mt-1",
                    "Another workflow resolves this promise by name"
                }
            }

            div {
                class: "form-field",
                label {
                    class: "block text-sm font-medium text-gray-700 mb-1",
                    "Give up after..."
                }
                div {
                    class: "grid grid-cols-3 gap-2",
                    button {
                        class: "px-3 py-2 text-sm border rounded-md hover:bg-gray-50",
                        onclick: move |_| {
                            config.write().timeout_ms = Some(60000);
                        },
                        "1 minute"
                    }
                    button {
                        class: "px-3 py-2 text-sm border rounded-md hover:bg-gray-50",
                        onclick: move |_| {
                            config.write().timeout_ms = Some(3600000);
                        },
                        "1 hour"
                    }
                    button {
                        class: "px-3 py-2 text-sm border rounded-md hover:bg-gray-50",
                        onclick: move |_| {
                            config.write().timeout_ms = None;
                        },
                        "No timeout"
                    }
                }
            }

            div {
                class: "bg-yellow-50 p-3 rounded-lg",
                p {
                    class: "text-sm text-yellow-800",
                    "💡 Use a 'Resolve Signal' step in another workflow to continue this one."
                }
            }
        }
    }
}

#[component]
pub fn WaitForSignalNodeCard() -> Element {
    rsx! {
        div {
            class: "flex items-center gap-3 p-3 bg-white border border-gray-200 rounded-lg shadow-sm hover:shadow-md transition-shadow",

            div {
                class: "w-10 h-10 bg-rose-100 rounded-full flex items-center justify-center",
                span {
                    class: "text-xl",
                    "📡"
                }
            },

            div {
                class: "flex-1",
                h3 {
                    class: "font-medium text-gray-900",
                    "Wait for Signal"
                }
                p {
                    class: "text-sm text-gray-500",
                    "Wait for another workflow"
                }
            }
        }
    }
}
