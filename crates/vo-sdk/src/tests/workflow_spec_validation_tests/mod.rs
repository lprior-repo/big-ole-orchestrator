//! Test Coverage: WorkflowSpec validation and discovery (ADR-003/017/022/031).
//!
//! bead_id: ve-jm7n
//!
//! Coverage areas:
//!   1. Valid specs accepted — complete workflow specs round-trip correctly
//!   2. Invalid node mixes rejected — node kind semantic constraint enforcement
//!   3. Version pinning enforced — schema version validation at both type layers
//!   4. Discovery validation — version compatibility, schema evolution, upgrade paths
//!   5. Node kind constraints — each kind's specific behavioral contracts

mod dag_build_determinism;
mod discovery_validation;
mod invalid_node_mixes;
mod node_kind_constraints;
mod serde_integrity;
mod valid_spec_acceptance;
mod version_pinning;
