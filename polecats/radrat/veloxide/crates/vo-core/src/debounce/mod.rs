//! File event debouncer with configurable duration.
//!
//! Provides debouncing of file system events to coalesce multiple rapid
//! modifications into single events.

pub mod debouncer;
#[cfg(test)]
pub mod debouncer_tests;
pub mod types;

pub use debouncer::Debouncer;
pub use types::{Error, FileEvent};
