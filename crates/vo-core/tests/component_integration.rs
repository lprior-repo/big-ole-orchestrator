//! Component integration tests for vo-core.
//!
//! These tests exercise integration between multiple vo-core components to verify
//! that they work correctly when composed together.

use std::sync::Arc;
use std::time::{Duration, Instant};

use vo_core::circuit_breaker::{
    evaluate_registration, record_failure, CircuitBreakerConfig, CircuitBreakerState,
    RegistrationOutcome, RegistrationRequest, RegistrationStatus,
};
use vo_core::resource_quota::{NamespaceQuota, OvercommitPolicy, QuotaEnforcer, QuotaUsage};
use vo_core::write_class::{WriteBudget, WriteClass};

pub mod admission_tests;
pub mod circuit_breaker_tests;
pub mod config_hot_reload_tests;
pub mod resource_quota_tests;
pub mod workload_class_tests;
pub mod workflow_version_tests;
pub mod write_class_tests;