//! Workflow definition upload UI.
//!
//! Provides a complete workflow for uploading workflow definitions:
//! - File upload (TOML/JSON) via file picker
//! - Inline text editor with line numbers and live preview
//! - Validation with error/warning display
//! - Graph preview of parsed workflow structure
//! - API submission to start workflow instances

pub mod editor;
pub mod file_upload;
pub mod graph_preview;
pub mod types;

use dioxus::prelude::*;

use self::editor::WorkflowEditor;
use self::file_upload::{parse_content, detect_format_from_filename, FormatHint};
use self::graph_preview::GraphPreview;
use self::types::{
    UploadState, UploadResult, validate_definition, ValidationResult, WorkflowDefinition,
    ValidationSeverity,
};

#[component]
pub fn WorkflowUploadForm(api_base_url: String) -> Element {
    let content = use_signal(|| String::new());
    let format = use_signal(|| FormatHint::Auto);
    let parse_error = use_signal(|| None::<String>);
    let validation_result = use_signal(|| None::<ValidationResult>);
    let preview = use_signal(|| None::<WorkflowDefinition>);
    let upload_state = use_signal(|| UploadState::Idle);
    let upload_result = use_signal(|| None::<UploadResult>);
    let active_tab = use_signal(|| 0usize);

    rsx! {
        div {
            class: "border border-gray-200 rounded-xl overflow-hidden shadow-sm",
            div {
                class: "px-6 py-4 bg-gradient-to-r from-blue-600 to-indigo-600",
                div { class: "flex items-center justify-between",
                    h2 { class: "text-lg font-semibold text-white",
                        "Upload Workflow Definition"
                    }
                    span { class: "text-sm text-blue-100",
                        "TOML · JSON · Drag & Drop"
                    }
                }
            }
            div { class: "flex border-b border-gray-200",
                TabButton {
                    active: active_tab.peek() == 0,
                    on_click: move |_| active_tab.set(0),
                    icon: TabIconEnum::Text,
                    label: "Editor",
                }
                TabButton {
                    active: active_tab.peek() == 1,
                    on_click: move |_| active_tab.set(1),
                    icon: TabIconEnum::File,
                    label: "File Upload",
                }
                TabButton {
                    active: active_tab.peek() == 2,
                    on_click: move |_| active_tab.set(2),
                    icon: TabIconEnum::Graph,
                    label: "Preview",
                    badge: badge_count(&validation_result),
                }
            }
            div { class: "p-6",
                if active_tab.peek() == 0 {
                    EditorTab {
                        content,
                        format,
                        parse_error,
                        preview,
                        validation_result,
                    }
                } else if active_tab.peek() == 1 {
                    UploadTab {
                        content,
                        format,
                        parse_error,
                        preview,
                        validation_result,
                    }
                } else {
                    PreviewTab {
                        validation_result,
                        preview,
                    }
                }
            }
            if *upload_state.read() != UploadState::Idle {
                ActionBar {
                    upload_state,
                    upload_result,
                    api_base_url,
                    on_submit: move || {
                        submit_workflow(&content.peek().clone(), &format.peek().clone(), &api_base_url, upload_state, upload_result);
                    },
                    on_reset: move || {
                        upload_state.set(UploadState::Idle);
                        upload_result.set(None);
                    },
                }
            } else if preview.peek().is_some() {
                let is_valid = preview.peek().as_ref().map_or(false, |d| validate_definition(d).is_valid());
                let node_count = preview.peek().as_ref().map_or(0, |d| d.nodes.len());
                SubmitButton {
                    is_valid,
                    node_count,
                    on_submit: move || {
                        submit_workflow(&content.peek().clone(), &format.peek().clone(), &api_base_url, upload_state, upload_result);
                    },
                }
            }
        }
    }
}

fn badge_count(validation_result: &Signal<Option<ValidationResult>>) -> Option<usize> {
    validation_result.peek().as_ref().and_then(|vr| {
        if vr.has_errors {
            Some(vr.issues.iter().filter(|i| i.severity == ValidationSeverity::Error).count())
        } else if !vr.issues.is_empty() {
            Some(vr.issues.len())
        } else {
            None
        }
    })
}

