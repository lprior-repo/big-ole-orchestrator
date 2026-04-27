// ============================================================================
// Calc Layer — Pure Functions
// ============================================================================

use super::types::{ProjectionCompat, ProjectionCompatibilityWindow, ProjectionError};

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
