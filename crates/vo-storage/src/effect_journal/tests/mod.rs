//! Effect journal tests.
//!
//! Test organization:
//! - `tests_effect_id` — `EffectId` construction and error display tests
//! - `tests_codec` — encode/decode function tests
//! - `tests_journal_integration` — `EffectJournal` trait integration tests
//! - `tests_journal_lifecycle` — lifecycle, error handling, and kani tests

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

mod tests_codec;
mod tests_crash_injection;
mod tests_durability;
mod tests_effect_id;
mod tests_journal_durability;
mod tests_journal_integration;
mod tests_journal_lifecycle;
