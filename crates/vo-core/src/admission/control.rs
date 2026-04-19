//! Admission control types and trait for ADR-028 deduplication and ADR-029 fencing.
//!
//! This module defines:
//! - `AdmissionResult` — exhaustive enum of admission outcomes
//! - `DedupeToken` — opaque token issued on admission
//! - `RejectionReason` — why a command was rejected
//! - `AdmissionCheck` — pure trait for dedupe and fence checks
//!
//! # Purity Contract
//!
//! All types are pure data. No I/O, no async, no side effects.
//! The trait is pure — implementations provide persistence-backed logic.

use std::fmt;

use serde::{Deserialize, Serialize};

use vo_types::{DedupeKey, FenceToken, InstanceId, StepId};

// ============================================================================
// DedupeToken
// ============================================================================

/// Opaque token issued on successful admission for exactly-once tracking.
///
/// Non-empty by construction. Used to correlate the admission decision
/// with downstream processing.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DedupeToken(String);

// GREEN: DedupeToken::parse added to make dedupe_token_parse_rejects_empty_string pass
// RED evidence: cargo test failed with E0599 "no method named `parse`" (run at 03:12 UTC)
impl DedupeToken {
    /// Create a new `DedupeToken`.
    ///
    /// # Precondition
    ///
    /// `value` must be non-empty. Callers MUST ensure non-empty input.
    /// Per INV-ADM-004: "DedupeToken is non-empty on construction".
    ///
    /// Prefer `parse` for validated construction.
    #[must_use]
    pub fn new(value: String) -> Self {
        Self(value)
    }

    /// Parse a string into a `DedupeToken`, rejecting empty input.
    ///
    /// # Errors
    ///
    /// Returns `ParseError::Empty` if the input is empty.
    pub fn parse(input: &str) -> Result<Self, vo_types::ParseError> {
        if input.is_empty() {
            return Err(vo_types::ParseError::Empty {
                type_name: "DedupeToken",
            });
        }
        Ok(Self(input.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DedupeToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

// ============================================================================
// RejectionReason
// ============================================================================

/// Exhaustive enum of why a command was rejected during admission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RejectionReason {
    /// The dedupe key is missing for an exact-workflow ingress surface.
    MissingDedupeKey,
    /// The dedupe key exceeds the maximum allowed length.
    DedupeKeyTooLong {
        max_length: usize,
        actual_length: usize,
    },
    /// The fence token does not match the current lease (stale execution attempt).
    FenceTokenMismatch {
        expected: FenceToken,
        actual: FenceToken,
    },
    /// No active lease exists for the (instance_id, step_id) pair.
    NoActiveLease,
    /// A generic admission policy violation with a human-readable message.
    PolicyViolation(String),
}

impl fmt::Display for RejectionReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RejectionReason::MissingDedupeKey => write!(f, "missing dedupe key"),
            RejectionReason::DedupeKeyTooLong {
                max_length,
                actual_length,
            } => {
                write!(
                    f,
                    "dedupe key too long: {actual_length} characters exceeds maximum of {max_length}"
                )
            }
            RejectionReason::FenceTokenMismatch { expected, actual } => {
                write!(f, "fence token mismatch: expected {expected}, got {actual}")
            }
            RejectionReason::NoActiveLease => {
                write!(f, "no active lease for the given instance and step")
            }
            RejectionReason::PolicyViolation(msg) => write!(f, "policy violation: {msg}"),
        }
    }
}

impl std::error::Error for RejectionReason {}

// ============================================================================
// AdmissionResult
// ============================================================================

/// Exhaustive enum of admission check outcomes.
///
/// Every admission check returns exactly one variant. No panics, no `Option`, no `unwrap`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdmissionResult {
    /// Command is admitted; carries the dedupe token for idempotent tracking.
    Admitted { dedupe_token: DedupeToken },
    /// Command is rejected due to a policy violation or precondition failure.
    Rejected { reason: RejectionReason },
    /// Command is a duplicate of an already-admitted command.
    Duplicate { original_instance_id: InstanceId },
}

// ============================================================================
// AdmissionCheck Trait
// ============================================================================

/// Pure trait for admission control with deduplication and fencing.
///
/// Implementations provide persistence-backed logic. The trait itself
/// is pure — no I/O, no async, no side effects.
///
/// # Design (ADR-028, ADR-029)
///
/// - `check_deduplicate`: Ensures exactly-once ingress by rejecting duplicates.
/// - `check_fence`: Ensures only the current lease holder can commit.
pub trait AdmissionCheck {
    /// Check deduplication for an incoming command.
    ///
    /// Returns `AdmissionResult::Admitted` if the command is new,
    /// `AdmissionResult::Duplicate` if a prior command with the same dedupe key
    /// was already admitted, or `AdmissionResult::Rejected` if the dedupe key
    /// is invalid.
    fn check_deduplicate(&self, dedupe_key: &DedupeKey) -> AdmissionResult;

    /// Check fencing for a step execution attempt.
    ///
    /// Returns `AdmissionResult::Admitted` if the fence token matches the
    /// current lease, `AdmissionResult::Rejected` if the token is stale or
    /// no lease exists.
    fn check_fence(
        &self,
        instance_id: &InstanceId,
        step_id: &StepId,
        fence_token: &FenceToken,
    ) -> AdmissionResult;
}
