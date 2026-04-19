//! Resource quota enforcement for CPU, memory, and disk limits.
//!
//! Implements per-namespace quotas with overcommit policies per ADR-033.

use serde::{Deserialize, Serialize};
use std::num::NonZeroU64;
use thiserror::Error;

pub mod enforcer;
pub mod policy;

#[cfg(test)]
mod proptests;

#[cfg(test)]
mod integration_tests;

#[cfg(test)]
mod tenant_isolation_tests;

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

    pub fn add_cpu(&mut self, cores: u64) {
        self.cpu_cores_used = self.cpu_cores_used.saturating_add(cores);
    }

    pub fn add_memory(&mut self, bytes: u64) {
        self.memory_bytes_used = self.memory_bytes_used.saturating_add(bytes);
    }

    pub fn add_disk(&mut self, bytes: u64) {
        self.disk_bytes_used = self.disk_bytes_used.saturating_add(bytes);
    }

    pub fn release_cpu(&mut self, cores: u64) -> u64 {
        let actual = cores.min(self.cpu_cores_used);
        self.cpu_cores_used -= actual;
        actual
    }

    pub fn release_memory(&mut self, bytes: u64) -> u64 {
        let actual = bytes.min(self.memory_bytes_used);
        self.memory_bytes_used -= actual;
        actual
    }

    pub fn release_disk(&mut self, bytes: u64) -> u64 {
        let actual = bytes.min(self.disk_bytes_used);
        self.disk_bytes_used -= actual;
        actual
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
    fn b001_resource_kind_has_exactly_three_variants() {
        fn _exhaustiveness(k: ResourceKind) -> &'static str {
            match k {
                ResourceKind::Cpu => "cpu",
                ResourceKind::Memory => "memory",
                ResourceKind::Disk => "disk",
            }
        }
        assert_eq!(_exhaustiveness(ResourceKind::Cpu), "cpu");
        assert_eq!(_exhaustiveness(ResourceKind::Memory), "memory");
        assert_eq!(_exhaustiveness(ResourceKind::Disk), "disk");
        let all: [ResourceKind; 3] = [ResourceKind::Cpu, ResourceKind::Memory, ResourceKind::Disk];
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn b002_resource_kind_as_str_returns_correct_strings() {
        assert_eq!(ResourceKind::Cpu.as_str(), "cpu");
        assert_eq!(ResourceKind::Memory.as_str(), "memory");
        assert_eq!(ResourceKind::Disk.as_str(), "disk");
    }

    #[test]
    fn b003_resource_kind_display_formats_as_lowercase() {
        assert_eq!(format!("{}", ResourceKind::Cpu), "cpu");
        assert_eq!(format!("{}", ResourceKind::Memory), "memory");
        assert_eq!(format!("{}", ResourceKind::Disk), "disk");
    }

    #[test]
    fn b004_cpu_quota_new_constructs_with_max_cores() {
        let quota = CpuQuota::new(NonZeroU64::new(4).unwrap());
        assert_eq!(quota.max_cores.get(), 4);
    }

    #[test]
    fn b005_memory_quota_new_constructs_with_max_bytes() {
        let quota = MemoryQuota::new(NonZeroU64::new(1024).unwrap());
        assert_eq!(quota.max_bytes.get(), 1024);
    }

    #[test]
    fn b006_disk_quota_new_constructs_with_max_bytes() {
        let quota = DiskQuota::new(NonZeroU64::new(10_000_000).unwrap());
        assert_eq!(quota.max_bytes.get(), 10_000_000);
    }

    #[test]
    fn b007_cpu_quota_implements_clone_copy_partial_eq_eq_hash() {
        let q = CpuQuota::new(NonZeroU64::new(4).unwrap());
        let q2 = q;
        assert_eq!(q, q2);
        let q3 = q.clone();
        assert_eq!(q, q3);
        let mut h1 = std::collections::HashSet::new();
        h1.insert(q);
        assert!(h1.contains(&q2));
    }

    #[test]
    fn b008_memory_quota_implements_clone_copy_partial_eq_eq_hash() {
        let q = MemoryQuota::new(NonZeroU64::new(1024).unwrap());
        let q2 = q;
        assert_eq!(q, q2);
        let q3 = q.clone();
        assert_eq!(q, q3);
        let mut h1 = std::collections::HashSet::new();
        h1.insert(q);
        assert!(h1.contains(&q2));
    }

    #[test]
    fn b009_disk_quota_implements_clone_copy_partial_eq_eq_hash() {
        let q = DiskQuota::new(NonZeroU64::new(999).unwrap());
        let q2 = q;
        assert_eq!(q, q2);
        let q3 = q.clone();
        assert_eq!(q, q3);
        let mut h1 = std::collections::HashSet::new();
        h1.insert(q);
        assert!(h1.contains(&q2));
    }

    #[test]
    fn b014_namespace_quota_new_constructs_with_namespace_and_all_none_quotas() {
        let quota = NamespaceQuota::new("payments");
        assert_eq!(quota.namespace, "payments");
        assert!(quota.cpu.is_none());
        assert!(quota.memory.is_none());
        assert!(quota.disk.is_none());
        assert_eq!(quota.overcommit, OvercommitPolicy::default());
    }

    #[test]
    fn b015_namespace_quota_with_cpu_sets_cpu() {
        let quota =
            NamespaceQuota::new("payments").with_cpu(CpuQuota::new(NonZeroU64::new(4).unwrap()));
        assert!(quota.cpu.is_some());
        assert!(quota.memory.is_none());
        assert!(quota.disk.is_none());
        assert_eq!(quota.cpu.unwrap().max_cores.get(), 4);
    }

    #[test]
    fn b016_namespace_quota_with_memory_sets_memory() {
        let quota = NamespaceQuota::new("payments")
            .with_memory(MemoryQuota::new(NonZeroU64::new(1024).unwrap()));
        assert!(quota.memory.is_some());
        assert!(quota.cpu.is_none());
        assert!(quota.disk.is_none());
        assert_eq!(quota.memory.unwrap().max_bytes.get(), 1024);
    }

    #[test]
    fn b017_namespace_quota_with_disk_sets_disk() {
        let quota = NamespaceQuota::new("payments")
            .with_disk(DiskQuota::new(NonZeroU64::new(10_000_000).unwrap()));
        assert!(quota.disk.is_some());
        assert!(quota.cpu.is_none());
        assert!(quota.memory.is_none());
        assert_eq!(quota.disk.unwrap().max_bytes.get(), 10_000_000);
    }

    #[test]
    fn b018_namespace_quota_with_overcommit_sets_policy() {
        let quota =
            NamespaceQuota::new("payments").with_overcommit(OvercommitPolicy::AllowOvercommit);
        assert_eq!(quota.overcommit, OvercommitPolicy::AllowOvercommit);
    }

    #[test]
    fn b019_namespace_quota_implements_clone_partial_eq_serialize_deserialize() {
        let q1 = NamespaceQuota::new("ns1")
            .with_cpu(CpuQuota::new(NonZeroU64::new(2).unwrap()))
            .with_overcommit(OvercommitPolicy::AllowOvercommit);
        let q2 = q1.clone();
        assert_eq!(q1, q2);
        let json = serde_json::to_string(&q1).unwrap();
        let q3: NamespaceQuota = serde_json::from_str(&json).unwrap();
        assert_eq!(q1, q3);
    }

    #[test]
    fn b020_quota_usage_new_creates_with_all_zeros() {
        let usage = QuotaUsage::new();
        assert_eq!(usage.cpu_cores_used, 0);
        assert_eq!(usage.memory_bytes_used, 0);
        assert_eq!(usage.disk_bytes_used, 0);
    }

    #[test]
    fn b021_quota_usage_with_cpu_sets_cpu_cores() {
        let usage = QuotaUsage::new().with_cpu(8);
        assert_eq!(usage.cpu_cores_used, 8);
        assert_eq!(usage.memory_bytes_used, 0);
        assert_eq!(usage.disk_bytes_used, 0);
    }

    #[test]
    fn b022_quota_usage_with_memory_sets_memory_bytes() {
        let usage = QuotaUsage::new().with_memory(2048);
        assert_eq!(usage.memory_bytes_used, 2048);
        assert_eq!(usage.cpu_cores_used, 0);
        assert_eq!(usage.disk_bytes_used, 0);
    }

    #[test]
    fn b023_quota_usage_with_disk_sets_disk_bytes() {
        let usage = QuotaUsage::new().with_disk(4096);
        assert_eq!(usage.disk_bytes_used, 4096);
        assert_eq!(usage.cpu_cores_used, 0);
        assert_eq!(usage.memory_bytes_used, 0);
    }

    #[test]
    fn b024_quota_usage_implements_default_clone_copy_partial_eq_eq() {
        let u1 = QuotaUsage::new().with_cpu(2).with_memory(100);
        let u2 = u1;
        assert_eq!(u1, u2);
        let u3 = u1.clone();
        assert_eq!(u1, u3);
        let u4 = QuotaUsage::default();
        assert_eq!(u4.cpu_cores_used, 0);
        assert_eq!(u4, QuotaUsage::new());
    }

    #[test]
    fn b053_quota_error_is_overcommit_rejected_for_quota_exceeded() {
        let err = QuotaError::QuotaExceeded {
            resource: ResourceKind::Cpu,
            namespace: "payments".to_string(),
            requested: 10,
            available: 4,
        };
        assert!(err.is_overcommit_rejected());
    }

    #[test]
    fn b054_quota_error_is_overcommit_rejected_for_quota_not_configured() {
        let err = QuotaError::QuotaNotConfigured {
            resource: ResourceKind::Memory,
            namespace: "payments".to_string(),
        };
        assert!(err.is_overcommit_rejected());
    }

    #[test]
    fn b055_quota_error_is_not_overcommit_rejected_for_namespace_not_found() {
        let err = QuotaError::NamespaceNotFound("unknown".to_string());
        assert!(!err.is_overcommit_rejected());
    }

    #[test]
    fn b056_quota_exceeded_error_display_includes_resource_namespace_requested_available() {
        let err = QuotaError::QuotaExceeded {
            resource: ResourceKind::Memory,
            namespace: "payments".to_string(),
            requested: 2048,
            available: 1024,
        };
        let display = err.to_string();
        assert!(
            display.contains("memory"),
            "display should contain resource"
        );
        assert!(
            display.contains("payments"),
            "display should contain namespace"
        );
        assert!(display.contains("2048"), "display should contain requested");
        assert!(display.contains("1024"), "display should contain available");
    }

    #[test]
    fn b057_namespace_not_found_error_display_includes_namespace() {
        let err = QuotaError::NamespaceNotFound("my-ns".to_string());
        let display = err.to_string();
        assert!(
            display.contains("my-ns"),
            "display should contain namespace name"
        );
    }

    #[test]
    fn b058_quota_not_configured_error_display_includes_resource_and_namespace() {
        let err = QuotaError::QuotaNotConfigured {
            resource: ResourceKind::Disk,
            namespace: "analytics".to_string(),
        };
        let display = err.to_string();
        assert!(display.contains("disk"), "display should contain resource");
        assert!(
            display.contains("analytics"),
            "display should contain namespace"
        );
    }

    #[test]
    fn overflow_cpu_quota_at_u64_max_request_zero_returns_ok() {
        let enforcer = crate::resource_quota::QuotaEnforcer::with_default_namespace();
        let result = enforcer.check_cpu("default", 0);
        assert!(result.is_ok());
    }

    #[test]
    fn overflow_add_cpu_saturates_at_max() {
        let mut usage = QuotaUsage::new().with_cpu(u64::MAX - 1);
        usage.add_cpu(5);
        assert_eq!(usage.cpu_cores_used, u64::MAX);
    }

    #[test]
    fn overflow_add_cpu_at_max_stays_at_max() {
        let mut usage = QuotaUsage::new().with_cpu(u64::MAX);
        usage.add_cpu(1);
        assert_eq!(usage.cpu_cores_used, u64::MAX);
    }

    #[test]
    fn overflow_add_memory_saturates_at_max() {
        let mut usage = QuotaUsage::new().with_memory(u64::MAX - 100);
        usage.add_memory(200);
        assert_eq!(usage.memory_bytes_used, u64::MAX);
    }

    #[test]
    fn overflow_add_memory_at_max_stays_at_max() {
        let mut usage = QuotaUsage::new().with_memory(u64::MAX);
        usage.add_memory(u64::MAX);
        assert_eq!(usage.memory_bytes_used, u64::MAX);
    }

    #[test]
    fn overflow_add_disk_saturates_at_max() {
        let mut usage = QuotaUsage::new().with_disk(u64::MAX - 1);
        usage.add_disk(2);
        assert_eq!(usage.disk_bytes_used, u64::MAX);
    }

    #[test]
    fn overflow_add_disk_at_max_stays_at_max() {
        let mut usage = QuotaUsage::new().with_disk(u64::MAX);
        usage.add_disk(1);
        assert_eq!(usage.disk_bytes_used, u64::MAX);
    }

    #[test]
    fn overflow_add_zero_does_not_change_counters() {
        let mut usage = QuotaUsage::new().with_cpu(100).with_memory(200).with_disk(300);
        usage.add_cpu(0);
        usage.add_memory(0);
        usage.add_disk(0);
        assert_eq!(usage.cpu_cores_used, 100);
        assert_eq!(usage.memory_bytes_used, 200);
        assert_eq!(usage.disk_bytes_used, 300);
    }

    #[test]
    fn overflow_release_cpu_clamps_to_zero() {
        let mut usage = QuotaUsage::new().with_cpu(10);
        let released = usage.release_cpu(100);
        assert_eq!(released, 10);
        assert_eq!(usage.cpu_cores_used, 0);
    }

    #[test]
    fn overflow_release_memory_clamps_to_zero() {
        let mut usage = QuotaUsage::new().with_memory(50);
        let released = usage.release_memory(u64::MAX);
        assert_eq!(released, 50);
        assert_eq!(usage.memory_bytes_used, 0);
    }

    #[test]
    fn overflow_release_disk_clamps_to_zero() {
        let mut usage = QuotaUsage::new().with_disk(1);
        let released = usage.release_disk(u64::MAX);
        assert_eq!(released, 1);
        assert_eq!(usage.disk_bytes_used, 0);
    }

    #[test]
    fn overflow_release_zero_returns_zero() {
        let mut usage = QuotaUsage::new().with_cpu(100).with_memory(200).with_disk(300);
        assert_eq!(usage.release_cpu(0), 0);
        assert_eq!(usage.release_memory(0), 0);
        assert_eq!(usage.release_disk(0), 0);
        assert_eq!(usage.cpu_cores_used, 100);
        assert_eq!(usage.memory_bytes_used, 200);
        assert_eq!(usage.disk_bytes_used, 300);
    }

    #[test]
    fn overflow_saturate_then_release_roundtrip() {
        let mut usage = QuotaUsage::new();
        usage.add_cpu(u64::MAX);
        usage.add_memory(u64::MAX);
        usage.add_disk(u64::MAX);
        assert_eq!(usage.cpu_cores_used, u64::MAX);
        assert_eq!(usage.memory_bytes_used, u64::MAX);
        assert_eq!(usage.disk_bytes_used, u64::MAX);

        let r_cpu = usage.release_cpu(1);
        let r_mem = usage.release_memory(1);
        let r_disk = usage.release_disk(1);
        assert_eq!(r_cpu, 1);
        assert_eq!(r_mem, 1);
        assert_eq!(r_disk, 1);
        assert_eq!(usage.cpu_cores_used, u64::MAX - 1);
        assert_eq!(usage.memory_bytes_used, u64::MAX - 1);
        assert_eq!(usage.disk_bytes_used, u64::MAX - 1);
    }

    #[test]
    fn overflow_multiple_accumulations_saturate_correctly() {
        let mut usage = QuotaUsage::new();
        for _ in 0..=u8::MAX {
            usage.add_cpu(1 << 56);
            usage.add_memory(1 << 56);
            usage.add_disk(1 << 56);
        }
        assert_eq!(usage.cpu_cores_used, u64::MAX);
        assert_eq!(usage.memory_bytes_used, u64::MAX);
        assert_eq!(usage.disk_bytes_used, u64::MAX);
    }

    #[test]
    fn edge_u64_max_requested_no_overcommit_returns_quota_exceeded() {
        let enforcer = crate::resource_quota::QuotaEnforcer::with_default_namespace();
        let result = enforcer.check_memory("default", u64::MAX);
        assert!(result.is_err());
    }

    #[test]
    fn edge_empty_namespace_string_is_allowed() {
        let mut registry = crate::resource_quota::NamespaceRegistry::new();
        let quota = NamespaceQuota::new("").with_cpu(CpuQuota::new(NonZeroU64::new(1).unwrap()));
        let result = registry.register(quota);
        assert!(result.is_ok());
        assert!(registry.get("").is_some());
    }

    #[test]
    fn edge_special_characters_in_namespace() {
        let mut registry = crate::resource_quota::NamespaceRegistry::new();
        let quota = NamespaceQuota::new("ns/with-special.chars_123")
            .with_cpu(CpuQuota::new(NonZeroU64::new(1).unwrap()));
        let result = registry.register(quota);
        assert!(result.is_ok());
        assert!(registry.get("ns/with-special.chars_123").is_some());
    }

    #[test]
    fn resource_kind_serializes_to_snake_case() {
        let json = serde_json::to_string(&ResourceKind::Cpu).unwrap();
        assert_eq!(json, "\"cpu\"");
        let json = serde_json::to_string(&ResourceKind::Memory).unwrap();
        assert_eq!(json, "\"memory\"");
        let json = serde_json::to_string(&ResourceKind::Disk).unwrap();
        assert_eq!(json, "\"disk\"");
    }

    #[test]
    fn resource_kind_deserializes_from_snake_case() {
        let cpu: ResourceKind = serde_json::from_str("\"cpu\"").unwrap();
        assert_eq!(cpu, ResourceKind::Cpu);
        let mem: ResourceKind = serde_json::from_str("\"memory\"").unwrap();
        assert_eq!(mem, ResourceKind::Memory);
        let disk: ResourceKind = serde_json::from_str("\"disk\"").unwrap();
        assert_eq!(disk, ResourceKind::Disk);
    }

    #[test]
    fn cpu_quota_serializes_and_deserializes() {
        let q = CpuQuota::new(NonZeroU64::new(8).unwrap());
        let json = serde_json::to_string(&q).unwrap();
        let q2: CpuQuota = serde_json::from_str(&json).unwrap();
        assert_eq!(q, q2);
    }

    #[test]
    fn memory_quota_serializes_and_deserializes() {
        let q = MemoryQuota::new(NonZeroU64::new(4096).unwrap());
        let json = serde_json::to_string(&q).unwrap();
        let q2: MemoryQuota = serde_json::from_str(&json).unwrap();
        assert_eq!(q, q2);
    }

    #[test]
    fn disk_quota_serializes_and_deserializes() {
        let q = DiskQuota::new(NonZeroU64::new(9999).unwrap());
        let json = serde_json::to_string(&q).unwrap();
        let q2: DiskQuota = serde_json::from_str(&json).unwrap();
        assert_eq!(q, q2);
    }
}
