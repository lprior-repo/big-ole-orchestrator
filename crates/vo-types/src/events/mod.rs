//! Domain events for the vo-engine event-sourcing layer.
//!
//! This module defines the event envelope, metadata, payload, and upcasting
//! infrastructure that forms the backbone of veloxide's durable execution:
//!
//! - [`EventEnvelope`] — versioned wrapper carrying event metadata + payload.
//! - [`EventMetadata`] — causal tracing data (correlation, causation, timestamps).
//! - [`EventPayload`] — the typed business event data.
//! - [`Upcaster`] / [`VersionRegistry`] — forward-compatible event migration
//!   machinery for schema evolution.
//! - [`decode_event`] — deserialization entry point from raw bytes.
//!
//! Events are the sole unit of state change in the engine. Every command
//! produces one or more events that are persisted to the event store and
//! applied to workflow state machines via the actor system.
//!
//! # Versioning
//!
//! Current wire version is [`MAX_SUPPORTED_VERSION`] (1). Upcasting allows
//! old events to be transparently migrated to the current schema on read.

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
pub use payload::EventPayload;
pub use upcaster::{Upcaster, UpcasterError, VersionRegistry};
