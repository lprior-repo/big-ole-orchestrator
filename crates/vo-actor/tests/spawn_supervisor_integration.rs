//! Integration tests for spawn supervisor.
//!
//! Decomposed into focused submodules:
//! - spawn_supervisor_unit: unit tests (pure functions, error classification, types)
//! - spawn_supervisor_lifecycle: supervisor lifecycle and process cycle integration tests
//! - spawn_supervisor_backoff: backoff/respawn timing integration tests
//! - spawn_supervisor_validation: supervisor constructor validation tests
//!
//! Shared mocks live in common/mod.rs.
