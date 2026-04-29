//! File upload and parsing for workflow definitions.
//!
//! Handles reading files from the browser, detecting TOML vs JSON format,
//! and parsing into a `WorkflowDefinition`.

use super::types::WorkflowDefinition;

// ---------------------------------------------------------------------------
// File reading (wasm only)
// ---------------------------------------------------------------------------

/// Read the text content of a selected File.
///
/// Uses the browser FileReader API to read the file as text.
/// Returns `None` if reading fails.
#[cfg(all(feature = "sse", target_arch = "wasm32"))]
pub fn read_file_content(file: &web_sys::File) -> Option<String> {
    use wasm_bindgen::JsCast;

    let _file = file.clone();
    let reader = web_sys::FileReader::new().ok()?;

    let reader_clone = reader.clone();
    let _onload =
        wasm_bindgen::closure::Closure::wrap(Box::new(move || {
            let _ = reader_clone; // keep alive
        }) as Box<dyn FnMut()>);
    reader
        .set_onload(Some(_onload.as_ref().unchecked_ref()));
    _onload.forget();

    // For now, return None since FileReader is async and complex to integrate
    // with synchronous Dioxus component state.
    // The editor component handles paste/drop text directly.
    None
}

/// Parse file content as either TOML or JSON.
///
/// Detects the format automatically (JSON starts with `{` or `[`, otherwise TOML).
/// Returns a parsed `WorkflowDefinition` on success.
pub fn parse_content(content: &str, format: FormatHint) -> Result<WorkflowDefinition, String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err("Empty workflow definition".to_string());
    }

    match format {
        FormatHint::Json | FormatHint::Auto if is_json_like(trimmed) => {
            parse_json(trimmed)
        }
        _ => parse_toml(trimmed),
    }
}

/// Detect the format of a content string.
pub fn detect_format(content: &str) -> FormatHint {
    let trimmed = content.trim();
    if is_json_like(trimmed) {
        FormatHint::Json
    } else {
        FormatHint::Toml
    }
}

/// Whether the content looks like JSON (starts with `{` or `[`).
fn is_json_like(content: &str) -> bool {
    let trimmed = content.trim_start();
    trimmed.starts_with('{') || trimmed.starts_with('[')
}

/// Parse content as JSON into a WorkflowDefinition.
fn parse_json(content: &str) -> Result<WorkflowDefinition, String> {
    serde_json::from_str(content).map_err(|e| {
        format!("JSON parse error: {e}")
    })
}

/// Parse content as TOML into a WorkflowDefinition.
fn parse_toml(content: &str) -> Result<WorkflowDefinition, String> {
    toml::from_str(content).map_err(|e| {
        format!("TOML parse error: {e}")
    })
}

/// Hint for the parser about the expected format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatHint {
    Toml,
    Json,
    Auto,
}

// ---------------------------------------------------------------------------
// Content type detection from filename
// ---------------------------------------------------------------------------

/// Detect format from a file extension.
pub fn detect_format_from_filename(filename: &str) -> FormatHint {
    let ext = filename
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "json" => FormatHint::Json,
        "toml" | "cfg" | "ini" => FormatHint::Toml,
        _ => FormatHint::Auto,
    }
}

// ---------------------------------------------------------------------------
// Workflow submission via API
// ---------------------------------------------------------------------------

/// Request body for the workflow creation API endpoint.
#[derive(Debug, Clone, Serialize)]
struct V3StartRequest {
    namespace: String,
    workflow_type: String,
    paradigm: String,
    input: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    workflow_binary_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    instance_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dedupe_key: Option<String>,
}

use serde::Serialize;

