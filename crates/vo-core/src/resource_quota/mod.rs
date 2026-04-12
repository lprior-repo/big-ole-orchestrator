//! Resource quota enforcement for CPU, memory, and disk limits.
//!
//! Implements per-namespace quotas with overcommit policies per ADR-033.

use serde::{Deserialize, Serialize};
use std::num::NonZeroU64;
use thiserror::Error;

pub mod enforcer;
pub mod policy;

pub use enforcer::{NamespaceRegistry, QuotaEnforcer};
pub use policy::OvercommitPolicy;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotaUsage {
    pub cpu_cores_used: u64,
    pub memory_bytes_used: u64,
    pub disk_bytes_used: u64,
}

impl QuotaUsage {
    pub fn new() -> Self {
        Self {
            cpu_cores_used: 0,
            memory_bytes_used: 0,
            disk_bytes_used: 0,
        }
    }

    pub fn with_cpu(mut self, cores: u64) -> Self {
        self.cpu_cores_used = cores;
        self
    }

    pub fn with_memory(mut self, bytes: u64) -> Self {
        self.memory_bytes_used = bytes;
        self
    }

    pub fn with_disk(mut self, bytes: u64) -> Self {
        self.disk_bytes_used = bytes;
        self
    }
}

impl Default for QuotaUsage {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    Cpu,
    Memory,
    Disk,
}

impl ResourceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ResourceKind::Cpu => "cpu",
            ResourceKind::Memory => "memory",
            ResourceKind::Disk => "disk",
        }
    }
}

impl std::fmt::Display for ResourceKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CpuQuota {
    pub max_cores: NonZeroU64,
}

