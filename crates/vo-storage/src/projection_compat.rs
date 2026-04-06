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
    min_supported: u8,
    max_supported: u8,
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

// ============================================================================
// Calc Layer — Pure Functions
// ============================================================================

/// Constructs a compatibility window, validating preconditions.
///
/// # Errors
///
/// Returns `ProjectionError::WindowMisconfigured` if `min < 1` or `max < min`.
pub const fn projection_compat_window(
    min_supported: u8,
    max_supported: u8,
) -> Result<ProjectionCompatibilityWindow, ProjectionError> {
    if min_supported < 1 {
        return Err(ProjectionError::WindowMisconfigured {
            min: min_supported,
            max: max_supported,
        });
    }
    if max_supported < min_supported {
        return Err(ProjectionError::WindowMisconfigured {
            min: min_supported,
            max: max_supported,
        });
    }
    Ok(ProjectionCompatibilityWindow {
        min_supported,
        max_supported,
    })
}

/// Returns true if the window satisfies invariant preconditions.
#[must_use]
pub const fn window_is_valid(window: &ProjectionCompatibilityWindow) -> bool {
    window.min_supported >= 1 && window.max_supported >= window.min_supported
}

/// Returns the min supported version for this window.
#[must_use]
pub const fn window_min_supported(window: &ProjectionCompatibilityWindow) -> u8 {
    window.min_supported
}

/// Returns the max supported version for this window.
#[must_use]
pub const fn window_max_supported(window: &ProjectionCompatibilityWindow) -> u8 {
    window.max_supported
}

/// Full compatibility assessment between a projection's schema version and the
/// engine's current compatibility window.
///
/// The version space is exhaustively partitioned:
/// - `version == max_supported` → `Fresh`
/// - `min_supported <= version < max_supported` → `NeedsUpcast`
/// - `version > max_supported` → `StaleTooOld` (too new)
/// - `0 < version < min_supported` → `StaleTooOld` (too old)
/// - `version == 0` → `StaleZeroVersion`
///
/// # Errors
///
/// Returns `ProjectionError::WindowMisconfigured` if window is invalid.
pub fn check_projection_compat(
    projection_version: u8,
    window: &ProjectionCompatibilityWindow,
) -> Result<ProjectionCompat, ProjectionError> {
    if !window_is_valid(window) {
        return Err(ProjectionError::WindowMisconfigured {
            min: window.min_supported,
            max: window.max_supported,
        });
    }

    // Version 0 is always stale — never valid for any projection record
    if projection_version == 0 {
        return Ok(ProjectionCompat::StaleZeroVersion);
    }

    let max = window.max_supported;
    let min = window.min_supported;

    match projection_version.cmp(&max) {
        std::cmp::Ordering::Equal => Ok(ProjectionCompat::Fresh),
        std::cmp::Ordering::Greater => {
            // Version is too new — outside the window on the upper bound
            Ok(ProjectionCompat::StaleTooOld {
                projection: projection_version,
                window_min: min,
            })
        }
        std::cmp::Ordering::Less => {
            // Version is less than max — could be NeedsUpcast or StaleTooOld
            if projection_version >= min {
                // Within the window but not fresh — needs upcasting
                Ok(ProjectionCompat::NeedsUpcast {
                    from: projection_version,
                    to: max,
                })
            } else {
                // Below the minimum — too old to upcast
                Ok(ProjectionCompat::StaleTooOld {
                    projection: projection_version,
                    window_min: min,
                })
            }
        }
    }
}

/// Returns true when the projection version is within the compatibility window
/// (Fresh or `NeedsUpcast`).
///
/// # Errors
///
/// Returns `false` if window is invalid.
#[must_use]
pub fn is_projection_compatible(
    projection_version: u8,
    window: &ProjectionCompatibilityWindow,
) -> bool {
    check_projection_compat(projection_version, window).is_ok_and(ProjectionCompat::is_compatible)
}

// ============================================================================
// Actions Layer — Fallible I/O Functions
// ============================================================================

