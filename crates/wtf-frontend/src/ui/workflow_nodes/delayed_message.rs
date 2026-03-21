use crate::ui::workflow_nodes::schema::{DelayedMessageConfig, TargetType};
use dioxus::prelude::*;

#[component]
pub fn DelayedMessageForm(config: Signal<DelayedMessageConfig>) -> Element {
    let pretty_input = if let Ok(value) = serde_json::to_string_pretty(&*config.read().input) {
        value
    } else {
        "{}".to_string()
    };
    let json_draft = use_signal(|| pretty_input.clone());
    let json_error = use_signal(|| Option::<String>::None);
    let delay_error = use_signal(|| Option::<String>::None);
    let last_synced_input = use_signal(|| pretty_input.clone());

    use_effect(move || {
        let latest = serde_json::to_string_pretty(&*config.read().input)
            .unwrap_or_else(|_| "{}".to_string());
        let synced = last_synced_input.read().clone();
        if latest != synced {
            last_synced_input.set(latest.clone());
            json_draft.set(latest);
            json_error.set(None);
        }
    });

    let key_value = match config.read().key.clone() {
        Some(value) => value,
        None => String::new(),
    };

    rsx! {
        div {
            class: "space-y-4",

            div {
                class: "bg-orange-50 border border-orange-200 rounded-lg p-3 mb-4",
                p {
                    class: "text-sm text-orange-800",
                    "⏰ ",
                    strong { "Schedule for Later" },
                    " - Send a message at a specific time."
                }
            }

            div {
                class: "form-field",
                label {
                    class: "block text-sm font-medium text-gray-700 mb-1",
                    "Send to"
                }
                select {
                    class: "w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-orange-500",
                    value: match config.read().target_type {
                        TargetType::Service => "Service",
                        TargetType::VirtualObject => "Virtual Object",
                        TargetType::Workflow => "Workflow",
                    },
                    onchange: move |e| {
                        let next_target_type = match e.value().as_str() {
                            "Service" => TargetType::Service,
                            "Virtual Object" => TargetType::VirtualObject,
                            "Workflow" => TargetType::Workflow,
                            _ => TargetType::Service,
                        };

                        let mut cfg = config.write();
                        cfg.target_type = next_target_type;
                        if matches!(next_target_type, TargetType::Service) {
                            cfg.key = None;
                        }
                    },
                    option { value: "Service", "Service" }
                    option { value: "Virtual Object", "Virtual Object" }
                    option { value: "Workflow", "Workflow" }
                }
            }

            div {
                class: "form-field",
                label {
                    class: "block text-sm font-medium text-gray-700 mb-1",
                    "Service Name"
                }
                input {
                    r#type: "text",
                    class: "w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-orange-500",
                    placeholder: "e.g., reminder_service",
                    value: "{config.read().service_name}",
                    oninput: move |e| {
                        config.write().service_name = e.value().clone();
                    }
                }
            }

            div {
                class: "form-field",
                visible: matches!(config.read().target_type, TargetType::VirtualObject | TargetType::Workflow),
                label {
                    class: "block text-sm font-medium text-gray-700 mb-1",
                    "Key (for objects/workflows)"
                }
                input {
                    r#type: "text",
                    class: "w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-orange-500",
                    placeholder: "e.g., user-456",
                    value: "{key_value}",
                    oninput: move |e| {
                        let value = e.value().clone();
                        config.write().key = if value.trim().is_empty() {
                            None
                        } else {
                            Some(value)
                        };
                    }
                }
            }

            div {
                class: "form-field",
                label {
                    class: "block text-sm font-medium text-gray-700 mb-1",
                    "Handler"
                }
                input {
                    r#type: "text",
                    class: "w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-orange-500",
                    placeholder: "e.g., send_reminder",
                    value: "{config.read().handler_name}",
                    oninput: move |e| {
                        config.write().handler_name = e.value().clone();
                    }
                }
            }

            div {
                class: "form-field",
                label {
                    class: "block text-sm font-medium text-gray-700 mb-1",
                    "How long to wait?"
                }
                div {
                    class: "grid grid-cols-4 gap-2 mb-2",
                    button {
                        class: "px-2 py-2 text-sm border rounded-md hover:bg-gray-50",
                        onclick: move |_| {
                            config.write().delay_ms = 60_000;
                            delay_error.set(None);
                        },
                        "1 min"
                    }
                    button {
                        class: "px-2 py-2 text-sm border rounded-md hover:bg-gray-50",
                        onclick: move |_| {
                            config.write().delay_ms = 3600_000;
                            delay_error.set(None);
                        },
                        "1 hour"
                    }
                    button {
                        class: "px-2 py-2 text-sm border rounded-md hover:bg-gray-50",
                        onclick: move |_| {
                            config.write().delay_ms = 86400_000;
                            delay_error.set(None);
                        },
                        "1 day"
                    }
                    button {
                        class: "px-2 py-2 text-sm border rounded-md hover:bg-gray-50",
                        onclick: move |_| {
                            config.write().delay_ms = 604800_000;
                            delay_error.set(None);
                        },
                        "1 week"
                    }
                }
                input {
                    r#type: "number",
                    min: "1",
                    class: "w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-orange-500",
                    placeholder: "Or enter milliseconds",
                    value: "{config.read().delay_ms}",
                    oninput: move |e| {
                        let value = e.value();
                        if value.trim().is_empty() {
                            delay_error.set(None);
                        } else if let Ok(parsed) = value.parse::<u64>() {
                            if parsed > 0 {
                                config.write().delay_ms = parsed;
                                delay_error.set(None);
                            } else {
                                delay_error
                                    .set(Some("Delay must be greater than 0 ms".to_string()));
                            }
                        }
                    }
                }
                if let Some(error) = delay_error() {
                    p {
                        class: "text-xs text-red-600 mt-1",
                        "{error}"
                    }
                } else {
                    p {
                        class: "text-xs text-gray-500 mt-1",
                        "How long to wait before sending (must be greater than 0 ms)"
                    }
                }
            }

            div {
                class: "form-field",
                label {
                    class: "block text-sm font-medium text-gray-700 mb-1",
                    "Message (JSON)"
                }
                textarea {
                    class: "w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-orange-500 font-mono text-sm",
                    rows: 3,
                    value: "{json_draft.read()}",
                    oninput: move |e| {
                        let draft = e.value().clone();
                        json_draft.set(draft.clone());

                        match serde_json::from_str(&draft) {
                            Ok(value) => {
                                config.write().input = value;
                                last_synced_input.set(draft);
                                json_error.set(None);
                            }
                            Err(error) => {
                                json_error.set(Some(format!("Invalid JSON: {error}")));
                            }
                        }
                    },
                }
                if let Some(error) = json_error() {
                    p {
                        class: "text-xs text-red-600 mt-1",
                        "{error}"
                    }
                } else {
                    p {
                        class: "text-xs text-gray-500 mt-1",
                        "Enter valid JSON to update this field"
                    }
                }
            }
        }
    }
}

#[component]
pub fn DelayedMessageNodeCard() -> Element {
    rsx! {
        div {
            class: "flex items-center gap-3 p-3 bg-white border border-gray-200 rounded-lg shadow-sm hover:shadow-md transition-shadow",

            div {
                class: "w-10 h-10 bg-orange-100 rounded-full flex items-center justify-center",
                span {
                    class: "text-xl",
                    "⏰"
                }
            },

            div {
                class: "flex-1",
                h3 {
                    class: "font-medium text-gray-900",
                    "Delayed Message"
                }
                p {
                    class: "text-sm text-gray-500",
                    "Send after a delay"
                }
            }
        }
    }
}