fn validate_editor_content(
    c: &str,
    current_format: &FormatHint,
    parse_error: &mut Signal<Option<String>>,
    preview: &mut Signal<Option<WorkflowDefinition>>,
    validation_result: &mut Signal<Option<ValidationResult>>,
) {
    let trimmed = c.trim();
    if trimmed.is_empty() {
        parse_error.set(None);
        preview.set(None);
        validation_result.set(None);
        return;
    }
    match parse_content(trimmed, *current_format) {
        Ok(def) => {
            parse_error.set(None);
            preview.set(Some(def.clone()));
            let vr = validate_definition(&def);
            validation_result.set(Some(vr));
        }
        Err(e) => {
            parse_error.set(Some(e.clone()));
            preview.set(None);
            validation_result.set(None);
        }
    }
}

fn submit_workflow(
    content: &str,
    _format: &FormatHint,
    api_url: &str,
    mut upload_state: Signal<UploadState>,
    mut upload_result: Signal<Option<UploadResult>>,
) {
    #[cfg(all(feature = "sse", target_arch = "wasm32"))]
    {
        use self::file_upload::submit_workflow as api_submit;

        let def = match serde_json::from_str::<WorkflowDefinition>(content.trim()) {
            Ok(d) => d,
            Err(e) => {
                upload_state.set(UploadState::UploadFailed);
                upload_result.set(Some(UploadResult {
                    success: false,
                    instance_id: None,
                    error: Some(format!("Failed to parse definition: {e}")),
                }));
                return;
            }
        };

        upload_state.set(UploadState::Uploading);
        let api_url = api_url.to_string();
        spawn(async move {
            match api_submit(&def, &api_url).await {
                Ok(resp) => {
                    upload_state.set(UploadState::Uploaded);
                    upload_result.set(Some(UploadResult {
                        success: resp.success,
                        instance_id: resp.instance_id,
                        error: None,
                    }));
                }
                Err(e) => {
                    upload_state.set(UploadState::UploadFailed);
                    upload_result.set(Some(UploadResult {
                        success: false,
                        instance_id: None,
                        error: Some(e),
                    }));
                }
            }
        });
    }

    #[cfg(not(all(feature = "sse", target_arch = "wasm32")))]
    {
        upload_state.set(UploadState::UploadFailed);
        upload_result.set(Some(UploadResult {
            success: false,
            instance_id: None,
            error: Some("Workflow upload is only available in the web target".to_string()),
        }));
    }
}

// ---------------------------------------------------------------------------
// Editor tab
// ---------------------------------------------------------------------------

#[component]
fn EditorTab(
    content: Signal<String>,
    format: Signal<FormatHint>,
    parse_error: Signal<Option<String>>,
    preview: Signal<Option<WorkflowDefinition>>,
    validation_result: Signal<Option<ValidationResult>>,
) -> Element {
    let value = content.peek().clone();

    rsx! {
        div {
            ondragover: move |e| { e.prevent_default(); },
            ondrop: move |e| { e.prevent_default(); },
            WorkflowEditor {
                value,
                format,
                parse_error,
                preview,
                on_change: move |new_value| {
                    content.set(new_value.clone());
                    validate_editor_content(
                        &new_value,
                        &format.peek().clone(),
                        &mut *parse_error.read_mut(),
                        &mut *preview.read_mut(),
                        &mut *validation_result.read_mut(),
                    );
                },
            }
        }
    }
}

// ---------------------------------------------------------------------------
// File upload tab
// ---------------------------------------------------------------------------

#[component]
fn UploadTab(
    content: Signal<String>,
    format: Signal<FormatHint>,
    parse_error: Signal<Option<String>>,
    preview: Signal<Option<WorkflowDefinition>>,
    validation_result: Signal<Option<ValidationResult>>,
) -> Element {
    let file_error = use_signal(|| None::<String>);

    rsx! {
        div {
            class: "flex flex-col items-center justify-center py-12",
            div {
                class: "border-2 border-dashed border-gray-300 rounded-lg px-8 py-12 text-center",
                class: "hover:border-blue-400 transition-colors",
                input {
                    r#type: "file",
                    id: "workflow-file",
                    accept: ".json,.toml,.cfg,.ini,.txt",
                    class: "hidden",
                    onchange: move |_e| { },
                }
                label {
                    class: "cursor-pointer",
                    for: "workflow-file",
                    div { class: "mb-4",
                        div { class: "text-4xl mb-2", "📄" }
                    }
                    p { class: "text-gray-600 mb-2", "Drop a TOML or JSON file here" }
                    p { class: "text-sm text-gray-400", "or click to browse" }
                }
            }
            div { class: "mt-6 flex gap-4",
                FormatBadge { label: "TOML", color: "green" }
                FormatBadge { label: "JSON", color: "blue" }
            }
            if let Some(ref err) = *file_error.read() {
                p { class: "mt-4 text-sm text-red-600", "{err}" }
            }
        }
    }
}

