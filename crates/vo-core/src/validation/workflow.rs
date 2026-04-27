//! Workflow sink validation for publish-time rejection of unsupported sinks.
//!
//! This module implements the validation that ensures no published workflow
//! can contain a managed effect targeting an unknown sink.
//!
//! # Architecture
//!
//! - Data: `KnownSinks` registry (set of accepted sink identifiers)
//! - Calc: `validate_workflow_sinks` pure function
//! - Error: `UnsupportedSinkError` for rejection reporting
//!
//! # Validation Contract
//!
//! Per EARS requirements:
//! - WHEN a workflow definition is published with an unsupported sink
//!   THE SYSTEM SHALL reject the publication synchronously
//! - No published workflow can contain an effect targeting an unknown sink

use std::collections::HashSet;
use std::fmt;
use thiserror::Error;
use vo_types::{EffectKind, GuaranteeClass, NodeKind};

/// The set of known sink identifiers that are allowed in workflows.
///
/// A "sink" is the target system for a managed effect (e.g., "blob", "sql", "http").
/// Any sink not in this set is considered unsupported.
#[derive(Debug, Clone)]
pub struct KnownSinks {
    sinks: HashSet<String>,
}

impl KnownSinks {
    /// Create a new `KnownSinks` with the default set of system sinks.
    ///
    /// The default sinks correspond to the known `EffectKind` categories:
    /// - `blob` - blob storage writes (S3, GCS, etc.)
    /// - `http` - HTTP API calls (REST, webhooks, etc.)
    /// - `sql` - SQL database queries/writes
    #[must_use]
    pub fn default_sinks() -> Self {
        let sinks = ["blob", "http", "sql"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        Self { sinks }
    }

    /// Create a `KnownSinks` with a custom set of sink identifiers.
    #[must_use]
    pub fn new(sinks: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let sinks = sinks.into_iter().map(|s| s.into()).collect();
        Self { sinks }
    }

    /// Check if the given sink identifier is known/supported.
    #[must_use]
    pub fn contains(&self, sink: &str) -> bool {
        self.sinks.contains(sink)
    }

    /// Returns the number of known sinks.
    #[must_use]
    pub fn len(&self) -> usize {
        self.sinks.len()
    }

    /// Returns true if there are no known sinks.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sinks.is_empty()
    }
}

impl Default for KnownSinks {
    fn default() -> Self {
        Self::default_sinks()
    }
}

impl PartialEq for KnownSinks {
    fn eq(&self, other: &Self) -> bool {
        self.sinks == other.sinks
    }
}

impl Eq for KnownSinks {}

impl fmt::Display for KnownSinks {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sorted: Vec<&str> = self.sinks.iter().map(|s| s.as_str()).collect();
        write!(f, "[{}]", sorted.join(", "))
    }
}

/// Error returned when a workflow contains a reference to an unsupported sink.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum UnsupportedSinkError {
    /// The workflow contains a managed effect targeting an unknown sink.
    #[error("unsupported sink: '{sink}' is not a known sink (known sinks: {known_sinks})")]
    UnknownSink {
        /// The unknown sink identifier that was encountered.
        sink: String,
        /// Comma-separated list of known sink identifiers.
        known_sinks: String,
    },

    /// The workflow contains a managed effect with an empty sink identifier.
    #[error("empty sink identifier in managed effect")]
    EmptySink,
}

impl UnsupportedSinkError {
    /// Returns the sink identifier that caused the error, if available.
    #[must_use]
    pub fn sink_identifier(&self) -> Option<&str> {
        match self {
            Self::UnknownSink { sink, .. } => Some(sink),
            Self::EmptySink => None,
        }
    }

    /// Returns the error code suitable for API responses.
    #[must_use]
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::UnknownSink { .. } => "unsupported_sink",
            Self::EmptySink => "empty_sink",
        }
    }
}

/// Validator for workflow sink compliance.
///
/// This struct provides the sink validation functionality with
/// a configurable set of known sinks.
#[derive(Debug, Clone)]
pub struct WorkflowSinkValidator {
    known_sinks: KnownSinks,
}

