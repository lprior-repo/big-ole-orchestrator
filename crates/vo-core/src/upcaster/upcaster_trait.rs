//! Upcaster trait for transforming schema versions.

use crate::upcaster::UpcasterError;

/// Trait for a single upcaster that transforms a specific schema version to the next version.
pub trait Upcaster: Send + Sync {
    /// The source schema version this upcaster handles
    fn source_version(&self) -> u8;

    /// Apply the upcast transformation to the raw JSON bytes
    fn upcast(&self, input: &[u8]) -> Result<Vec<u8>, UpcasterError>;
}