#[component]
fn FormatBadge(label: String, color: String) -> Element {
    let bg_color = match color.as_str() {
        "blue" => "bg-blue-100 text-blue-700",
        "green" => "bg-green-100 text-green-700",
        _ => "bg-gray-100 text-gray-700",
    };
    rsx! {
        span { class: "text-xs px-3 py-1 rounded-full font-medium {bg_color}", "{label}" }
    }
}

// ---------------------------------------------------------------------------
// Preview tab
// ---------------------------------------------------------------------------

#[component]
fn PreviewTab(
    validation_result: Signal<Option<ValidationResult>>,
    preview: Signal<Option<WorkflowDefinition>>,
) -> Element {
    let vr = validation_result.read();
    let def = preview.read();

    let validation_title = if vr.as_ref().map_or(false, |v| v.has_errors) {
        "Validation Failed"
    } else if vr.as_ref().map_or(false, |v| !v.issues.is_empty()) {
        "Warnings Only"
    } else {
        "Valid"
    };
    let validation_icon = if vr.as_ref().map_or(false, |v| v.has_errors) {
        "✗"
    } else if vr.as_ref().map_or(false, |v| !v.issues.is_empty()) {
        "⚠"
    } else {
        "✓"
    };
    let validation_text_color = if vr.as_ref().map_or(false, |v| v.has_errors) {
        "text-red-700"
    } else if vr.as_ref().map_or(false, |v| !v.issues.is_empty()) {
        "text-yellow-700"
    } else {
        "text-green-700"
    };
    let validation_icon_color = if vr.as_ref().map_or(false, |v| v.has_errors) {
        "text-red-500"
    } else if vr.as_ref().map_or(false, |v| !v.issues.is_empty()) {
        "text-yellow-500"
    } else {
        "text-green-500"
    };

    rsx! {
        div { class: "space-y-6",
            if let Some(ref d) = *def {
                GraphPreview { def: d.clone() }
            } else {
                div { class: "text-center py-8 text-gray-400",
                    "No preview available — enter a valid workflow definition"
                }
            }
            if let Some(ref vr) = *vr {
                div {
                    class: "border border-gray-200 rounded-lg",
                    div {
                        class: "px-4 py-3 border-b border-gray-200",
                        div { class: "flex items-center gap-2",
                            span { class: "{validation_icon_color}", "{validation_icon}" }
                            span { class: "text-sm font-medium {validation_text_color}",
                                "{validation_title}"
                            }
                        }
                    }
                    if !vr.issues.is_empty() {
                        div { class: "divide-y divide-gray-100",
                            for issue in &vr.issues {
                                IssueRow { issue }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Submit button
// ---------------------------------------------------------------------------

#[component]
fn SubmitButton(
    is_valid: bool,
    node_count: usize,
    on_submit: Callback<()>,
) -> Element {
    let button_class = if is_valid {
        "bg-blue-600 hover:bg-blue-700"
    } else {
        "bg-gray-300 cursor-not-allowed"
    };
    let btn_text = if is_valid {
        if node_count == 1 {
            "Start Workflow (1 node)"
        } else {
            format!("Start Workflow ({node_count} nodes)")
        }
    } else {
        "Fix validation issues to start"
    };

    rsx! {
        div { class: "px-6 py-4 border-t border-gray-200 bg-gray-50",
            button {
                class: "w-full px-6 py-3 rounded-lg font-medium text-white transition-colors {button_class}",
                disabled: !is_valid,
                onclick: move |_| {
                    if is_valid {
                        on_submit.call(());
                    }
                },
                "{btn_text}"
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Action bar
// ---------------------------------------------------------------------------

#[component]
fn ActionBar(
    upload_state: Signal<UploadState>,
    upload_result: Signal<Option<UploadResult>>,
    api_base_url: String,
    on_submit: Callback<()>,
    on_reset: Callback<()>,
) -> Element {
    let state = *upload_state.read();
    let result = upload_result.read().clone();

    rsx! {
        div { class: "px-6 py-4 border-t border-gray-200 bg-gray-50",
            if state == UploadState::Uploading {
                div { class: "flex items-center gap-3",
                    div { class: "animate-spin",
                        svg {
                            class: "w-4 h-4 text-blue-600",
                            xmlns: "http://www.w3.org/2000/svg",
                            fill: "none",
                            view_box: "0 0 24 24",
                            circle {
                                cx: "12",
                                cy: "12",
                                r: "10",
                                stroke: "currentColor",
                                "stroke-width": "3",
                                "stroke-dasharray": "30, 70",
                                "stroke-linecap": "round",
                            }
                        }
                    }
                    span { class: "text-sm text-gray-700", "Starting workflow..." }
                }
            }
            if let Some(ref r) = result {
                if r.success {
                    div { class: "flex items-center justify-between",
                        div { class: "flex items-center gap-3",
                            span { class: "text-green-500", "✓" }
                            div {
                                p { class: "text-sm text-green-700 font-medium",
                                    "Workflow started successfully"
                                }
                                if let Some(ref id) = r.instance_id {
                                    p { class: "text-xs text-green-600 font-mono",
                                        "Instance: {id}"
                                    }
                                }
                            }
                        }
                        button {
                            class: "text-sm text-gray-500 hover:text-gray-700",
                            onclick: move |_| on_reset.call(()),
                            "Upload Another"
                        }
                    }
                } else {
                    div { class: "flex items-center justify-between",
                        div { class: "flex items-center gap-3",
                            span { class: "text-red-500", "✗" }
                            div {
                                p { class: "text-sm text-red-700 font-medium",
                                    "Failed to start workflow"
                                }
                                if let Some(ref err) = r.error {
                                    p { class: "text-xs text-red-600 font-mono", "{err}" }
                                }
                            }
                        }
                        button {
                            class: "text-sm text-gray-500 hover:text-gray-700",
                            onclick: move |_| on_reset.call(()),
                            "Try Again"
                        }
                    }
                }
            }
            if state == UploadState::UploadFailed && result.is_none() {
                button {
                    class: "w-full px-6 py-3 rounded-lg font-medium text-white bg-blue-600 hover:bg-blue-700",
                    onclick: move |_| on_submit.call(()),
                    "Retry Upload"
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Issue row
// ---------------------------------------------------------------------------

#[component]
fn IssueRow(issue: ValidationIssue) -> Element {
    let row_bg = if issue.severity == ValidationSeverity::Error {
        "bg-red-50"
    } else {
        "bg-yellow-50"
    };
    let dot = if issue.severity == ValidationSeverity::Error {
        "● "
    } else {
        "○ "
    };
    let issue_text: String = if let Some(ref node_id) = issue.node_id {
        format!("{}: {}", node_id, issue.message)
    } else {
        issue.message.clone()
    };
    rsx! {
        div {
            class: "px-4 py-2 flex items-start gap-3 {row_bg}",
            span { class: "text-sm", "{dot}" }
            span { class: "text-sm text-gray-700 flex-1", "{issue_text}" }
        }
    }
}

// ---------------------------------------------------------------------------
// Tab components
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TabIconEnum {
    Text,
    File,
    Graph,
}

#[component]
fn TabButton(
    active: bool,
    on_click: Callback<()>,
    icon: TabIconEnum,
    label: String,
    badge: Option<usize>,
) -> Element {
    let tab_class = if active {
        "border-blue-500 text-blue-600 bg-blue-50/50"
    } else {
        "border-transparent text-gray-500 hover:text-gray-700 hover:border-gray-300"
    };
    rsx! {
        button {
            class: "flex items-center gap-1.5 px-4 py-2.5 text-sm font-medium border-b-2 transition-colors {tab_class}",
            onclick: move |_| on_click.call(()),
            TabIcon { icon }
            span { "{label}" }
            if let Some(count) = badge {
                Badge { count }
            }
        }
    }
}

#[component]
fn TabIcon(icon: TabIconEnum) -> Element {
    rsx! {
        span { class: "text-sm", "{icon_char(&icon)}" }
    }
}

#[component]
fn Badge(count: usize) -> Element {
    let badge_class = if count > 0 {
        "bg-red-100 text-red-700"
    } else {
        "bg-gray-100 text-gray-600"
    };
    rsx! {
        span { class: "ml-1 text-xs px-1.5 py-0.5 rounded-full {badge_class}", "{count}" }
    }
}

fn icon_char(icon: &TabIconEnum) -> char {
    match icon {
        TabIconEnum::Text => '📝',
        TabIconEnum::File => '📂',
        TabIconEnum::Graph => '🔗',
    }
}