impl WorkflowSinkValidator {
    /// Create a new validator with the default set of known sinks.
    #[must_use]
    pub fn new() -> Self {
        Self {
            known_sinks: KnownSinks::default(),
        }
    }

    /// Create a validator with a custom set of known sinks.
    #[must_use]
    pub fn with_sinks(sinks: KnownSinks) -> Self {
        Self { known_sinks: sinks }
    }

    /// Validate that a sink identifier is known/supported.
    ///
    /// # Errors
    ///
    /// Returns `UnsupportedSinkError::UnknownSink` if the sink is not in the known set.
    /// Returns `UnsupportedSinkError::EmptySink` if the sink is empty.
    pub fn validate_sink(&self, sink: &str) -> Result<(), UnsupportedSinkError> {
        if sink.is_empty() {
            return Err(UnsupportedSinkError::EmptySink);
        }
        if !self.known_sinks.contains(sink) {
            let known_sinks_str = self
                .known_sinks
                .sinks
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
            return Err(UnsupportedSinkError::UnknownSink {
                sink: sink.to_string(),
                known_sinks: known_sinks_str,
            });
        }
        Ok(())
    }

    /// Validate a collection of sink identifiers.
    ///
    /// # Errors
    ///
    /// Returns the first error encountered, if any.
    pub fn validate_sinks<'a>(
        &self,
        sinks: impl IntoIterator<Item = &'a str>,
    ) -> Result<(), UnsupportedSinkError> {
        for sink in sinks {
            self.validate_sink(sink)?;
        }
        Ok(())
    }

    /// Returns a reference to the known sinks registry.
    #[must_use]
    pub fn known_sinks(&self) -> &KnownSinks {
        &self.known_sinks
    }
}

impl Default for WorkflowSinkValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Validate a workflow's managed effects against known sinks.
///
/// This is a convenience function that creates a default validator
/// and checks all the given sink identifiers.
///
/// # Errors
///
/// Returns `UnsupportedSinkError` if any sink is unknown or empty.
pub fn validate_workflow_sinks<'a>(
    sinks: impl IntoIterator<Item = &'a str>,
) -> Result<(), UnsupportedSinkError> {
    let validator = WorkflowSinkValidator::new();
    validator.validate_sinks(sinks)
}

/// Validate that a workflow's managed effects target known sinks.
///
/// This function checks that all managed effects in a workflow target
/// sinks that are in the known sinks set (blob, http, sql).
///
/// # Errors
///
/// Returns `UnsupportedSinkError` if any managed effect targets an unknown sink.
#[allow(dead_code)]
pub fn validate_workflow_effects(
    effect_kinds: impl IntoIterator<Item = EffectKind>,
) -> Result<(), UnsupportedSinkError> {
    validate_effect_kinds(effect_kinds)
}

/// Validate that a workflow's effect kinds target known sinks.
///
/// Each `EffectKind` maps to a specific sink identifier:
/// - `EffectKind::HttpCall` → "http"
/// - `EffectKind::SqlQuery` → "sql"
/// - `EffectKind::BlobWrite` → "blob"
///
/// # Errors
///
/// Returns `UnsupportedSinkError` if any effect kind targets an unknown sink.
#[allow(dead_code)]
pub fn validate_effect_kinds(
    effect_kinds: impl IntoIterator<Item = EffectKind>,
) -> Result<(), UnsupportedSinkError> {
    let validator = WorkflowSinkValidator::new();
    for kind in effect_kinds {
        let sink = effect_kind_to_sink(kind);
        validator.validate_sink(sink)?;
    }
    Ok(())
}

/// Error returned when a ManagedEffect node targets an unsupported connector sink.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("managed effect '{node_name}' targets unsupported connector sink: '{sink}' (known sinks: {known_sinks})")]
pub struct UnsupportedConnectorSink {
    pub node_name: String,
    pub sink: String,
    pub known_sinks: String,
}

impl UnsupportedConnectorSink {
    #[must_use]
    pub fn error_code() -> &'static str {
        "unsupported_connector_sink"
    }
}

