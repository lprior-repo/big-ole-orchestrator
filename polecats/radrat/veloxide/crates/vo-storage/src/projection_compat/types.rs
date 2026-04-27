//! Projection schema version compatibility checking (ADR-035).
//!
//! This module provides Data → Calc → Actions layered functions for validating
//! projection schema versions against a compatibility window.
//!
//! ## Data Layer
//! - `ProjectionCompatibilityWindow` — validated window bounds [min, max]
//! - `ProjectionCompat` — exhaustive partition enum for version compatibility
//! - `ProjectionError` — structured error taxonomy
//!
//! ## Calc Layer (pure)
//! - `projection_compat_window()` — validated window constructor
//! - `window_is_valid()` — predicate for window validity
//! - `window_min_supported()` / `window_max_supported()` — accessors
//! - `check_projection_compat()` — core pure compatibility function
//! - `is_projection_compatible()` — boolean wrapper
//!
//! ## Actions Layer (fallible)
//! - `validate_projection_payload()` — JSON parsing + compat check
//! - `validate_projection_batch()` — short-circuit batch validation
//! - `CompatibleProjectionIterator` — iterator guard wrapper

use serde::{Deserialize, Serialize};

// ============================================================================
// Data Layer — Types
// ============================================================================

/// Compatibility window for projection schema versions.
///
/// The window defines the range `[min_supported, max_supported]` of acceptable
/// versions. Versions outside this range are stale.
///
/// Construct via [`projection_compat_window()`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionCompatibilityWindow {
    pub(crate) min_supported: u8,
    pub(crate) max_supported: u8,
}

/// Result of checking a projection's schema version against the compatibility window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectionCompat {
    /// Projection version matches engine version exactly. Safe to use as-is.
    Fresh,
    /// Projection version is older than engine version but within the window.
    /// Upcasting required before use.
    NeedsUpcast {
        /// The projection's schema version that needs upcasting.
        from: u8,
        /// The target schema version after upcasting.
        to: u8,
    },
    /// Projection version is outside the compatibility window.
    /// Cannot be safely consumed; must rebuild from event log.
    StaleTooOld {
        /// The projection's schema version that was detected as stale.
        projection: u8,
        /// The minimum supported version of the compatibility window.
        window_min: u8,
    },
    /// Projection version is zero (invalid). Always stale.
    StaleZeroVersion,
}

impl ProjectionCompat {
    /// Returns `true` if this result permits further processing (Fresh or `NeedsUpcast`).
    #[must_use]
    pub const fn is_compatible(self) -> bool {
        matches!(self, Self::Fresh | Self::NeedsUpcast { .. })
    }
}

/// Errors that can occur during projection compatibility checking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectionError {
    /// Projection payload is outside the compatibility window.
    StaleProjection(u8, u8, u8),

    /// Projection payload has no schema version field.
    MissingSchemaVersion,

    /// Projection payload's schema version field is not a valid u8.
    InvalidSchemaVersionType,

    /// Projection payload's schema version exceeds engine's maximum supported version.
    SchemaVersionExceedsMax(u8, u8),

    /// Compatibility window does not satisfy preconditions (min >= 1, max >= min).
    WindowMisconfigured { min: u8, max: u8 },

    /// Batch validation failed — at least one payload could not be decoded.
    BatchDecodeFailed(String),
}

impl std::fmt::Display for ProjectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StaleProjection(version, min, max) => {
                write!(
                    f,
                    "projection schema version {version} is stale (window: {min}..{max})"
                )
            }
            Self::MissingSchemaVersion => {
                write!(f, "projection payload missing schema version field")
            }
            Self::InvalidSchemaVersionType => {
                write!(f, "schema version has invalid type (expected u8)")
            }
            Self::SchemaVersionExceedsMax(version, max) => {
                write!(
                    f,
                    "schema version {version} exceeds maximum supported {max}"
                )
            }
            Self::WindowMisconfigured { min, max } => {
                write!(
                    f,
                    "compatibility window misconfigured (min: {min}, max: {max})"
                )
            }
            Self::BatchDecodeFailed(msg) => {
                write!(f, "batch decode failed: {msg}")
            }
        }
    }
}

impl std::error::Error for ProjectionError {}

impl ProjectionError {
    /// Returns true if this error is a stale projection error.
    #[must_use]
    pub const fn is_stale(&self) -> bool {
        matches!(self, Self::StaleProjection(..))
    }
}