impl CpuQuota {
    pub fn new(max_cores: NonZeroU64) -> Self {
        Self { max_cores }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MemoryQuota {
    pub max_bytes: NonZeroU64,
}

impl MemoryQuota {
    pub fn new(max_bytes: NonZeroU64) -> Self {
        Self { max_bytes }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DiskQuota {
    pub max_bytes: NonZeroU64,
}

impl DiskQuota {
    pub fn new(max_bytes: NonZeroU64) -> Self {
        Self { max_bytes }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct NamespaceQuota {
    pub namespace: String,
    pub cpu: Option<CpuQuota>,
    pub memory: Option<MemoryQuota>,
    pub disk: Option<DiskQuota>,
    pub overcommit: OvercommitPolicy,
}

impl NamespaceQuota {
    pub fn new(namespace: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            cpu: None,
            memory: None,
            disk: None,
            overcommit: OvercommitPolicy::default(),
        }
    }

    pub fn with_cpu(mut self, quota: CpuQuota) -> Self {
        self.cpu = Some(quota);
        self
    }

    pub fn with_memory(mut self, quota: MemoryQuota) -> Self {
        self.memory = Some(quota);
        self
    }

    pub fn with_disk(mut self, quota: DiskQuota) -> Self {
        self.disk = Some(quota);
        self
    }

    pub fn with_overcommit(mut self, policy: OvercommitPolicy) -> Self {
        self.overcommit = policy;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum QuotaError {
    #[error("quota exceeded for {resource} in namespace {namespace}: requested {requested}, available {available}")]
    QuotaExceeded {
        resource: ResourceKind,
        namespace: String,
        requested: u64,
        available: u64,
    },

    #[error("namespace {0} not found")]
    NamespaceNotFound(String),

    #[error("quota not configured for {resource} in namespace {namespace}")]
    QuotaNotConfigured {
        resource: ResourceKind,
        namespace: String,
    },
}

impl QuotaError {
    pub const fn is_overcommit_rejected(&self) -> bool {
        matches!(
            self,
            QuotaError::QuotaExceeded { .. } | QuotaError::QuotaNotConfigured { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_kind_as_str_returns_cpu() {
        assert_eq!(ResourceKind::Cpu.as_str(), "cpu");
    }

    #[test]
    fn resource_kind_as_str_returns_memory() {
        assert_eq!(ResourceKind::Memory.as_str(), "memory");
    }

    #[test]
    fn resource_kind_as_str_returns_disk() {
        assert_eq!(ResourceKind::Disk.as_str(), "disk");
    }

    #[test]
    fn cpu_quota_new_constructs_with_max_cores() {
        let quota = CpuQuota::new(NonZeroU64::new(4).unwrap());
        assert_eq!(quota.max_cores.get(), 4);
    }

    #[test]
    fn memory_quota_new_constructs_with_max_bytes() {
        let quota = MemoryQuota::new(NonZeroU64::new(1024).unwrap());
        assert_eq!(quota.max_bytes.get(), 1024);
    }

    #[test]
    fn disk_quota_new_constructs_with_max_bytes() {
        let quota = DiskQuota::new(NonZeroU64::new(10_000_000).unwrap());
        assert_eq!(quota.max_bytes.get(), 10_000_000);
    }

    #[test]
    fn namespace_quota_new_constructs_with_namespace() {
        let quota = NamespaceQuota::new("payments");
        assert_eq!(quota.namespace, "payments");
        assert!(quota.cpu.is_none());
        assert!(quota.memory.is_none());
        assert!(quota.disk.is_none());
        assert_eq!(quota.overcommit, OvercommitPolicy::default());
    }

    #[test]
    fn namespace_quota_with_cpu_sets_cpu() {
        let quota =
            NamespaceQuota::new("payments").with_cpu(CpuQuota::new(NonZeroU64::new(4).unwrap()));
        assert!(quota.cpu.is_some());
        assert_eq!(quota.cpu.unwrap().max_cores.get(), 4);
    }

    #[test]
    fn namespace_quota_with_memory_sets_memory() {
        let quota = NamespaceQuota::new("payments")
            .with_memory(MemoryQuota::new(NonZeroU64::new(1024).unwrap()));
        assert!(quota.memory.is_some());
        assert_eq!(quota.memory.unwrap().max_bytes.get(), 1024);
    }

    #[test]
    fn namespace_quota_with_disk_sets_disk() {
        let quota = NamespaceQuota::new("payments")
            .with_disk(DiskQuota::new(NonZeroU64::new(10_000_000).unwrap()));
        assert!(quota.disk.is_some());
        assert_eq!(quota.disk.unwrap().max_bytes.get(), 10_000_000);
    }

    #[test]
    fn namespace_quota_with_overcommit_sets_policy() {
        let quota =
            NamespaceQuota::new("payments").with_overcommit(OvercommitPolicy::AllowOvercommit);
        assert_eq!(quota.overcommit, OvercommitPolicy::AllowOvercommit);
    }

    #[test]
    fn quota_error_is_overcommit_rejected_for_quota_exceeded() {
        let err = QuotaError::QuotaExceeded {
            resource: ResourceKind::Cpu,
            namespace: "payments".to_string(),
            requested: 10,
            available: 4,
        };
        assert!(err.is_overcommit_rejected());
    }

    #[test]
    fn quota_error_is_overcommit_rejected_for_quota_not_configured() {
        let err = QuotaError::QuotaNotConfigured {
            resource: ResourceKind::Memory,
            namespace: "payments".to_string(),
        };
        assert!(err.is_overcommit_rejected());
    }

    #[test]
    fn quota_error_is_not_overcommit_rejected_for_namespace_not_found() {
        let err = QuotaError::NamespaceNotFound("unknown".to_string());
        assert!(!err.is_overcommit_rejected());
    }

    #[test]
    fn quota_error_display_includes_resource_and_namespace() {
        let err = QuotaError::QuotaExceeded {
            resource: ResourceKind::Memory,
            namespace: "payments".to_string(),
            requested: 2048,
            available: 1024,
        };
        let display = err.to_string();
        assert!(display.contains("memory"));
        assert!(display.contains("payments"));
        assert!(display.contains("2048"));
        assert!(display.contains("1024"));
    }
}
