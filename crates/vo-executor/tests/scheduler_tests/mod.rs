// TDD Red: Background job scheduler tests
// These tests define the expected behavior per ADR-047-v2 contract
// Tests marked FAILING are not yet passing - implementation incomplete

mod retry_policy;
mod unit_types;
mod unit_config;
mod unit_errors;
mod integration;
mod concurrency;
mod priority;
