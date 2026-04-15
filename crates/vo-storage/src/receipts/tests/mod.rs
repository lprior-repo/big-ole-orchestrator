//! Receipt persistence tests.
//!
//! Test organization:
//! - `tests_receipt_construction` — Receipt type construction and validation
//! - `tests_write` — Receipt write persistence tests
//! - `tests_read` — Receipt read/query tests
//! - `tests_duplicate` — Idempotency and duplicate rejection tests
//! - `tests_crash` — Crash recovery and durability tests
//! - `tests_codec` — Key/value encode/decode round-trip tests

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

mod tests_codec;
mod tests_crash;
mod tests_duplicate;
mod tests_read;
mod tests_receipt_construction;
mod tests_write;