/// Validates a raw projection payload bytes against the compatibility window.
///
/// Extracts the `schema_version` from JSON, then calls `check_projection_compat`.
///
/// # Errors
///
/// Returns `ProjectionError::StaleProjection` if stale.
/// Returns `ProjectionError::PayloadDecodeError` variants on parse failure.
/// Returns `ProjectionError::WindowMisconfigured` if window is invalid.
pub fn validate_projection_payload(
    payload_bytes: &[u8],
    window: &ProjectionCompatibilityWindow,
) -> Result<ProjectionCompat, ProjectionError> {
    // Parse JSON to extract schema_version
    let value: serde_json::Value = serde_json::from_slice(payload_bytes)
        .map_err(|_| ProjectionError::BatchDecodeFailed("invalid JSON".to_string()))?;

    let obj = value.as_object().ok_or_else(|| {
        ProjectionError::BatchDecodeFailed("payload is not a JSON object".to_string())
    })?;

    let version_field = obj
        .get("version")
        .ok_or(ProjectionError::MissingSchemaVersion)?;

    let version_u64 = version_field
        .as_u64()
        .ok_or(ProjectionError::InvalidSchemaVersionType)?;
    let version =
        u8::try_from(version_u64).map_err(|_| ProjectionError::InvalidSchemaVersionType)?;

    // Check against max supported version first (before even checking window validity)
    if version > window.max_supported {
        return Err(ProjectionError::SchemaVersionExceedsMax(
            version,
            window.max_supported,
        ));
    }

    check_projection_compat(version, window)
}

/// Validates a batch of projection payloads, returning the first stale detection.
///
/// Short-circuits on first stale projection.
///
/// # Errors
///
/// Returns `ProjectionError::StaleProjection` with the first stale projection's version.
/// Returns `ProjectionError::BatchDecodeFailed` if any payload fails to decode.
/// Returns `ProjectionError::WindowMisconfigured` if window is invalid.
pub fn validate_projection_batch(
    payloads: impl IntoIterator<Item = impl AsRef<[u8]>>,
    window: &ProjectionCompatibilityWindow,
) -> Result<(), ProjectionError> {
    payloads.into_iter().try_for_each(|payload| {
        let result = validate_projection_payload(payload.as_ref(), window)?;
        // Convert stale versions to errors
        match result {
            ProjectionCompat::StaleTooOld {
                projection,
                window_min,
            } => Err(ProjectionError::StaleProjection(
                projection,
                window_min,
                window.max_supported,
            )),
            ProjectionCompat::StaleZeroVersion => Err(ProjectionError::StaleProjection(
                0,
                window.min_supported,
                window.max_supported,
            )),
            ProjectionCompat::Fresh | ProjectionCompat::NeedsUpcast { .. } => Ok(()),
        }
    })?;
    Ok(())
}

// ============================================================================
// Actions Layer — Iterator Guard
// ============================================================================

/// Placeholder for `ProjectionRecord` type from vo-storage.
///
/// In production this would be a real storage record type.
/// We use a type alias here to satisfy the trait bound without adding
/// a dependency on the storage module's internal types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionRecord {
    schema_version: u8,
    payload: Vec<u8>,
}

impl ProjectionRecord {
    #[must_use]
    pub const fn new(schema_version: u8, payload: Vec<u8>) -> Self {
        Self {
            schema_version,
            payload,
        }
    }
}

/// Iterator wrapper that validates each projection against the compatibility window
/// before yielding.
///
/// The wrapped iterator must yield items in increasing sequence order.
/// This guard filters out stale projections by checking their schema version
/// against the window before passing them to the caller.
#[allow(dead_code)]
pub struct CompatibleProjectionIterator<T> {
    inner: T,
    window: ProjectionCompatibilityWindow,
}

