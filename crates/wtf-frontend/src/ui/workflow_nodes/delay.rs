use crate::ui::workflow_nodes::schema::DelayConfig;
use dioxus::prelude::*;

#[component]
pub fn DelayForm(config: ReadOnlySignal<DelayConfig>) -> Element {
    let mut write_config = config.writer();
    let duration_error = use_signal(|| Option::<String>::None);

    rsx! {
        div {
            class: "space-y-4",

            div {
                class: "bg-gray-50 border border-gray-200 rounded-lg p-3 mb-4",
                p {
                    class: "text-sm text-gray-700",
                    "⏱️ ",
                    strong { "Wait" },
                    " - Pause the workflow for a while."
                }
            }

            div {
                class: "form-field",
                label {
                    class: "block text-sm font-medium text-gray-700 mb-1",
                    "How long to wait?"
                }
                div {
                    class: "grid grid-cols-3 gap-2 mb-3",
                    button {
                        class: "px-3 py-2 text-sm border rounded-md hover:bg-gray-50",
                        onclick: move |_| {
                            write_config.write().duration_ms = 1000;
                        },
                        "1 second"
                    }
                    button {
                        class: "px-3 py-2 text-sm border rounded-md hover:bg-gray-50",
                        onclick: move |_| {
                            write_config.write().duration_ms = 5000;
                        },
                        "5 seconds"
                    }
                    button {
                        class: "px-3 py-2 text-sm border rounded-md hover:bg-gray-50",
                        onclick: move |_| {
                            write_config.write().duration_ms = 30000;
                        },
                        "30 seconds"
                    }
                }
                div {
                    class: "grid grid-cols-3 gap-2 mb-2",
                    button {
                        class: "px-3 py-2 text-sm border rounded-md hover:bg-gray-50",
                        onclick: move |_| {
                            write_config.write().duration_ms = 60_000;
                        },
                        "1 minute"
                    }
                    button {
                        class: "px-3 py-2 text-sm border rounded-md hover:bg-gray-50",
                        onclick: move |_| {
                            write_config.write().duration_ms = 300_000;
                        },
                        "5 minutes"
                    }
                    button {
                        class: "px-3 py-2 text-sm border rounded-md hover:bg-gray-50",
                        onclick: move |_| {
                            write_config.write().duration_ms = 3600_000;
                        },
                        "1 hour"
                    }
                }
                div {
                    class: "flex items-center gap-2",
                    input {
                        r#type: "number",
                        min: "1",
                        class: "flex-1 px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-gray-500",
                        placeholder: "milliseconds",
                        value: "{config.read().duration_ms}",
                        oninput: move |e| {
                            let value = e.value();
                            if value.trim().is_empty() {
                                duration_error.set(None);
                            } else if let Ok(v) = value.parse::<u64>() {
                                if v > 0 {
                                    write_config.write().duration_ms = v;
                                    duration_error.set(None);
                                } else {
                                    duration_error.set(Some("Duration must be greater than 0 ms".to_string()));
                                }
                            }
                        }
                    }
                    span {
                        class: "text-sm text-gray-500",
                        "ms"
                    }
                }
            }

                if let Some(error) = duration_error() {
                    p {
                        class: "text-xs text-red-600 mt-1",
                        "{error}"
                    }
                } else {
                    p {
                        class: "text-xs text-gray-500 mt-1",
                        "Duration must be greater than 0 ms"
                    }
                }

                div {
                    class: "bg-yellow-50 p-3 rounded-lg",
                    p {
                        class: "text-sm text-yellow-800",
                        "⚠️ The workflow will be paused. For Virtual Objects, this blocks other calls to the same object."
                    }
                }
        }
    }
}

#[component]
pub fn DelayNodeCard() -> Element {
    rsx! {
        div {
            class: "flex items-center gap-3 p-3 bg-white border border-gray-200 rounded-lg shadow-sm hover:shadow-md transition-shadow",

            div {
                class: "w-10 h-10 bg-gray-200 rounded-full flex items-center justify-center",
                span {
                    class: "text-xl",
                    "⏱️"
                }
            },

            div {
                class: "flex-1",
                h3 {
                    class: "font-medium text-gray-900",
                    "Delay"
                }
                p {
                    class: "text-sm text-gray-500",
                    "Wait for a period of time"
                }
            }
        }
    }
}
