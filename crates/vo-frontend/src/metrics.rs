//! Stub metrics recording for suggestion decisions.
//!
//! Provides a no-op metrics store for recording which extension suggestions
//! were accepted or rejected. The real implementation will persist to disk.

use chrono::{DateTime, Utc};
use std::path::Path;

/// Whether a suggestion was accepted or rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuggestionDecision {
    Accepted,
    Rejected,
}

/// A recorded suggestion decision event.
#[derive(Debug, Clone)]
pub struct SuggestionDecisionMetrics {
    pub timestamp: DateTime<Utc>,
    pub suggestion_key: String,
    pub decision: SuggestionDecision,
    pub source: String,
}

/// Stub metrics store. Records are silently discarded.
pub struct MetricsStore {
    _root: std::path::PathBuf,
}

impl MetricsStore {
    pub fn new(root: &Path) -> Self {
        Self {
            _root: root.to_path_buf(),
        }
    }

    pub fn record_suggestion_decision(
        &self,
        _metrics: SuggestionDecisionMetrics,
    ) -> Result<(), String> {
        Ok(())
    }
}