/// Validate that all ManagedEffect nodes in a workflow target known connector sinks.
///
/// Non-ManagedEffect nodes are ignored. Only nodes with `NodeKind::ManagedEffect`
/// are checked against the known sinks registry.
///
/// # Errors
///
/// Returns `UnsupportedConnectorSink` if any ManagedEffect node targets a sink
/// not in the known set.
pub fn validate_managed_effect_sinks(
    nodes: &[(NodeKind, &str, &str)],
    known_sinks: &KnownSinks,
) -> Result<(), UnsupportedConnectorSink> {
    for (kind, name, sink) in nodes {
        if *kind == NodeKind::ManagedEffect {
            if sink.is_empty() {
                return Err(UnsupportedConnectorSink {
                    node_name: (*name).to_string(),
                    sink: sink.to_string(),
                    known_sinks: known_sinks.to_string(),
                });
            }
            if !known_sinks.contains(sink) {
                return Err(UnsupportedConnectorSink {
                    node_name: (*name).to_string(),
                    sink: sink.to_string(),
                    known_sinks: known_sinks.to_string(),
                });
            }
        }
    }
    Ok(())
}

fn effect_kind_to_sink(kind: EffectKind) -> &'static str {
    match kind {
        EffectKind::HttpCall => "http",
        EffectKind::SqlQuery => "sql",
        EffectKind::BlobWrite => "blob",
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum UnsafeNodeError {
    #[error(
        "exact-once workflow contains unsafe node '{node_name}' at index {node_index}; \
         exact-once guarantee class does not permit unsafe nodes"
    )]
    UnsafeNodeInExactWorkflow {
        node_name: String,
        node_index: usize,
    },
}

impl UnsafeNodeError {
    #[must_use]
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::UnsafeNodeInExactWorkflow { .. } => "unsafe_node_in_exact_workflow",
        }
    }
}

