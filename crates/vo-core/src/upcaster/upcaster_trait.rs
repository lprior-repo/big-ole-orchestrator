//! Upcaster trait for transforming schema versions.

use crate::upcaster::UpcasterError;

/// Trait for a single upcaster that transforms a specific schema version to the next version.
pub trait Upcaster: Send + Sync {
    /// The source schema version this upcaster handles
    fn source_version(&self) -> u8;

    /// The target schema version this upcaster produces.
    ///
    /// Defaults to `source_version() + 1` for single-step upcasters.
    /// Override for multi-version upcasters (e.g., v0 -> v2 skipping v1).
    fn target_version(&self) -> u8 {
        self.source_version() + 1
    }

    /// Apply the upcast transformation to the raw JSON bytes
    fn upcast(&self, input: &[u8]) -> Result<Vec<u8>, UpcasterError>;
}
