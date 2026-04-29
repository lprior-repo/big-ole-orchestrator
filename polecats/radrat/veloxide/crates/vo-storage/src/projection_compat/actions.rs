// ============================================================================
// Actions Layer — Fallible I/O Functions
// ============================================================================

use super::calc::check_projection_compat;
use super::types::{ProjectionCompat, ProjectionCompatibilityWindow, ProjectionError};

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
    pub(crate) schema_version: u8,
    pub(crate) payload: Vec<u8>,
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
        use super::calc::window_is_valid;
        if !window_is_valid(&window) {
            return Err(ProjectionError::WindowMisconfigured {
                min: window.min_supported,
                max: window.max_supported,
            });
        }
        Ok(Self { inner, window })
    }
}
