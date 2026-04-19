//! Stub flow extension system.
//!
//! These types and functions provide the interface for workflow extension
//! suggestions, presets, and patch previews. Currently returns empty results
//! as the real extension engine has not been ported yet.

use crate::ui::graph::Workflow;

/// Preview of what an extension patch would add to the workflow.
#[derive(Debug, Clone)]
pub struct ExtensionPatchPreview {
    pub nodes: Vec<crate::ui::graph::Node>,
    pub connections: Vec<crate::ui::graph::Connection>,
}

/// Priority level for flow extension suggestions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionPriority {
    High,
    Medium,
    Low,
}

/// A single flow extension suggestion.
#[derive(Debug, Clone)]
pub struct FlowExtension {
    pub key: String,
    pub title: String,
    pub rationale: String,
    pub priority: ExtensionPriority,
}

/// A named preset bundling multiple extension keys.
#[derive(Debug, Clone)]
pub struct ExtensionPreset {
    pub key: String,
    pub title: String,
    pub description: String,
    pub extension_keys: Vec<String>,
}

/// Result of applying a single extension.
#[derive(Debug, Clone)]
pub struct ExtensionApplyResult {
    pub created_nodes: Vec<String>,
}

/// Result of resolving a preset for preview/apply.
#[derive(Debug, Clone)]
pub struct ExtensionPresetResolution {
    pub ordered_keys: Vec<String>,
    pub conflicts: Vec<String>,
}

/// Preview what an extension would add. Returns `Ok(None)` for unknown keys.
pub fn preview_extension(
    _workflow: &Workflow,
    _key: &str,
) -> Result<Option<ExtensionPatchPreview>, String> {
    Ok(None)
}

/// Apply an extension to the workflow. Stub — always returns an error.
pub fn apply_extension(
    _workflow: &mut Workflow,
    _key: &str,
) -> Result<ExtensionApplyResult, String> {
    Err("extension engine not yet implemented".to_string())
}

/// List extension presets. Stub — returns empty.
pub fn extension_presets() -> Vec<ExtensionPreset> {
    Vec::new()
}

/// Suggest extensions for the current workflow. Stub — returns empty.
pub fn suggest_extensions(_workflow: &Workflow) -> Vec<FlowExtension> {
    Vec::new()
}

/// Resolve a preset into ordered keys with conflict detection. Stub — returns error.
pub fn resolve_extension_preset(
    _workflow: &Workflow,
    _key: &str,
) -> Result<ExtensionPresetResolution, String> {
    Err("preset engine not yet implemented".to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn preview_extension_returns_none_for_unknown_key() {
        let workflow = crate::ui::graph::Workflow::new("test".to_string());
        let result = crate::flow_extender::preview_extension(&workflow, "nonexistent-key");
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }
}
