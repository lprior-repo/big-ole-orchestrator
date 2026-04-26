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
use vo_types::EffectKind;
use vo_types::{DedupeScope, GuaranteeClass, NodeKind};

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

fn effect_kind_to_sink(kind: EffectKind) -> &'static str {
    match kind {
        EffectKind::HttpCall => "http",
        EffectKind::SqlQuery => "sql",
        EffectKind::BlobWrite => "blob",
    }
}

/// Error returned when a workflow contains `Unsafe` nodes that are not permitted
/// by its guarantee class (ADR-003, ADR-031).
///
/// Only `BestEffort` workflows may contain `Unsafe` nodes, since unsafe nodes
/// break all delivery guarantees by definition.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum UnsafePublishError {
    /// The workflow contains `Unsafe` nodes but its guarantee class does not permit them.
    #[error("workflow '{workflow_name}' has guarantee class '{guarantee_class}' which does not permit Unsafe nodes, but contains {unsafe_node_count} Unsafe node(s): {unsafe_nodes}")]
    UnsafeNotAllowed {
        /// The workflow name.
        workflow_name: String,
        /// The guarantee class of the workflow.
        guarantee_class: String,
        /// The number of Unsafe nodes found.
        unsafe_node_count: usize,
        /// The names of the Unsafe nodes.
        unsafe_nodes: String,
    },
}

/// Specification of a workflow at publish time, carrying both its guarantee class
/// and the full node list for validation.
#[derive(Debug, Clone)]
pub struct WorkflowPublishSpec {
    /// The guarantee class determining delivery semantics.
    pub guarantee_class: GuaranteeClass,
    /// The nodes in the workflow.
    pub nodes: Vec<vo_types::NodeName>,
    /// The corresponding kind for each node (parallel to `nodes`).
    pub node_kinds: Vec<NodeKind>,
    /// Dedupe scope for exactly-once ingress deduplication (ADR-028, ADR-031).
    pub dedupe_scope: DedupeScope,
}

impl WorkflowPublishSpec {
    /// Create a new `WorkflowPublishSpec`.
    #[must_use]
    pub fn new(guarantee_class: GuaranteeClass, nodes: Vec<vo_types::NodeName>, node_kinds: Vec<NodeKind>) -> Self {
        assert_eq!(
            nodes.len(),
            node_kinds.len(),
            "nodes and node_kinds must have the same length"
        );
        Self {
            guarantee_class,
            nodes,
            node_kinds,
            dedupe_scope: DedupeScope::default(),
        }
    }

    /// Returns the workflow name from the first node, or "unknown" if empty.
    #[must_use]
    pub fn workflow_name(&self) -> String {
        self.nodes.first()
            .map(|n| n.to_string())
            .unwrap_or_else(|| "unknown".to_string())
    }
}

/// Validate that a workflow does not contain `Unsafe` nodes when its guarantee
/// class forbids them (ADR-003, ADR-031).
///
/// # Errors
///
/// Returns `UnsafePublishError::UnsafeNotAllowed` if the workflow's guarantee
/// class does not permit unsafe nodes but the workflow contains one or more.
pub fn validate_unsafe_nodes(spec: &WorkflowPublishSpec) -> Result<(), UnsafePublishError> {
    if spec.guarantee_class.permits_unsafe_nodes() {
        return Ok(());
    }

    let unsafe_nodes: Vec<String> = spec
        .nodes
        .iter()
        .zip(&spec.node_kinds)
        .filter_map(|(name, kind)| {
            if *kind == NodeKind::Unsafe {
                Some(name.to_string())
            } else {
                None
            }
        })
        .collect();

    if unsafe_nodes.is_empty() {
        return Ok(());
    }

    Err(UnsafePublishError::UnsafeNotAllowed {
        workflow_name: spec.workflow_name(),
        guarantee_class: spec.guarantee_class.label().to_string(),
        unsafe_node_count: unsafe_nodes.len(),
        unsafe_nodes: unsafe_nodes.join(", "),
    })
}

/// Error returned when an exact-once workflow lacks required dedupe policy (ADR-028, ADR-031).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DedupePolicyError {
    /// The workflow requires exact-once deduplication but has no dedupe scope configured.
    #[error("workflow '{workflow_name}' has guarantee class '{guarantee_class}' which requires dedupe policy, but dedupe scope is '{dedupe_scope}'")]
    Missing {
        /// The workflow name.
        workflow_name: String,
        /// The guarantee class of the workflow.
        guarantee_class: String,
        /// The current dedupe scope value.
        dedupe_scope: String,
    },
}

