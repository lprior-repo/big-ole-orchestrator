use crate::ui::workflow_nodes::schema::{ServiceCallConfig, TargetType};
use dioxus::prelude::*;

#[component]
pub fn ServiceCallForm(config: Signal<ServiceCallConfig>) -> Element {
    let pretty_input = if let Ok(value) = serde_json::to_string_pretty(&*config.read().input) {
        value
    } else {
        "{}".to_string()
    };
    let json_draft = use_signal(|| pretty_input.clone());
    let json_error = use_signal(|| Option::<String>::None);
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

    let condition_value = match config.read().condition.clone() {
        Some(value) => value,
        None => String::new(),
    };

    rsx! {
        div {
            class: "space-y-4",

            div {
                class: "bg-green-50 border border-green-200 rounded-lg p-3 mb-4",
                p {
                    class: "text-sm text-green-800",
                    "🔗 ",
                    strong { "Action" },
                    " - Call another service and wait for the response."
                }
            }

            div {
                class: "form-field",
                label {
                    class: "block text-sm font-medium text-gray-700 mb-1",
                    "What type of thing are you calling?"
                }
                select {
                    class: "w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-green-500",
                    value: match &*config.read() {
                        ServiceCallConfig { target_type: TargetType::Service, .. } => "Service",
                        ServiceCallConfig { target_type: TargetType::VirtualObject, .. } => "Virtual Object",
                        ServiceCallConfig { target_type: TargetType::Workflow, .. } => "Workflow",
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
                    option { value: "Service", "Service (stateless)" }
                    option { value: "Virtual Object", "Virtual Object (stateful)" }
                    option { value: "Workflow", "Workflow (long-running)" }
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
                    class: "w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-green-500",
                    placeholder: "e.g., payment_service",
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
                    "Object/Workflow Key"
                }
                input {
                    r#type: "text",
                    class: "w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-green-500",
                    placeholder: "e.g., order-123",
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
                p {
                    class: "text-xs text-gray-500 mt-1",
                    "Which specific object/workflow to call"
                }
            }

            div {
                class: "form-field",
                label {
                    class: "block text-sm font-medium text-gray-700 mb-1",
                    "What to do"
                }
                input {
                    r#type: "text",
                    class: "w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-green-500",
                    placeholder: "e.g., charge_card",
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
                    "What to send (JSON)"
                }
                textarea {
                    class: "w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-green-500 font-mono text-sm",
                    rows: 4,
                    placeholder: r#"{"amount": 100, "currency": "USD"}"#,
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

            div {
                class: "form-field",
                label {
                    class: "block text-sm font-medium text-gray-700 mb-1",
                    "Only run if..."
                }
                input {
                    r#type: "text",
                    class: "w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-green-500",
                    placeholder: "e.g., {{ steps.validate.valid }} == true",
                    value: "{condition_value}",
                    oninput: move |e| {
                        let value = e.value().clone();
                        config.write().condition = if value.trim().is_empty() {
                            None
                        } else {
                            Some(value)
                        };
                    }
                }
                p {
                    class: "text-xs text-gray-500 mt-1",
                    "Leave empty to always run this step"
                }
            }
        }
    }
}

#[component]
pub fn ServiceCallNodeCard() -> Element {
    rsx! {
        div {
            class: "flex items-center gap-3 p-3 bg-white border border-gray-200 rounded-lg shadow-sm hover:shadow-md transition-shadow",

            div {
                class: "w-10 h-10 bg-green-100 rounded-full flex items-center justify-center",
                span {
                    class: "text-xl",
                    "🔗"
                }
            }

            div {
                class: "flex-1",
                h3 {
                    class: "font-medium text-gray-900",
                    "Call Service"
                }
                p {
                    class: "text-sm text-gray-500",
                    "Call another service and wait for result"
                }
            }
        }
    }
}
