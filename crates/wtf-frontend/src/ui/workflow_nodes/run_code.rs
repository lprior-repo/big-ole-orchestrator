use crate::ui::workflow_nodes::schema::{CodeLanguage, RunCodeConfig};
use dioxus::prelude::*;

#[derive(Clone)]
struct LanguageDrafts {
    expression: String,
    javascript: String,
    python: String,
}

fn default_template(language: &CodeLanguage) -> &'static str {
    match language {
        CodeLanguage::Expression => "{{ steps.total.amount }} * 1.2",
        CodeLanguage::JavaScript => {
            "// Available: input (from previous step)\n// Return value is saved\n\nconst result = input.amount * 1.2;\nreturn { total: result };"
        }
        CodeLanguage::Python => {
            "# Available: input (from previous step)\n# Return value is saved\n\nresult = input[\"amount\"] * 1.2\nreturn {\"total\": result}"
        }
    }
}

fn code_for_language(drafts: &LanguageDrafts, language: &CodeLanguage) -> String {
    match language {
        CodeLanguage::Expression => drafts.expression.clone(),
        CodeLanguage::JavaScript => drafts.javascript.clone(),
        CodeLanguage::Python => drafts.python.clone(),
    }
}

fn with_language_draft(drafts: &LanguageDrafts, language: &CodeLanguage, code: String) -> LanguageDrafts {
    match language {
        CodeLanguage::Expression => LanguageDrafts {
            expression: code,
            javascript: drafts.javascript.clone(),
            python: drafts.python.clone(),
        },
        CodeLanguage::JavaScript => LanguageDrafts {
            expression: drafts.expression.clone(),
            javascript: code,
            python: drafts.python.clone(),
        },
        CodeLanguage::Python => LanguageDrafts {
            expression: drafts.expression.clone(),
            javascript: drafts.javascript.clone(),
            python: code,
        },
    }
}

#[component]
pub fn RunCodeForm(config: Signal<RunCodeConfig>) -> Element {
    let initial_config = config.read().clone();
    let drafts = use_signal(move || {
        let expression = match &initial_config.language {
            CodeLanguage::Expression => initial_config.code.clone(),
            _ => default_template(&CodeLanguage::Expression).to_string(),
        };
        let javascript = match &initial_config.language {
            CodeLanguage::JavaScript => initial_config.code.clone(),
            _ => default_template(&CodeLanguage::JavaScript).to_string(),
        };
        let python = match &initial_config.language {
            CodeLanguage::Python => initial_config.code.clone(),
            _ => default_template(&CodeLanguage::Python).to_string(),
        };

        LanguageDrafts {
            expression,
            javascript,
            python,
        }
    });

    let last_synced_code = use_signal(|| initial_config.code.clone());
    let last_synced_lang = use_signal(|| initial_config.language.clone());

    use_effect(move || {
        let current_code = config.read().code.clone();
        let current_lang = config.read().language.clone();
        let synced_code = last_synced_code.read().clone();
        let synced_lang = last_synced_lang.read().clone();

        if current_code != synced_code || current_lang != synced_lang {
            last_synced_code.set(current_code.clone());
            last_synced_lang.set(current_lang.clone());
            drafts.set(with_language_draft(&drafts.read(), &current_lang, current_code));
        }
    });

    rsx! {
        div {
            class: "space-y-4",

            div {
                class: "bg-red-50 border border-red-200 rounded-lg p-3 mb-4",
                p {
                    class: "text-sm text-red-800",
                    "⚡ ",
                    strong { "Run Code" },
                    " - Execute custom code. Results are saved for retries."
                }
            }

            div {
                class: "form-field",
                label {
                    class: "block text-sm font-medium text-gray-700 mb-1",
                    "Language"
                }
                select {
                    class: "w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-red-500",
                    value: match &*config.read() {
                        RunCodeConfig { language: CodeLanguage::JavaScript, .. } => "JavaScript",
                        RunCodeConfig { language: CodeLanguage::Python, .. } => "Python",
                        RunCodeConfig { language: CodeLanguage::Expression, .. } => "Expression",
                    },
                    onchange: move |e| {
                        let next_language = match e.value().as_str() {
                            "JavaScript" => CodeLanguage::JavaScript,
                            "Python" => CodeLanguage::Python,
                            "Expression" => CodeLanguage::Expression,
                            _ => CodeLanguage::Expression,
                        };

                        let current_config = config.read().clone();
                        let updated_drafts = with_language_draft(
                            &drafts.read(),
                            &current_config.language,
                            current_config.code,
                        );
                        let next_code = code_for_language(&updated_drafts, &next_language);

                        drafts.set(updated_drafts);
                        config.set(RunCodeConfig {
                            language: next_language,
                            code: next_code,
                        });
                    },
                    option { value: "Expression", "Expression (simple math/transform)" }
                    option { value: "JavaScript", "JavaScript" }
                    option { value: "Python", "Python" }
                }
            }

            match &*config.read() {
                RunCodeConfig { language: CodeLanguage::Expression, .. } => {
                    rsx! {
                        div {
                            class: "form-field",
                            label {
                                class: "block text-sm font-medium text-gray-700 mb-1",
                                "Expression"
                            }
                            input {
                                r#type: "text",
                                class: "w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-red-500 font-mono",
                                placeholder: "{{ steps.total.amount }} * 1.2",
                                value: "{config.read().code}",
                                oninput: move |e| {
                                    let value = e.value();
                                    let language = config.read().language.clone();
                                    drafts.set(with_language_draft(&drafts.read(), &language, value.clone()));
                                    config.set(RunCodeConfig {
                                        language,
                                        code: value,
                                    });
                                }
                            }
                            p {
                                class: "text-xs text-gray-500 mt-1",
                                "Use double braces like {{ and }} to render literals"
                            }
                        }
                    }
                }
                _ => {
                    rsx! {
                        div {
                            class: "form-field",
                            label {
                                class: "block text-sm font-medium text-gray-700 mb-1",
                                "Code"
                            }
                            textarea {
                                class: "w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-red-500 font-mono text-sm",
                                rows: 8,
                                placeholder: "// Available: input (from previous step)\n// Return value is saved\n\nconst result = input.amount * 1.2;\nreturn { total: result };",
                                value: "{config.read().code}",
                                oninput: move |e| {
                                    let value = e.value();
                                    let language = config.read().language.clone();
                                    drafts.set(with_language_draft(&drafts.read(), &language, value.clone()));
                                    config.set(RunCodeConfig {
                                        language,
                                        code: value,
                                    });
                                }
                            }
                            p {
                                class: "text-xs text-gray-500 mt-1",
                                "Use double braces like {{ and }} to render literals"
                            }
                        }
                    }
                }
            }

            div {
                class: "bg-yellow-50 p-3 rounded-lg",
                p {
                    class: "text-sm text-yellow-800",
                    "⚠️ Code runs once. On retry, the saved result is used to ensure consistency."
                }
            }
        }
    }
}

#[component]
pub fn RunCodeNodeCard() -> Element {
    rsx! {
        div {
            class: "flex items-center gap-3 p-3 bg-white border border-gray-200 rounded-lg shadow-sm hover:shadow-md transition-shadow",

            div {
                class: "w-10 h-10 bg-red-100 rounded-full flex items-center justify-center",
                span {
                    class: "text-xl",
                    "⚡"
                }
            },

            div {
                class: "flex-1",
                h3 {
                    class: "font-medium text-gray-900",
                    "Run Code"
                }
                p {
                    class: "text-sm text-gray-500",
                    "Execute custom logic"
                }
            }
        }
    }
}