/// Validate that a workflow with `ExactOnce` guarantee class has a dedupe scope set
/// to `Exact` (ADR-028, ADR-031).
///
/// # Errors
///
/// Returns `DedupePolicyError::Missing` if the workflow's guarantee class requires
/// deduplication but the dedupe scope is not set to `Exact`.
pub fn validate_dedupe_policy(spec: &WorkflowPublishSpec) -> Result<(), DedupePolicyError> {
    if !spec.guarantee_class.requires_deduplication() {
        return Ok(());
    }

    if spec.dedupe_scope == DedupeScope::Exact {
        return Ok(());
    }

    Err(DedupePolicyError::Missing {
        workflow_name: spec.workflow_name(),
        guarantee_class: spec.guarantee_class.label().to_string(),
        dedupe_scope: format!("{:?}", spec.dedupe_scope),
    })
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

    // ========================================================================
    // BDD Tests: Publish Validation for Unsafe Nodes (ADR-003, ADR-031)
    // ========================================================================

    #[test]
    fn given_exact_workflow_with_unsafe_node_when_published_then_validation_rejects() {
        // Given an exact workflow spec contains an Unsafe node
        use vo_types::NodeName;
        let spec = WorkflowPublishSpec::new(
            GuaranteeClass::ExactOnce,
            vec![
                NodeName::parse("entry-point").unwrap(),
                NodeName::parse("unsafe-step").unwrap(),
            ],
            vec![NodeKind::Pure, NodeKind::Unsafe],
        );

        // When publish validation runs
        let result = validate_unsafe_nodes(&spec);

        // Then publish fails and no workflow version is activated
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, UnsafePublishError::UnsafeNotAllowed { .. }));
        let err_msg = err.to_string();
        assert!(err_msg.contains("unsafe-step"));
        assert!(err_msg.contains("exact-once"));
        assert!(err_msg.contains("Unsafe"));
    }

    #[test]
    fn given_at_least_once_workflow_with_unsafe_node_when_published_then_validation_rejects() {
        // AtLeastOnce also does not permit Unsafe nodes
        use vo_types::NodeName;
        let spec = WorkflowPublishSpec::new(
            GuaranteeClass::AtLeastOnce,
            vec![NodeName::parse("unsafe-step").unwrap()],
            vec![NodeKind::Unsafe],
        );

        let result = validate_unsafe_nodes(&spec);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), UnsafePublishError::UnsafeNotAllowed { .. }));
    }

    #[test]
    fn given_best_effort_workflow_with_unsafe_node_when_published_then_validation_accepts() {
        // BestEffort DOES permit Unsafe nodes
        use vo_types::NodeName;
        let spec = WorkflowPublishSpec::new(
            GuaranteeClass::BestEffort,
            vec![
                NodeName::parse("entry-point").unwrap(),
                NodeName::parse("unsafe-step").unwrap(),
            ],
            vec![NodeKind::Pure, NodeKind::Unsafe],
        );

        let result = validate_unsafe_nodes(&spec);

        assert!(result.is_ok());
    }

    #[test]
    fn given_exact_workflow_without_unsafe_node_when_published_then_validation_accepts() {
        // ExactOnce workflow with only Pure and ManagedEffect nodes is valid
        use vo_types::NodeName;
        let spec = WorkflowPublishSpec::new(
            GuaranteeClass::ExactOnce,
            vec![
                NodeName::parse("entry-point").unwrap(),
                NodeName::parse("compute").unwrap(),
                NodeName::parse("effect").unwrap(),
            ],
            vec![NodeKind::Pure, NodeKind::Pure, NodeKind::ManagedEffect],
        );

        let result = validate_unsafe_nodes(&spec);

        assert!(result.is_ok());
    }

    #[test]
    fn given_empty_workflow_when_published_then_validation_accepts() {
        // Empty workflow has no unsafe nodes regardless of guarantee class
        let spec = WorkflowPublishSpec::new(
            GuaranteeClass::ExactOnce,
            vec![],
            vec![],
        );

        let result = validate_unsafe_nodes(&spec);

        assert!(result.is_ok());
    }

    #[test]
    fn given_multiple_unsafe_nodes_in_exact_workflow_then_validation_rejects_all() {
        // When there are multiple Unsafe nodes, they should all be reported
        use vo_types::NodeName;
        let spec = WorkflowPublishSpec::new(
            GuaranteeClass::ExactOnce,
            vec![
                NodeName::parse("unsafe-a").unwrap(),
                NodeName::parse("unsafe-b").unwrap(),
                NodeName::parse("unsafe-c").unwrap(),
            ],
            vec![NodeKind::Unsafe, NodeKind::Unsafe, NodeKind::Unsafe],
        );

        let result = validate_unsafe_nodes(&spec);

        assert!(result.is_err());
        let err = result.unwrap_err();
        let err_msg = err.to_string();
        assert!(err_msg.contains("unsafe-a"));
        assert!(err_msg.contains("unsafe-b"));
        assert!(err_msg.contains("unsafe-c"));
        assert!(err_msg.contains("3"));
    }

    #[test]
    fn validate_unsafe_nodes_error_contains_workflow_name() {
        use vo_types::NodeName;
        let spec = WorkflowPublishSpec::new(
            GuaranteeClass::ExactOnce,
            vec![NodeName::parse("my-workflow").unwrap()],
            vec![NodeKind::Unsafe],
        );

        let err = validate_unsafe_nodes(&spec).unwrap_err();
        let err_msg = err.to_string();
        assert!(err_msg.contains("my-workflow"));
    }

    // ========================================================================
    // BDD Tests: Dedupe Policy Validation for Exact-Once Workflows (ADR-028, ADR-031)
    // ========================================================================

    #[test]
    fn given_exact_workflow_without_dedupe_policy_when_published_then_validation_rejects() {
        use vo_types::{DedupeScope, NodeName};
        let spec = WorkflowPublishSpec {
            guarantee_class: GuaranteeClass::ExactOnce,
            nodes: vec![NodeName::parse("entry-point").unwrap()],
            node_kinds: vec![NodeKind::Pure],
            dedupe_scope: DedupeScope::Unbounded,
        };

        let result = validate_dedupe_policy(&spec);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, DedupePolicyError::Missing { .. }));
        let err_msg = err.to_string();
        assert!(err_msg.contains("entry-point"));
        assert!(err_msg.contains("exact-once"));
        assert!(err_msg.contains("Unbounded"));
    }

    #[test]
    fn given_exact_workflow_with_dedupe_scope_exact_when_published_then_validation_accepts() {
        use vo_types::{DedupeScope, NodeName};
        let spec = WorkflowPublishSpec {
            guarantee_class: GuaranteeClass::ExactOnce,
            nodes: vec![NodeName::parse("entry-point").unwrap()],
            node_kinds: vec![NodeKind::Pure],
            dedupe_scope: DedupeScope::Exact,
        };

        let result = validate_dedupe_policy(&spec);

        assert!(result.is_ok());
    }

    #[test]
    fn given_at_least_once_workflow_without_dedupe_scope_when_published_then_validation_accepts() {
        use vo_types::DedupeScope;
        let spec = WorkflowPublishSpec::new(
            GuaranteeClass::AtLeastOnce,
            vec![],
            vec![],
        );

        let result = validate_dedupe_policy(&spec);

        assert!(result.is_ok());
    }

    #[test]
    fn given_best_effort_workflow_without_dedupe_scope_when_published_then_validation_accepts() {
        use vo_types::DedupeScope;
        let spec = WorkflowPublishSpec::new(
            GuaranteeClass::BestEffort,
            vec![],
            vec![],
        );

        let result = validate_dedupe_policy(&spec);

        assert!(result.is_ok());
    }

    #[test]
    fn dedupe_policy_error_message_contains_all_details() {
        use vo_types::{DedupeScope, NodeName};
        let spec = WorkflowPublishSpec {
            guarantee_class: GuaranteeClass::ExactOnce,
            nodes: vec![NodeName::parse("my-workflow").unwrap()],
            node_kinds: vec![NodeKind::Pure],
            dedupe_scope: DedupeScope::Unbounded,
        };

        let err = validate_dedupe_policy(&spec).unwrap_err();
        let err_msg = err.to_string();
        assert!(err_msg.contains("my-workflow"));
        assert!(err_msg.contains("exact-once"));
        assert!(err_msg.contains("Unbounded"));
    }
}
