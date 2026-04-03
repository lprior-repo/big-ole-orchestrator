//! Upcaster registry interfaces for schema evolution support.
//!
//! This module provides traits for registering and applying upcast transforms
//! that normalize older schema versions to newer ones.

mod error;
mod registry;
mod upcaster_trait;

#[cfg(test)]
mod error_tests;
#[cfg(test)]
mod event_envelope_error_tests;

#[allow(unexpected_cfgs)]
#[cfg(kani)]
mod kani_harnesses;

pub use error::{EventEnvelopeError, UpcasterError, MAX_SUPPORTED_VERSION};
pub use registry::{
    DefaultUpcasterRegistryBuilder, UpcasterRegistry, UpcasterRegistryBuilder, UpcasterRegistryImpl,
};
pub use upcaster_trait::Upcaster;
