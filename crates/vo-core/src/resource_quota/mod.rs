//! Resource quota enforcement for CPU, memory, and disk limits.
//!
//! Implements per-namespace quotas with overcommit policies per ADR-033.

pub mod enforcer;
pub mod policy;
pub mod registry;
pub mod types;

#[cfg(test)]
mod enforcer_tests;

#[cfg(test)]
mod tests;

pub use enforcer::QuotaEnforcer;
pub use policy::OvercommitPolicy;
pub use registry::NamespaceRegistry;
pub use types::{
    CpuQuota, DiskQuota, MemoryQuota, NamespaceQuota, QuotaError, QuotaUsage, ResourceKind,
};
