use crate::ui::workflow_nodes::schema::{MemoryKey, SaveToMemoryConfig};
use dioxus::prelude::*;

fn json_to_display(value: &serde_json::Value) -> String {
    serde_json::to_string_pretty(value).map_or_else(|_| String::new(), |value| value)
}

fn parse_json_draft(input: &str) -> Result<serde_json::Value, String> {
    serde_json::from_str(input).map_err(|error| format!("Invalid JSON: {error}"))
}

#[component]
pub fn SaveToMemoryForm(config: ReadOnlySignal<SaveToMemoryConfig>) -> Element {
    let mut write_config = config.writer();
    let initial_draft = json_to_display(&config.read().value);
    let draft = use_signal(move || initial_draft);
    let parse_error = use_signal(|| None::<String>);

    rsx! {
        div {
            class: "space-y-4",
            div {
                class: "bg-indigo-50 border border-indigo-200 rounded-lg p-3 mb-4",
                p {
                    class: "text-sm text-indigo-800",
                    "💾 ",
                    strong { "Save Data" },
                    " - Store a value you can use later."
                }
            }
            div {
                class: "form-field",
                label { class: "block text-sm font-medium text-gray-700 mb-1", "What name should this have?" }
                input {
                    r#type: "text",
                    class: "w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-indigo-500",
                    placeholder: "e.g., order_total, user_email, approval_status",
                    value: "{config.read().key.as_str()}",
                    oninput: move |e| write_config.write().key = MemoryKey::new(e.value()),
                }
                p { class: "text-xs text-gray-500 mt-1", "Use this name to load the data later" }
            }
            div {
                class: "form-field",
                label { class: "block text-sm font-medium text-gray-700 mb-1", "What to save (JSON)" }
                textarea {
                    class: "w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-indigo-500 font-mono text-sm",
                    rows: 4,
                    placeholder: r#"{"amount": 100, "currency": "USD"}"#,
                    value: "{draft.read()}",
                    oninput: move |e| {
                        let next_value = e.value().clone();
                        draft.set(next_value.clone());
                        match parse_json_draft(next_value.as_str()) {
                            Ok(value) => {
                                parse_error.set(None);
                                write_config.write().value = value;
                            }
                            Err(error_text) => parse_error.set(Some(error_text)),
                        }
                    },
                }
                if let Some(error_text) = parse_error.read().as_ref() {
                    p { class: "text-xs text-red-600 mt-1", "{error_text}" }
                }
            }
            div {
                class: "bg-gray-50 p-3 rounded-lg",
                p { class: "text-sm text-gray-600", "💡 This is like saving a variable. The data persists until you overwrite it or the workflow ends." }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_json_draft;

    #[test]
    fn parse_json_draft_accepts_valid_json() {
        assert!(parse_json_draft(r#"{"ok":true}"#).is_ok());
    }

    #[test]
    fn parse_json_draft_rejects_invalid_json() {
        assert!(parse_json_draft("{not-json}").is_err());
    }
}

#[component]
pub fn SaveToMemoryNodeCard() -> Element {
    rsx! {
        div {
            class: "flex items-center gap-3 p-3 bg-white border border-gray-200 rounded-lg shadow-sm hover:shadow-md transition-shadow",
            div { class: "w-10 h-10 bg-indigo-100 rounded-full flex items-center justify-center", span { class: "text-xl", "💾" } },
            div {
                class: "flex-1",
                h3 { class: "font-medium text-gray-900", "Save to Memory" }
                p { class: "text-sm text-gray-500", "Store a value for later" }
            }
        }
    }
}