pub fn validate_exact_workflow_node_kinds(
    guarantee_class: GuaranteeClass,
    nodes: &[NodeDescriptor],
) -> Result<(), UnsafeNodeError> {
    if guarantee_class.permits_unsafe_nodes() {
        return Ok(());
    }
    for (idx, node) in nodes.iter().enumerate() {
        if node.kind == NodeKind::Unsafe {
            return Err(UnsafeNodeError::UnsafeNodeInExactWorkflow {
                node_name: node.name.clone(),
                node_index: idx,
            });
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeDescriptor {
    pub name: String,
    pub kind: NodeKind,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_sinks_default_contains_blob_http_sql() {
        let sinks = KnownSinks::default_sinks();
        assert!(sinks.contains("blob"));
        assert!(sinks.contains("http"));
        assert!(sinks.contains("sql"));
        assert_eq!(sinks.len(), 3);
    }

    #[test]
    fn known_sinks_does_not_contain_unknown() {
        let sinks = KnownSinks::default_sinks();
        assert!(!sinks.contains("unknown-sink"));
        assert!(!sinks.contains(""));
    }

    #[test]
    fn known_sinks_with_custom_sinks() {
        let sinks = KnownSinks::new(["custom1", "custom2"]);
        assert!(sinks.contains("custom1"));
        assert!(sinks.contains("custom2"));
        assert!(!sinks.contains("blob"));
    }

    #[test]
    fn validator_accepts_known_sink() {
        let validator = WorkflowSinkValidator::new();
        assert!(validator.validate_sink("blob").is_ok());
        assert!(validator.validate_sink("http").is_ok());
        assert!(validator.validate_sink("sql").is_ok());
    }

    #[test]
    fn validator_rejects_unknown_sink() {
        let validator = WorkflowSinkValidator::new();
        let result = validator.validate_sink("unknown-sink");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, UnsupportedSinkError::UnknownSink { .. }));
        assert_eq!(err.error_code(), "unsupported_sink");
    }

    #[test]
    fn validator_rejects_empty_sink() {
        let validator = WorkflowSinkValidator::new();
        let result = validator.validate_sink("");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, UnsupportedSinkError::EmptySink));
        assert_eq!(err.error_code(), "empty_sink");
    }

    #[test]
    fn validator_error_message_contains_sink_and_known() {
        let validator = WorkflowSinkValidator::new();
        let result = validator.validate_sink("unknown-sink");
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("unknown-sink"));
        assert!(msg.contains("blob"));
        assert!(msg.contains("http"));
        assert!(msg.contains("sql"));
    }

    #[test]
    fn validate_workflow_sinks_convenience_function() {
        assert!(validate_workflow_sinks(["blob", "sql"]).is_ok());
        assert!(validate_workflow_sinks(["unknown"]).is_err());
        assert!(validate_workflow_sinks(["blob", ""]).is_err());
    }

    #[test]
    fn unsupported_sink_error_sink_identifier() {
        let err = UnsupportedSinkError::UnknownSink {
            sink: "test-sink".to_string(),
            known_sinks: "blob, http, sql".to_string(),
        };
        assert_eq!(err.sink_identifier(), Some("test-sink"));

        let empty_err = UnsupportedSinkError::EmptySink;
        assert_eq!(empty_err.sink_identifier(), None);
    }

    #[test]
    fn known_sinks_display() {
        let sinks = KnownSinks::default_sinks();
        let display = format!("{}", sinks);
        assert!(display.contains("blob"));
        assert!(display.contains("http"));
        assert!(display.contains("sql"));
    }

    #[test]
    fn workflow_sink_validator_with_custom_sinks() {
        let custom_sinks = KnownSinks::new(["custom-sink"]);
        let validator = WorkflowSinkValidator::with_sinks(custom_sinks);
        assert!(validator.validate_sink("custom-sink").is_ok());
        assert!(validator.validate_sink("blob").is_err());
    }

    #[test]
    fn validate_multiple_sinks_returns_first_error() {
        let validator = WorkflowSinkValidator::new();
        let result = validator.validate_sinks(["blob", "unknown", "sql"]);
        assert!(result.is_err());
    }

    #[test]
    fn validate_multiple_known_sinks_succeeds() {
        let validator = WorkflowSinkValidator::new();
        let result = validator.validate_sinks(["blob", "http", "sql"]);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_effect_kinds_with_all_known_effects_succeeds() {
        use vo_types::EffectKind;
        let result = validate_effect_kinds([
            EffectKind::HttpCall,
            EffectKind::SqlQuery,
            EffectKind::BlobWrite,
        ]);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_effect_kinds_http_call_maps_to_http_sink() {
        use vo_types::EffectKind;
        let result = validate_effect_kinds([EffectKind::HttpCall]);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_effect_kinds_sql_query_maps_to_sql_sink() {
        use vo_types::EffectKind;
        let result = validate_effect_kinds([EffectKind::SqlQuery]);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_effect_kinds_blob_write_maps_to_blob_sink() {
        use vo_types::EffectKind;
        let result = validate_effect_kinds([EffectKind::BlobWrite]);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_workflow_effects_with_all_known_effects_succeeds() {
        use vo_types::EffectKind;
        let effect_kinds = [
            EffectKind::HttpCall,
            EffectKind::SqlQuery,
            EffectKind::BlobWrite,
        ];
        let result = validate_workflow_effects(effect_kinds);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_workflow_effects_rejects_empty_sink() {
        let result = validate_workflow_sinks([""]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.error_code(), "empty_sink");
    }

    #[test]
    fn given_exact_workflow_with_unsafe_node_when_published_then_validation_rejects() {
        use super::{NodeDescriptor, UnsafeNodeError};
        use vo_types::{GuaranteeClass, NodeKind};

        let nodes = vec![
            NodeDescriptor {
                name: "safe_step".to_string(),
                kind: NodeKind::Pure,
            },
            NodeDescriptor {
                name: "dangerous_step".to_string(),
                kind: NodeKind::Unsafe,
            },
        ];

        let result =
            validate_exact_workflow_node_kinds(GuaranteeClass::ExactOnce, &nodes);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(
                &err,
                UnsafeNodeError::UnsafeNodeInExactWorkflow { node_name, node_index }
                if node_name == "dangerous_step" && *node_index == 1
            ),
            "expected UnsafeNodeInExactWorkflow with node_name='dangerous_step' index=1, got {:?}",
            err
        );
        assert_eq!(err.error_code(), "unsafe_node_in_exact_workflow");
    }
}
