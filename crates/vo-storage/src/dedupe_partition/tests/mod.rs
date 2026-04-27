//! Integration tests for `dedupe_partition` (vel-7ffu).
//!
//! This module delegates to focused test submodules:
//! - `tests_entry_construction`: Unit tests for `DedupeEntry` construction and expiry
//! - `tests_encoding`: Unit tests for encode/decode functions
//! - `tests_store_operations`: Integration tests for `check_and_insert` and contains
//! - `tests_purge`: Integration tests for `purge_expired`
//! - `tests_mutation_killers`: Mutation-killer tests

#![allow(clippy::unwrap_used)]

// Re-export parent module items so child modules can access via `use super::*`
pub(super) use super::*;

mod tests_concurrent;
mod tests_encoding;
mod tests_entry_construction;
mod tests_mutation_killers;
mod tests_purge;
mod tests_store_operations;