impl<T> CompatibleProjectionIterator<T> {
    /// Constructs a guard that filters out stale projections.
    ///
    /// # Errors
    ///
    /// Returns `ProjectionError::WindowMisconfigured` if window is invalid.
    pub fn new(inner: T, window: ProjectionCompatibilityWindow) -> Result<Self, ProjectionError> {
        if !window_is_valid(&window) {
            return Err(ProjectionError::WindowMisconfigured {
                min: window.min_supported,
                max: window.max_supported,
            });
        }
        Ok(Self { inner, window })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // -------------------------------------------------------------------------
    // projection_compat_window tests
    // -------------------------------------------------------------------------

    #[test]
    fn projection_compat_window_returns_ok_when_min_ge_1_and_max_ge_min() {
        let result = projection_compat_window(1, 3);
        assert!(result.is_ok());
        let window = result.unwrap();
        assert!(window_is_valid(&window));
        assert_eq!(window_min_supported(&window), 1);
        assert_eq!(window_max_supported(&window), 3);
    }

    #[test]
    fn projection_compat_window_returns_ok_for_min_equals_max() {
        let result = projection_compat_window(5, 5);
        assert!(result.is_ok());
    }

    #[test]
    fn projection_compat_window_returns_ok_at_minimum_boundary() {
        let result = projection_compat_window(1, 1);
        assert!(result.is_ok());
    }

    #[test]
    fn projection_compat_window_returns_window_misconfigured_when_min_is_zero() {
        let result = projection_compat_window(0, 3);
        assert!(matches!(
            result,
            Err(ProjectionError::WindowMisconfigured { .. })
        ));
    }

    #[test]
    fn projection_compat_window_returns_window_misconfigured_when_max_lt_min() {
        let result = projection_compat_window(5, 3);
        assert!(matches!(
            result,
            Err(ProjectionError::WindowMisconfigured { .. })
        ));
    }

    #[test]
    fn projection_compat_window_returns_window_misconfigured_when_min_gt_max() {
        let result = projection_compat_window(7, 5);
        assert!(matches!(
            result,
            Err(ProjectionError::WindowMisconfigured { .. })
        ));
    }

    // -------------------------------------------------------------------------
    // check_projection_compat tests
    // -------------------------------------------------------------------------

    #[test]
    fn check_projection_compat_returns_fresh_when_version_equals_max() {
        let window = projection_compat_window(2, 5).unwrap();
        let result = check_projection_compat(5, &window);
        assert_eq!(result, Ok(ProjectionCompat::Fresh));
    }

    #[test]
    fn check_projection_compat_returns_needs_upcast_when_version_within_window() {
        let window = projection_compat_window(2, 5).unwrap();
        let result = check_projection_compat(3, &window);
        assert_eq!(result, Ok(ProjectionCompat::NeedsUpcast { from: 3, to: 5 }));
    }

    #[test]
    fn check_projection_compat_returns_needs_upcast_when_version_equals_min() {
        let window = projection_compat_window(2, 5).unwrap();
        let result = check_projection_compat(2, &window);
        assert_eq!(result, Ok(ProjectionCompat::NeedsUpcast { from: 2, to: 5 }));
    }

    #[test]
    fn check_projection_compat_returns_stale_too_old_when_version_below_min() {
        let window = projection_compat_window(3, 7).unwrap();
        let result = check_projection_compat(1, &window);
        assert_eq!(
            result,
            Ok(ProjectionCompat::StaleTooOld {
                projection: 1,
                window_min: 3
            })
        );
    }

    #[test]
    fn check_projection_compat_returns_stale_zero_version_when_version_is_zero() {
        let window = projection_compat_window(1, 5).unwrap();
        let result = check_projection_compat(0, &window);
        assert_eq!(result, Ok(ProjectionCompat::StaleZeroVersion));
    }

    #[test]
    fn check_projection_compat_returns_window_misconfigured_when_window_invalid() {
        let invalid_window = ProjectionCompatibilityWindow {
            min_supported: 0,
            max_supported: 5,
        };
        let result = check_projection_compat(3, &invalid_window);
        assert!(matches!(
            result,
            Err(ProjectionError::WindowMisconfigured { .. })
        ));
    }

    #[test]
    fn check_projection_compat_returns_stale_too_old_when_version_exceeds_max() {
        let window = projection_compat_window(2, 5).unwrap();
        let result = check_projection_compat(10, &window);
        assert_eq!(
            result,
            Ok(ProjectionCompat::StaleTooOld {
                projection: 10,
                window_min: 2
            })
        );
    }

    #[test]
    fn check_projection_compat_is_reflexive_at_max_for_any_valid_window() {
        let window = projection_compat_window(1, 3).unwrap();
        let result = check_projection_compat(window_max_supported(&window), &window);
        assert_eq!(result, Ok(ProjectionCompat::Fresh));
    }

    #[test]
    fn check_projection_compat_returns_fresh_at_u8_max_boundary() {
        let window = projection_compat_window(100, u8::MAX).unwrap();
        let result = check_projection_compat(u8::MAX, &window);
        assert_eq!(result, Ok(ProjectionCompat::Fresh));
    }

    #[test]
    fn check_projection_compat_returns_stale_too_old_when_version_exceeds_max_u8() {
        let window = projection_compat_window(1, 5).unwrap();
        let result = check_projection_compat(u8::MAX, &window);
        assert_eq!(
            result,
            Ok(ProjectionCompat::StaleTooOld {
                projection: u8::MAX,
                window_min: 1
            })
        );
    }

    // -------------------------------------------------------------------------
    // is_projection_compatible tests
    // -------------------------------------------------------------------------

    #[test]
    fn is_projection_compatible_returns_true_for_fresh() {
        let window = projection_compat_window(2, 5).unwrap();
        assert!(is_projection_compatible(5, &window));
    }

    #[test]
    fn is_projection_compatible_returns_true_for_needs_upcast() {
        let window = projection_compat_window(2, 5).unwrap();
        assert!(is_projection_compatible(3, &window));
    }

    #[test]
    fn is_projection_compatible_returns_false_for_stale_too_old() {
        let window = projection_compat_window(3, 7).unwrap();
        assert!(!is_projection_compatible(1, &window));
    }

    #[test]
    fn is_projection_compatible_returns_false_for_stale_zero_version() {
        let window = projection_compat_window(1, 5).unwrap();
        assert!(!is_projection_compatible(0, &window));
    }

    #[test]
    fn is_projection_compatible_returns_false_for_version_exceeding_max() {
        let window = projection_compat_window(2, 5).unwrap();
        assert!(!is_projection_compatible(10, &window));
    }

    #[test]
    fn is_projection_compatible_returns_false_for_invalid_window() {
        let invalid_window = ProjectionCompatibilityWindow {
            min_supported: 0,
            max_supported: 5,
        };
        assert!(!is_projection_compatible(3, &invalid_window));
    }

    // -------------------------------------------------------------------------
    // ProjectionCompat::is_compatible tests
    // -------------------------------------------------------------------------

    #[test]
    fn projection_compat_is_compatible_returns_true_for_fresh() {
        assert!(ProjectionCompat::Fresh.is_compatible());
    }

    #[test]
    fn projection_compat_is_compatible_returns_true_for_needs_upcast() {
        assert!(ProjectionCompat::NeedsUpcast { from: 3, to: 7 }.is_compatible());
    }

    #[test]
    fn projection_compat_is_compatible_returns_false_for_stale_too_old() {
        assert!(!ProjectionCompat::StaleTooOld {
            projection: 1,
            window_min: 3
        }
        .is_compatible());
    }

    #[test]
    fn projection_compat_is_compatible_returns_false_for_stale_zero_version() {
        assert!(!ProjectionCompat::StaleZeroVersion.is_compatible());
    }

    // -------------------------------------------------------------------------
    // validate_projection_payload tests
    // -------------------------------------------------------------------------

    #[test]
    fn validate_projection_payload_returns_fresh_when_version_matches_max() {
        let window = projection_compat_window(2, 5).unwrap();
        let payload = br#"{"version": 5, "data": "foo"}"#;
        let result = validate_projection_payload(payload, &window);
        assert_eq!(result, Ok(ProjectionCompat::Fresh));
    }

    #[test]
    fn validate_projection_payload_returns_needs_upcast_when_version_within_window() {
        let window = projection_compat_window(2, 5).unwrap();
        let payload = br#"{"version": 3, "data": "bar"}"#;
        let result = validate_projection_payload(payload, &window);
        assert_eq!(result, Ok(ProjectionCompat::NeedsUpcast { from: 3, to: 5 }));
    }

    #[test]
    fn validate_projection_payload_returns_stale_too_old_when_version_below_window() {
        let window = projection_compat_window(3, 7).unwrap();
        let payload = br#"{"version": 1, "data": "old"}"#;
        let result = validate_projection_payload(payload, &window);
        assert_eq!(
            result,
            Ok(ProjectionCompat::StaleTooOld {
                projection: 1,
                window_min: 3
            })
        );
    }

    #[test]
    fn validate_projection_payload_returns_stale_zero_version_when_version_is_zero() {
        let window = projection_compat_window(1, 5).unwrap();
        let payload = br#"{"version": 0, "data": "invalid"}"#;
        let result = validate_projection_payload(payload, &window);
        assert_eq!(result, Ok(ProjectionCompat::StaleZeroVersion));
    }

    #[test]
    fn validate_projection_payload_returns_missing_schema_version_when_no_version_field() {
        let window = projection_compat_window(1, 5).unwrap();
        let payload = br#"{"data": "no_version"}"#;
        let result = validate_projection_payload(payload, &window);
        assert!(matches!(result, Err(ProjectionError::MissingSchemaVersion)));
    }

    #[test]
    fn validate_projection_payload_returns_invalid_schema_version_type_when_version_is_string() {
        let window = projection_compat_window(1, 5).unwrap();
        let payload = br#"{"version": "5"}"#;
        let result = validate_projection_payload(payload, &window);
        assert!(matches!(
            result,
            Err(ProjectionError::InvalidSchemaVersionType)
        ));
    }

    #[test]
    fn validate_projection_payload_returns_invalid_schema_version_type_when_version_is_null() {
        let window = projection_compat_window(1, 5).unwrap();
        let payload = br#"{"version": null}"#;
        let result = validate_projection_payload(payload, &window);
        assert!(matches!(
            result,
            Err(ProjectionError::InvalidSchemaVersionType)
        ));
    }

    #[test]
    fn validate_projection_payload_returns_schema_version_exceeds_max_when_version_too_new() {
        let window = projection_compat_window(1, 5).unwrap();
        let payload = br#"{"version": 100}"#;
        let result = validate_projection_payload(payload, &window);
        assert!(matches!(
            result,
            Err(ProjectionError::SchemaVersionExceedsMax(100, 5))
        ));
    }

    // -------------------------------------------------------------------------
    // validate_projection_batch tests
    // -------------------------------------------------------------------------

    #[test]
    fn validate_projection_batch_returns_ok_for_empty_iterator() {
        let window = projection_compat_window(1, 5).unwrap();
        let payloads: Vec<&[u8]> = vec![];
        let result = validate_projection_batch(payloads, &window);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_projection_batch_returns_ok_when_all_payloads_compatible() {
        let window = projection_compat_window(2, 5).unwrap();
        let payloads = vec![
            br#"{"version": 5}"#,
            br#"{"version": 3}"#,
            br#"{"version": 2}"#,
        ];
        let result = validate_projection_batch(payloads, &window);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_projection_batch_returns_stale_projection_at_first_stale() {
        let window = projection_compat_window(2, 5).unwrap();
        let payloads = vec![
            br#"{"version": 5}"#,
            br#"{"version": 1}"#,
            br#"{"version": 3}"#,
        ];
        let result = validate_projection_batch(payloads, &window);
        assert!(matches!(
            result,
            Err(ProjectionError::StaleProjection(1, 2, 5))
        ));
    }

    #[test]
    fn validate_projection_batch_returns_missing_schema_version_from_first_invalid_payload() {
        let window = projection_compat_window(1, 5).unwrap();
        let p1 = br#"{"version": 5}"#;
        let p2 = br#"{"data": "no_version"}"#;
        let p3 = br#"{"version": 3}"#;
        let payloads = vec![p1.as_slice(), p2.as_slice(), p3.as_slice()];
        let result = validate_projection_batch(payloads, &window);
        assert!(matches!(result, Err(ProjectionError::MissingSchemaVersion)));
    }

    #[test]
    fn validate_projection_batch_short_circuits_on_first_error() {
        let window = projection_compat_window(2, 5).unwrap();
        let payloads = vec![
            br#"{"version": 5}"#,
            br#"{"version": 1}"#,
            br#"{"version": 0}"#,
        ];
        let result = validate_projection_batch(payloads, &window);
        // Should return first stale (version 1), not StaleZeroVersion
        assert!(matches!(
            result,
            Err(ProjectionError::StaleProjection(1, 2, 5))
        ));
    }

    #[test]
    fn validate_projection_batch_returns_window_misconfigured_for_invalid_window() {
        let invalid_window = ProjectionCompatibilityWindow {
            min_supported: 0,
            max_supported: 5,
        };
        let payloads = vec![br#"{"version": 3}"#];
        let result = validate_projection_batch(payloads, &invalid_window);
        assert!(matches!(
            result,
            Err(ProjectionError::WindowMisconfigured { .. })
        ));
    }

    // -------------------------------------------------------------------------
    // CompatibleProjectionIterator tests
    // -------------------------------------------------------------------------

    #[test]
    fn compatible_projection_iterator_constructs_with_valid_window() {
        let window = projection_compat_window(2, 5).unwrap();
        let inner = std::iter::empty::<Result<ProjectionRecord, &'static str>>();
        let result = CompatibleProjectionIterator::new(inner, window);
        assert!(result.is_ok());
    }

    #[test]
    fn compatible_projection_iterator_returns_window_misconfigured_for_invalid_window() {
        let invalid_window = ProjectionCompatibilityWindow {
            min_supported: 0,
            max_supported: 5,
        };
        let inner = std::iter::empty::<Result<ProjectionRecord, &'static str>>();
        let result = CompatibleProjectionIterator::new(inner, invalid_window);
        assert!(matches!(
            result,
            Err(ProjectionError::WindowMisconfigured { .. })
        ));
    }

    #[test]
    fn compatible_projection_iterator_wraps_any_iterator_type() {
        let window = projection_compat_window(1, 5).unwrap();
        let inner: Vec<Result<ProjectionRecord, &'static str>> =
            vec![Ok(ProjectionRecord::new(5, vec![]))];
        let result = CompatibleProjectionIterator::new(inner.into_iter(), window);
        assert!(result.is_ok());
    }

    #[test]
    fn compatible_projection_iterator_is_send_and_sync() {
        let window = projection_compat_window(1, 5).unwrap();
        let inner: Vec<Result<ProjectionRecord, &'static str>> = vec![];
        let iterator = CompatibleProjectionIterator::new(inner.into_iter(), window).unwrap();
        fn assert_send_sync<T: Send + Sync>(_: &T) {}
        assert_send_sync(&iterator);
    }

    // -------------------------------------------------------------------------
    // Invariant tests
    // -------------------------------------------------------------------------

    #[test]
    fn invariant_zero_is_always_stale() {
        let windows = [
            projection_compat_window(1, 1).unwrap(),
            projection_compat_window(1, 5).unwrap(),
            projection_compat_window(100, u8::MAX).unwrap(),
        ];
        for window in windows {
            let result = check_projection_compat(0, &window);
            assert_eq!(result, Ok(ProjectionCompat::StaleZeroVersion));
        }
    }

    #[test]
    fn invariant_fresh_is_always_compatible() {
        let windows = [
            projection_compat_window(1, 1).unwrap(),
            projection_compat_window(2, 5).unwrap(),
            projection_compat_window(100, u8::MAX).unwrap(),
        ];
        for window in windows {
            let result = check_projection_compat(window_max_supported(&window), &window);
            assert!(result.is_ok());
            assert!(result.unwrap().is_compatible());
        }
    }

    // -------------------------------------------------------------------------
    // proptest invariants
    // -------------------------------------------------------------------------

    proptest! {
        #[test]
        fn proptest_check_projection_compat_partition_exhaustive(
            window_min in 1u8..=u8::MAX,
            window_max in 1u8..=u8::MAX,
            version in 0u8..=u8::MAX
        ) {
            prop_assume!(window_max >= window_min);
            let window = projection_compat_window(window_min, window_max).unwrap();
            let result = check_projection_compat(version, &window);
            prop_assert!(result.is_ok());

            let compat = result.unwrap();

            // Exactly one variant should match
            let is_fresh = matches!(compat, ProjectionCompat::Fresh);
            let is_needs_upcast = matches!(compat, ProjectionCompat::NeedsUpcast { .. });
            let is_stale_too_old = matches!(compat, ProjectionCompat::StaleTooOld { .. });
            let is_stale_zero = matches!(compat, ProjectionCompat::StaleZeroVersion);

            let variant_count = is_fresh as u8 + is_needs_upcast as u8
                + is_stale_too_old as u8 + is_stale_zero as u8;

            prop_assert_eq!(variant_count, 1);
        }

        #[test]
        fn proptest_window_constructor_validates_preconditions(
            window_min in 0u8..=u8::MAX,
            window_max in 0u8..=u8::MAX
        ) {
            let result = projection_compat_window(window_min, window_max);
            if window_min >= 1 && window_max >= window_min {
                prop_assert!(result.is_ok());
                let window = result.unwrap();
                prop_assert!(window_is_valid(&window));
            } else {
                prop_assert!(result.is_err());
            }
        }

        #[test]
        fn proptest_is_projection_compatible_matches_check_projection_compat(
            window_min in 1u8..=u8::MAX,
            window_max in 1u8..=u8::MAX,
            version in 0u8..=u8::MAX
        ) {
            prop_assume!(window_max >= window_min);
            let window = projection_compat_window(window_min, window_max).unwrap();
            let compat_result = check_projection_compat(version, &window);
            let is_compat = is_projection_compatible(version, &window);

            prop_assert_eq!(compat_result.map(|c| c.is_compatible()).unwrap_or(false), is_compat);
        }

        #[test]
        fn proptest_needs_upcast_from_to_consistency(
            window_min in 1u8..=254u8,
            window_max in 1u8..=u8::MAX,
            version in 0u8..=u8::MAX
        ) {
            prop_assume!(window_max >= window_min);
            let window = projection_compat_window(window_min, window_max).unwrap();
            let result = check_projection_compat(version, &window);

            if let Ok(ProjectionCompat::NeedsUpcast { from, to }) = result {
                prop_assert!(from < to);
                prop_assert_eq!(to, window_max);
            }
        }
    }
}
