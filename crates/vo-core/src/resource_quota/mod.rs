//! Resource quota enforcement for CPU, memory, and disk limits.
//!
//! Implements per-namespace quotas with overcommit policies per ADR-033.
//! Supports soft limit warnings that trigger before hard limits are reached.

pub mod enforcer;
pub mod policy;
pub mod registry;
pub mod types;
pub mod warning_tracker;

#[cfg(test)]
mod enforcer_tests;

#[cfg(test)]
mod soft_limit_tests;

#[cfg(test)]
mod tests;

pub use enforcer::QuotaEnforcer;
pub use policy::OvercommitPolicy;
pub use registry::NamespaceRegistry;
pub use types::{
    CpuQuota, DiskQuota, MemoryQuota, NamespaceQuota, QuotaCheckResult, QuotaError, QuotaUsage,
    ResourceKind, SoftLimitPercent, SoftLimitWarning,
};
pub use warning_tracker::QuotaWarningTracker;
