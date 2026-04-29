//! Domain events for the vo-engine.

pub mod decode;
pub mod envelope;
pub mod error;
pub mod metadata;
pub mod payload;
pub mod upcaster;

#[cfg(test)]
mod tests;

pub const MAX_SUPPORTED_VERSION: u8 = 1;

// Re-export all public types for backward compatibility
pub use decode::decode_event;
pub use envelope::EventEnvelope;
pub use error::Error;
pub use metadata::EventMetadata;
pub use payload::{EventPayload, SinkKind};
pub use upcaster::{Upcaster, UpcasterError, VersionRegistry};
