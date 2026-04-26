//! Managed effect types for exact-once side effects (ADR-030).
//!
//! Architecture: Data (EffectIntent, EffectKind, EffectRecord, CompensationPolicy)
//!             → Calc (apply_effect_transition, is_terminal, all_variants).
//!
//! This module defines the type system for managed effects flowing through the Engine.
//! No I/O, no engine integration — pure types and state machine logic.

pub mod types;
pub mod transitions;
pub mod receipt;

#[cfg(test)]
mod tests;
#[cfg(feature = "proptest")]
mod proptests;
#[cfg(kani)]
mod verification;

pub use types::{
    CompensationPolicy, EffectIntent, EffectKind, EffectRecord, EffectTransitionError,
    EffectTransitionEvent, ExternalReceipt,
};
pub use transitions::apply_effect_transition;
pub use receipt::Receipt;
