use crate::ui::workflow_nodes::schema::{LoadFromMemoryConfig, MemoryKey};
use dioxus::prelude::*;

fn json_to_display(value: &serde_json::Value) -> String {
    serde_json::to_string_pretty(value).map_or_else(|_| String::new(), |value| value)
}

fn optional_json_to_display(value: Option<&serde_json::Value>) -> String {
    if let Some(value) = value {
        json_to_display(value)
    } else {
        String::new()
    }
}

fn parse_optional_json_draft(input: &str) -> Result<Option<serde_json::Value>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        serde_json::from_str(trimmed)
            .map(Some)
            .map_err(|error| format!("Invalid JSON: {error}"))
    }
}

#[component]
pub fn LoadFromMemoryForm(config: ReadOnlySignal<LoadFromMemoryConfig>) -> Element {
    let mut write_config = config.writer();
    let initial_draft = optional_json_to_display(config.read().default.as_ref());
    let draft = use_signal(move || initial_draft);
    let parse_error = use_signal(|| None::<String>);

    rsx! {
        div {
            class: "space-y-4",
            div {
                class: "bg-cyan-50 border border-cyan-200 rounded-lg p-3 mb-4",
                p {
                    class: "text-sm text-cyan-800",
                    "📂 ",
                    strong { "Load Data" },
                    " - Get a value you saved earlier."
                }
            }
            div {
                class: "form-field",
                label { class: "block text-sm font-medium text-gray-700 mb-1", "Which saved item?" }
                input {
                    r#type: "text",
                    class: "w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-cyan-500",
                    placeholder: "e.g., order_total, user_email",
                    value: "{config.read().key.as_str()}",
                    oninput: move |e| write_config.write().key = MemoryKey::new(e.value()),
                }
                p { class: "text-xs text-gray-500 mt-1", "Name you used when saving" }
            }
            div {
                class: "form-field",
                label { class: "block text-sm font-medium text-gray-700 mb-1", "Default value (if not found)" }
                textarea {
                    class: "w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-cyan-500 font-mono text-sm",
                    rows: 2,
                    placeholder: "null",
                    value: "{draft.read()}",
                    oninput: move |e| {
                        let next_value = e.value().clone();
                        draft.set(next_value.clone());
                        match parse_optional_json_draft(next_value.as_str()) {
                            Ok(next_default) => {
                                parse_error.set(None);
                                write_config.write().default = next_default;
                            }
                            Err(error_text) => parse_error.set(Some(error_text)),
                        }
                    }
                }
                if let Some(error_text) = parse_error.read().as_ref() {
                    p { class: "text-xs text-red-600 mt-1", "{error_text}" }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_optional_json_draft;

    #[test]
    fn parse_optional_json_draft_empty_is_none() {
        assert_eq!(parse_optional_json_draft("   "), Ok(None));
    }

    #[test]
    fn parse_optional_json_draft_valid_is_some() {
        assert!(matches!(parse_optional_json_draft(r#"{"fallback":1}"#), Ok(Some(_))));
    }

    #[test]
    fn parse_optional_json_draft_invalid_is_err() {
        assert!(parse_optional_json_draft("{not-json}").is_err());
    }
}

#[component]
pub fn LoadFromMemoryNodeCard() -> Element {
    rsx! {
        div {
            class: "flex items-center gap-3 p-3 bg-white border border-gray-200 rounded-lg shadow-sm hover:shadow-md transition-shadow",
            div { class: "w-10 h-10 bg-cyan-100 rounded-full flex items-center justify-center", span { class: "text-xl", "📂" } },
            div {
                class: "flex-1",
                h3 { class: "font-medium text-gray-900", "Load from Memory" }
                p { class: "text-sm text-gray-500", "Get a saved value" }
            }
        }
    }
}
