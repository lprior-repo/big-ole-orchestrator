//! Managed effect types for exact-once side effects (ADR-030).
//!
//! Architecture: Data (EffectIntent, EffectKind, EffectRecord, CompensationPolicy)
//!             → Calc (apply_effect_transition, is_terminal, all_variants).
//!
//! This module defines the type system for managed effects flowing through the Engine.
//! No I/O, no engine integration — pure types and state machine logic.

pub mod receipt;
pub mod transitions;
pub mod types;

#[cfg(feature = "proptest")]
mod proptests;
#[cfg(test)]
mod tests;
#[cfg(kani)]
mod verification;

pub use receipt::Receipt;
pub use transitions::apply_effect_transition;
pub use types::{
    CompensationPolicy, EffectIntent, EffectKind, EffectRecord, EffectTransitionError,
    EffectTransitionEvent, ExternalReceipt,
};
