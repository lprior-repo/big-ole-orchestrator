//! Red Queen adversarial tests for effect_journal.
//!
//! These tests attempt to find bugs by violating contracts and testing edge cases.
//!
//! Organization:
//! - `effect_id` — EffectId construction and shape tests
//! - `codec` — key and record codec tests
//! - `journal` — journal lifecycle and idempotency tests

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

mod codec;
mod effect_id;
mod journal;
