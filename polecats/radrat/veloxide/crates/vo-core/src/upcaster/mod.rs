//! Upcaster registry interfaces for schema evolution support.
//!
//! This module provides traits for registering and applying upcast transforms
//! that normalize older schema versions to newer ones.
//!
//! The [`Upcaster`] trait is re-exported from [`vo_types::events::upcaster::Upcaster`]
//! to provide a unified interface for both envelope-level and payload-level upcasting.

mod error;
mod registry;

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
pub use vo_types::events::upcaster::Upcaster;
