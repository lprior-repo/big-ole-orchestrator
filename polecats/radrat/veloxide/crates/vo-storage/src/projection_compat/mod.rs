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

mod actions;
mod calc;
mod types;

#[cfg(test)]
mod tests;

// Re-export everything from the Data layer
pub use types::{ProjectionCompat, ProjectionCompatibilityWindow, ProjectionError};

// Re-export everything from the Calc layer
pub use calc::{
    check_projection_compat, is_projection_compatible, projection_compat_window, window_is_valid,
    window_max_supported, window_min_supported,
};

// Re-export everything from the Actions layer
pub use actions::{
    validate_projection_batch, validate_projection_payload, CompatibleProjectionIterator,
    ProjectionRecord,
};
