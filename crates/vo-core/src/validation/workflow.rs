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


