//! Effect domain types for vo-core (ADR-030).
//!
//! This module re-exports the core effect lifecycle types from vo-types and provides
//! vo-core-specific effect transition logic and invariants.
//!
//! # Domain Model
//!
//! An effect transitions through a strict lifecycle:
//! ```text
//! Prepared → Committed  (via Commit transition)
//! Prepared → RolledBack (via Rollback transition)
//! ```
//!
//! The Committed and RolledBack states are **terminal** — no further transitions
//! are permitted. This invariant is enforced by [`apply_effect_transition`].
//!
//! # Architectural Notes
//!
//! - Data types ([`EffectIntent`], [`EffectKind`], [`EffectRecord`]) live in vo-types
//! - State machine logic ([`apply_effect_transition`]) lives in vo-types
//! - vo-core re-exports and tests the domain model

use vo_types::TimestampMs;
use vo_types::{
    CompensationPolicy, EffectIntent, EffectKind, EffectRecord,
    EffectTransitionError,
};

#[cfg(test)]
mod tests;

// ============================================================================
// Re-exports from vo-types
// ============================================================================

pub use CompensationPolicy::{Automatic, Manual, None as CompensationNone};
pub use EffectIntent::{Committed, Prepared, RolledBack};
pub use EffectKind::{BlobWrite, HttpCall, SqlQuery};

// ============================================================================
// Domain constants
// ============================================================================

/// Maximum allowed age of a committed effect record before requiring archival.
pub const MAX_EFFECT_RECORD_AGE_MS: i64 = 90 * 24 * 60 * 60 * 1000; // 90 days

// ============================================================================
// Domain predicates
// ============================================================================

/// Returns `true` if the effect can legally transition to `Committed`.
///
/// An effect can only transition to `Committed` if it is currently in
/// [`EffectIntent::Prepared`]. This is a direct encoding of INV-EFF-001.
///
/// # Formal Encoding
///
/// ```text
/// can_commit(e) ≡ e.status = Prepared
/// ```
#[must_use]
pub fn can_commit(effect: &EffectRecord) -> bool {
    effect.status() == EffectIntent::Prepared
}

/// Returns `true` if the effect can legally transition to `RolledBack`.
///
/// An effect can only transition to `RolledBack` if it is currently in
/// [`EffectIntent::Prepared`]. This is a direct encoding of INV-EFF-001.
///
/// # Formal Encoding
///
/// ```text
/// can_rollback(e) ≡ e.status = Prepared
/// ```
#[must_use]
pub fn can_rollback(effect: &EffectRecord) -> bool {
    effect.status() == EffectIntent::Prepared
}

/// Returns `true` if the given effect is in a terminal state.
///
/// Terminal states are [`EffectIntent::Committed`] and [`EffectIntent::RolledBack`].
/// Once an effect reaches a terminal state, no further transitions are permitted
/// (INV-EFF-002).
#[must_use]
pub fn is_terminal(effect: &EffectRecord) -> bool {
    effect.status().is_terminal()
}

/// Check whether committing the given effect would violate the lifecycle invariant.
///
/// INV-EFF-001: An effect cannot transition to Committed without first being Prepared.
///
/// This predicate returns `Ok(())` if the commit is valid, or an error describing
/// the violation.
pub fn validate_commit_precondition(effect: &EffectRecord) -> Result<(), EffectTransitionError> {
    if !can_commit(effect) {
        return Err(EffectTransitionError::InvalidTransition);
    }
    Ok(())
}

// ============================================================================
// Transition helpers
// ============================================================================

/// Commit the given effect, returning a new EffectRecord with Committed status.
///
/// # Errors
///
/// Returns `EffectTransitionError::InvalidTransition` if the effect is not in
/// [`EffectIntent::Prepared`] state.
///
/// # Formal Encoding
///
/// ```text
/// commit(e) = EffectRecord with status = Committed, committed_at = now()
/// precondition: e.status = Prepared
/// ```
pub fn commit_effect(
    effect: &EffectRecord,
    now: TimestampMs,
) -> Result<EffectRecord, EffectTransitionError> {
    validate_commit_precondition(effect)?;
    let committed_effect = EffectRecord::new(
        effect.intent_id().to_string(),
        effect.kind(),
        effect.params_json().clone(),
        EffectIntent::Committed,
        Some(now),
    );
    committed_effect.ok_or(EffectTransitionError::InvalidTransition)
}

/// Roll back the given effect, returning a new EffectRecord with RolledBack status.
///
/// # Errors
///
/// Returns `EffectTransitionError::InvalidTransition` if the effect is not in
/// [`EffectIntent::Prepared`] state.
pub fn rollback_effect(effect: &EffectRecord) -> Result<EffectRecord, EffectTransitionError> {
    if !can_rollback(effect) {
        return Err(EffectTransitionError::InvalidTransition);
    }
    let rolled_back_effect = EffectRecord::new(
        effect.intent_id().to_string(),
        effect.kind(),
        effect.params_json().clone(),
        EffectIntent::RolledBack,
        None,
    );
    rolled_back_effect.ok_or(EffectTransitionError::InvalidTransition)
}
