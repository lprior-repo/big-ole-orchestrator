//! Signal matching and wait types per ADR-042.
//!
//! This module defines pure data types for signal routing, wait-state matching,
//! buffer policies, and signal delivery outcomes.

mod buffer_policy;
mod dedupe_key;
mod lineage_scope;
mod signal_address;
mod signal_delivery;
mod signal_match;
mod wait_key;
mod wait_record;

pub use buffer_policy::BufferPolicy;
pub use dedupe_key::SignalDedupeKey;
pub use lineage_scope::LineageScope;
pub use signal_address::SignalAddress;
pub use signal_delivery::SignalDelivery;
pub use signal_match::{signal_match, SignalMatchResult};
pub use wait_key::WaitKey;
pub use wait_record::WaitRecord;

#[cfg(test)]
mod tests;