/// Submit a workflow definition to the vo-api server.
///
/// Creates a new workflow instance by converting the definition into the
/// V3StartRequest format and POSTing to `/api/v1/workflows`.
#[cfg(all(feature = "sse", target_arch = "wasm32"))]
pub async fn submit_workflow(
    def: &WorkflowDefinition,
    api_base_url: &str,
) -> Result<SubmitResponse, String> {
    use super::types::GuaranteeClassInput;

    // Build input payload from the workflow definition
    let input = serde_json::json!({
        "workflow_name": def.name,
        "guarantee_class": match &def.guarantee_class {
            GuaranteeClassInput::BestEffort => "best_effort",
            GuaranteeClassInput::AtLeastOnce => "at_least_once",
            GuaranteeClassInput::ExactlyOnce => "exactly_once",
            GuaranteeClassInput::AtMostOnce => "at_most_once",
        },
        "nodes": def.nodes,
        "edges": def.edges,
    });

    let body = V3StartRequest {
        namespace: "default".to_string(),
        workflow_type: def.name.clone(),
        paradigm: "dag".to_string(),
        input,
        workflow_binary_hash: None,
        instance_id: None,
        dedupe_key: Some(format!("upload-{}", def.name)),
    };

    let url = format!("{api_base_url}/api/v1/workflows");
    let resp = reqwest::Client::new()
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    if resp.status().is_success() {
        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {e}"))?;
        let instance_id = data
            .get("instance_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        Ok(SubmitResponse {
            success: true,
            instance_id,
        })
    } else {
        let error_msg = resp
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        Err(format!(
            "Upload failed (HTTP {}): {}",
            resp.status(),
            error_msg
        ))
    }
}

/// Response from a workflow submission.
#[derive(Debug, Clone)]
pub struct SubmitResponse {
    pub success: bool,
    pub instance_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_format_json_content() {
        assert_eq!(
            detect_format(r#"{"name": "test"}"#),
            FormatHint::Json
        );
    }

    #[test]
    fn detect_format_json_array() {
        assert_eq!(detect_format(r#"[1, 2, 3]"#), FormatHint::Json);
    }

    #[test]
    fn detect_format_toml_content() {
        assert_eq!(
            detect_format(r#"[workflow]
name = "test""#),
            FormatHint::Toml
        );
    }

    #[test]
    fn detect_format_auto_mixed() {
        assert_eq!(
            detect_format(r#"
{
  "name": "test"
}"#),
            FormatHint::Json
        );
    }

    #[test]
    fn detect_format_from_filename_json() {
        assert_eq!(
            detect_format_from_filename("workflow.json"),
            FormatHint::Json
        );
    }

    #[test]
    fn detect_format_from_filename_toml() {
        assert_eq!(
            detect_format_from_filename("workflow.toml"),
            FormatHint::Toml
        );
    }

    #[test]
    fn detect_format_from_filename_cfg() {
        assert_eq!(
            detect_format_from_filename("workflow.cfg"),
            FormatHint::Toml
        );
    }

    #[test]
    fn detect_format_from_filename_unknown() {
        assert_eq!(
            detect_format_from_filename("workflow.yml"),
            FormatHint::Auto
        );
    }

    #[test]
    fn parse_json_workflow_definition() {
        let content = r#"{
            "name": "test-workflow",
            "guarantee_class": "best_effort",
            "nodes": [
                {
                    "id": "node-1",
                    "name": "First Step",
                    "kind": "pure"
                }
            ],
            "edges": []
        }"#;
        let result = parse_content(content, FormatHint::Json).unwrap();
        assert_eq!(result.name, "test-workflow");
        assert_eq!(result.nodes.len(), 1);
        assert_eq!(result.nodes[0].name, "First Step");
    }

    #[test]
    fn parse_toml_workflow_definition() {
        let content = r#"
name = "test-workflow"
guarantee_class = "best_effort"

[[nodes]]
id = "node-1"
name = "First Step"
kind = "pure"

[edges]
"#;
        let result = parse_content(content, FormatHint::Toml).unwrap();
        assert_eq!(result.name, "test-workflow");
        assert_eq!(result.nodes.len(), 1);
    }

    #[test]
    fn parse_empty_content_fails() {
        assert!(parse_content("", FormatHint::Json).is_err());
        assert!(parse_content("   ", FormatHint::Toml).is_err());
    }

    #[test]
    fn parse_invalid_json_fails() {
        let result = parse_content("{ invalid json", FormatHint::Json);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_lowercase()
            .contains("parse"));
    }
}
