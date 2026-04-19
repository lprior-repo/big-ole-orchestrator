//! Quota enforcer for resource quota checking and enforcement.

use super::{CpuQuota, DiskQuota, MemoryQuota, NamespaceQuota, QuotaError, ResourceKind};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct NamespaceRegistry {
    quotas: HashMap<String, NamespaceQuota>,
}

impl NamespaceRegistry {
    pub fn new() -> Self {
        Self {
            quotas: HashMap::new(),
        }
    }

    pub fn register(&mut self, quota: NamespaceQuota) -> Result<(), QuotaError> {
        let namespace = quota.namespace.clone();
        self.quotas.insert(namespace, quota);
        Ok(())
    }

    pub fn get(&self, namespace: &str) -> Option<&NamespaceQuota> {
        self.quotas.get(namespace)
    }

    pub fn remove(&mut self, namespace: &str) -> Option<NamespaceQuota> {
        self.quotas.remove(namespace)
    }

    pub fn list_namespaces(&self) -> Vec<&str> {
        self.quotas.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for NamespaceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct QuotaEnforcer {
    registry: NamespaceRegistry,
}

impl QuotaEnforcer {
    pub fn new(registry: NamespaceRegistry) -> Self {
        Self { registry }
    }

    pub fn with_default_namespace() -> Self {
        let mut registry = NamespaceRegistry::new();
        let default_ns = NamespaceQuota::new("default")
            .with_cpu(CpuQuota::new(
                std::num::NonZeroU64::new(4).expect("default cpu quota is non-zero"),
            ))
            .with_memory(MemoryQuota::new(
                std::num::NonZeroU64::new(8 * 1024 * 1024 * 1024)
                    .expect("default memory quota is non-zero"),
            ))
            .with_disk(DiskQuota::new(
                std::num::NonZeroU64::new(100 * 1024 * 1024 * 1024)
                    .expect("default disk quota is non-zero"),
            ));
        let _ = registry.register(default_ns);
        Self { registry }
    }

    pub fn check_cpu(&self, namespace: &str, requested_cores: u64) -> Result<(), QuotaError> {
        let quota = self
            .registry
            .get(namespace)
            .ok_or_else(|| QuotaError::NamespaceNotFound(namespace.to_string()))?;

        let cpu_quota = quota
            .cpu
            .as_ref()
            .ok_or_else(|| QuotaError::QuotaNotConfigured {
                resource: ResourceKind::Cpu,
                namespace: namespace.to_string(),
            })?;

        let max_cores = cpu_quota.max_cores.get();
        if requested_cores > max_cores {
            if quota.overcommit.allows_overcommit() {
                return Ok(());
            }
            return Err(QuotaError::QuotaExceeded {
                resource: ResourceKind::Cpu,
                namespace: namespace.to_string(),
                requested: requested_cores,
                available: max_cores,
            });
        }
        Ok(())
    }

    pub fn check_memory(&self, namespace: &str, requested_bytes: u64) -> Result<(), QuotaError> {
        let quota = self
            .registry
            .get(namespace)
            .ok_or_else(|| QuotaError::NamespaceNotFound(namespace.to_string()))?;

        let memory_quota = quota
            .memory
            .as_ref()
            .ok_or_else(|| QuotaError::QuotaNotConfigured {
                resource: ResourceKind::Memory,
                namespace: namespace.to_string(),
            })?;

        let max_bytes = memory_quota.max_bytes.get();
        if requested_bytes > max_bytes {
            if quota.overcommit.allows_overcommit() {
                return Ok(());
            }
            return Err(QuotaError::QuotaExceeded {
                resource: ResourceKind::Memory,
                namespace: namespace.to_string(),
                requested: requested_bytes,
                available: max_bytes,
            });
        }
        Ok(())
    }

    pub fn check_disk(&self, namespace: &str, requested_bytes: u64) -> Result<(), QuotaError> {
        let quota = self
            .registry
            .get(namespace)
            .ok_or_else(|| QuotaError::NamespaceNotFound(namespace.to_string()))?;

        let disk_quota = quota
            .disk
            .as_ref()
            .ok_or_else(|| QuotaError::QuotaNotConfigured {
                resource: ResourceKind::Disk,
                namespace: namespace.to_string(),
            })?;

        let max_bytes = disk_quota.max_bytes.get();
        if requested_bytes > max_bytes {
            if quota.overcommit.allows_overcommit() {
                return Ok(());
            }
            return Err(QuotaError::QuotaExceeded {
                resource: ResourceKind::Disk,
                namespace: namespace.to_string(),
                requested: requested_bytes,
                available: max_bytes,
            });
        }
        Ok(())
    }

    pub fn registry(&self) -> &NamespaceRegistry {
        &self.registry
    }

    pub fn registry_mut(&mut self) -> &mut NamespaceRegistry {
        &mut self.registry
    }
}

impl Default for QuotaEnforcer {
    fn default() -> Self {
        Self::with_default_namespace()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource_quota::{OvercommitPolicy, QuotaUsage};
    use std::num::NonZeroU64;

    fn make_test_enforcer() -> QuotaEnforcer {
        let mut registry = NamespaceRegistry::new();
        let _ = registry.register(
            NamespaceQuota::new("payments")
                .with_cpu(CpuQuota::new(NonZeroU64::new(4).unwrap()))
                .with_memory(MemoryQuota::new(NonZeroU64::new(1024).unwrap()))
                .with_disk(DiskQuota::new(NonZeroU64::new(10_000).unwrap()))
                .with_overcommit(OvercommitPolicy::NoOvercommit),
        );
        QuotaEnforcer::new(registry)
    }

    fn make_overcommit_enforcer() -> QuotaEnforcer {
        let mut registry = NamespaceRegistry::new();
        let _ = registry.register(
            NamespaceQuota::new("payments")
                .with_cpu(CpuQuota::new(NonZeroU64::new(4).unwrap()))
                .with_memory(MemoryQuota::new(NonZeroU64::new(1024).unwrap()))
                .with_disk(DiskQuota::new(NonZeroU64::new(10_000).unwrap()))
                .with_overcommit(OvercommitPolicy::AllowOvercommit),
        );
        QuotaEnforcer::new(registry)
    }

    #[test]
    fn b025_namespace_registry_new_creates_empty_registry() {
        let registry = NamespaceRegistry::new();
        assert!(registry.list_namespaces().is_empty());
    }

    #[test]
    fn b026_namespace_registry_register_inserts_quota() {
        let mut registry = NamespaceRegistry::new();
        let quota =
            NamespaceQuota::new("payments").with_cpu(CpuQuota::new(NonZeroU64::new(4).unwrap()));
        let result = registry.register(quota);
        assert!(result.is_ok());
        assert!(registry.get("payments").is_some());
    }

    #[test]
    fn b027_namespace_registry_register_replaces_existing() {
        let mut registry = NamespaceRegistry::new();
        let q1 =
            NamespaceQuota::new("payments").with_cpu(CpuQuota::new(NonZeroU64::new(2).unwrap()));
        let _ = registry.register(q1);

        let q2 =
            NamespaceQuota::new("payments").with_cpu(CpuQuota::new(NonZeroU64::new(8).unwrap()));
        let _ = registry.register(q2);

        let retrieved = registry.get("payments").unwrap();
        assert_eq!(retrieved.cpu.unwrap().max_cores.get(), 8);
    }

    #[test]
    fn b028_namespace_registry_get_returns_some_for_registered() {
        let mut registry = NamespaceRegistry::new();
        let quota =
            NamespaceQuota::new("test").with_cpu(CpuQuota::new(NonZeroU64::new(2).unwrap()));
        let _ = registry.register(quota);
        assert!(registry.get("test").is_some());
    }

    #[test]
    fn b029_namespace_registry_get_returns_none_for_unregistered() {
        let registry = NamespaceRegistry::new();
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn b030_namespace_registry_remove_returns_some_and_removes() {
        let mut registry = NamespaceRegistry::new();
        let quota =
            NamespaceQuota::new("test").with_cpu(CpuQuota::new(NonZeroU64::new(2).unwrap()));
        let _ = registry.register(quota);
        let removed = registry.remove("test");
        assert!(removed.is_some());
        assert!(registry.get("test").is_none());
    }

    #[test]
    fn b031_namespace_registry_remove_returns_none_for_unregistered() {
        let mut registry = NamespaceRegistry::new();
        let result = registry.remove("nonexistent");
        assert!(result.is_none());
    }

    #[test]
    fn b032_namespace_registry_list_namespaces_returns_all() {
        let mut registry = NamespaceRegistry::new();
        let _ = registry.register(NamespaceQuota::new("a"));
        let _ = registry.register(NamespaceQuota::new("b"));
        let _ = registry.register(NamespaceQuota::new("c"));
        let namespaces = registry.list_namespaces();
        assert_eq!(namespaces.len(), 3);
        assert!(namespaces.contains(&"a"));
        assert!(namespaces.contains(&"b"));
        assert!(namespaces.contains(&"c"));
    }

    #[test]
    fn b033_quota_enforcer_new_constructs_with_registry() {
        let mut registry = NamespaceRegistry::new();
        let _ = registry.register(
            NamespaceQuota::new("ns").with_cpu(CpuQuota::new(NonZeroU64::new(1).unwrap())),
        );
        let enforcer = QuotaEnforcer::new(registry);
        assert!(enforcer.check_cpu("ns", 1).is_ok());
    }

    #[test]
    fn b034_quota_enforcer_with_default_namespace_creates_default() {
        let enforcer = QuotaEnforcer::with_default_namespace();
        assert!(enforcer.registry().get("default").is_some());
    }

    #[test]
    fn b035_default_namespace_has_4_cores_8gb_memory_100gb_disk() {
        let enforcer = QuotaEnforcer::with_default_namespace();
        let ns = enforcer.registry().get("default").unwrap();
        assert_eq!(ns.cpu.unwrap().max_cores.get(), 4);
        assert_eq!(
            ns.memory.unwrap().max_bytes.get(),
            8u64 * 1024 * 1024 * 1024
        );
        assert_eq!(
            ns.disk.unwrap().max_bytes.get(),
            100u64 * 1024 * 1024 * 1024
        );
    }

    #[test]
    fn b036_check_cpu_returns_ok_when_under_limit() {
        let enforcer = make_test_enforcer();
        let result = enforcer.check_cpu("payments", 2);
        assert!(result.is_ok());
    }

    #[test]
    fn b037_check_cpu_returns_ok_when_at_limit() {
        let enforcer = make_test_enforcer();
        let result = enforcer.check_cpu("payments", 4);
        assert!(result.is_ok());
    }

    #[test]
    fn b038_check_cpu_returns_quota_exceeded_when_over_limit() {
        let enforcer = make_test_enforcer();
        let result = enforcer.check_cpu("payments", 8);
        assert!(matches!(
            result,
            Err(QuotaError::QuotaExceeded {
                resource: ResourceKind::Cpu,
                namespace: ns,
                requested: 8,
                available: 4
            }) if ns == "payments"
        ));
    }

    #[test]
    fn b039_check_cpu_returns_ok_when_over_limit_with_allow_overcommit() {
        let enforcer = make_overcommit_enforcer();
        let result = enforcer.check_cpu("payments", 100);
        assert!(result.is_ok());
    }

    #[test]
    fn b040_check_cpu_returns_namespace_not_found_for_unknown() {
        let enforcer = make_test_enforcer();
        let result = enforcer.check_cpu("unknown", 2);
        assert!(matches!(
            result,
            Err(QuotaError::NamespaceNotFound(ns)) if ns == "unknown"
        ));
    }

    #[test]
    fn b041_check_cpu_returns_quota_not_configured_when_no_cpu() {
        let mut registry = NamespaceRegistry::new();
        let _ = registry.register(
            NamespaceQuota::new("no-cpu")
                .with_memory(MemoryQuota::new(NonZeroU64::new(1024).unwrap())),
        );
        let enforcer = QuotaEnforcer::new(registry);
        let result = enforcer.check_cpu("no-cpu", 2);
        assert!(matches!(
            result,
            Err(QuotaError::QuotaNotConfigured {
                resource: ResourceKind::Cpu,
                namespace: ns
            }) if ns == "no-cpu"
        ));
    }

    #[test]
    fn b042_check_memory_returns_ok_when_under_limit() {
        let enforcer = make_test_enforcer();
        let result = enforcer.check_memory("payments", 512);
        assert!(result.is_ok());
    }

    #[test]
    fn b042_check_memory_returns_ok_when_at_limit() {
        let enforcer = make_test_enforcer();
        let result = enforcer.check_memory("payments", 1024);
        assert!(result.is_ok());
    }

    #[test]
    fn b043_check_memory_returns_quota_exceeded_when_over_limit() {
        let enforcer = make_test_enforcer();
        let result = enforcer.check_memory("payments", 2048);
        assert!(matches!(
            result,
            Err(QuotaError::QuotaExceeded {
                resource: ResourceKind::Memory,
                namespace: ns,
                requested: 2048,
                available: 1024
            }) if ns == "payments"
        ));
    }

    #[test]
    fn b044_check_memory_returns_ok_when_over_limit_with_allow_overcommit() {
        let enforcer = make_overcommit_enforcer();
        let result = enforcer.check_memory("payments", u64::MAX);
        assert!(result.is_ok());
    }

    #[test]
    fn b045_check_memory_returns_namespace_not_found_for_unknown() {
        let enforcer = make_test_enforcer();
        let result = enforcer.check_memory("unknown", 100);
        assert!(matches!(
            result,
            Err(QuotaError::NamespaceNotFound(ns)) if ns == "unknown"
        ));
    }

    #[test]
    fn b046_check_memory_returns_quota_not_configured_when_no_memory() {
        let mut registry = NamespaceRegistry::new();
        let _ = registry.register(
            NamespaceQuota::new("no-mem").with_cpu(CpuQuota::new(NonZeroU64::new(4).unwrap())),
        );
        let enforcer = QuotaEnforcer::new(registry);
        let result = enforcer.check_memory("no-mem", 100);
        assert!(matches!(
            result,
            Err(QuotaError::QuotaNotConfigured {
                resource: ResourceKind::Memory,
                namespace: ns
            }) if ns == "no-mem"
        ));
    }

    #[test]
    fn b047_check_disk_returns_ok_when_under_limit() {
        let enforcer = make_test_enforcer();
        let result = enforcer.check_disk("payments", 5000);
        assert!(result.is_ok());
    }

    #[test]
    fn b047_check_disk_returns_ok_when_at_limit() {
        let enforcer = make_test_enforcer();
        let result = enforcer.check_disk("payments", 10_000);
        assert!(result.is_ok());
    }

    #[test]
    fn b048_check_disk_returns_quota_exceeded_when_over_limit() {
        let enforcer = make_test_enforcer();
        let result = enforcer.check_disk("payments", 20_000);
        assert!(matches!(
            result,
            Err(QuotaError::QuotaExceeded {
                resource: ResourceKind::Disk,
                namespace: ns,
                requested: 20_000,
                available: 10_000
            }) if ns == "payments"
        ));
    }

    #[test]
    fn b049_check_disk_returns_ok_when_over_limit_with_allow_overcommit() {
        let enforcer = make_overcommit_enforcer();
        let result = enforcer.check_disk("payments", u64::MAX);
        assert!(result.is_ok());
    }

    #[test]
    fn b050_check_disk_returns_namespace_not_found_for_unknown() {
        let enforcer = make_test_enforcer();
        let result = enforcer.check_disk("unknown", 100);
        assert!(matches!(
            result,
            Err(QuotaError::NamespaceNotFound(ns)) if ns == "unknown"
        ));
    }

    #[test]
    fn b051_check_disk_returns_quota_not_configured_when_no_disk() {
        let mut registry = NamespaceRegistry::new();
        let _ = registry.register(
            NamespaceQuota::new("no-disk").with_cpu(CpuQuota::new(NonZeroU64::new(4).unwrap())),
        );
        let enforcer = QuotaEnforcer::new(registry);
        let result = enforcer.check_disk("no-disk", 100);
        assert!(matches!(
            result,
            Err(QuotaError::QuotaNotConfigured {
                resource: ResourceKind::Disk,
                namespace: ns
            }) if ns == "no-disk"
        ));
    }

    #[test]
    fn b052_overcommit_policy_applies_to_all_resources() {
        let mut registry = NamespaceRegistry::new();
        let _ = registry.register(
            NamespaceQuota::new("payments")
                .with_cpu(CpuQuota::new(NonZeroU64::new(2).unwrap()))
                .with_memory(MemoryQuota::new(NonZeroU64::new(1024).unwrap()))
                .with_disk(DiskQuota::new(NonZeroU64::new(1000).unwrap()))
                .with_overcommit(OvercommitPolicy::AllowOvercommit),
        );
        let enforcer = QuotaEnforcer::new(registry);

        assert!(enforcer.check_cpu("payments", 100).is_ok());
        assert!(enforcer.check_memory("payments", u64::MAX).is_ok());
        assert!(enforcer.check_disk("payments", u64::MAX).is_ok());
    }

    #[test]
    fn edge_remove_namespace_then_check_returns_namespace_not_found() {
        let mut registry = NamespaceRegistry::new();
        let _ = registry.register(
            NamespaceQuota::new("temp").with_cpu(CpuQuota::new(NonZeroU64::new(4).unwrap())),
        );
        let mut enforcer = QuotaEnforcer::new(registry);
        assert!(enforcer.check_cpu("temp", 1).is_ok());
        enforcer.registry_mut().remove("temp");
        let result = enforcer.check_cpu("temp", 1);
        assert!(matches!(result, Err(QuotaError::NamespaceNotFound(_))));
    }

    #[test]
    fn edge_zero_requested_returns_ok() {
        let enforcer = make_test_enforcer();
        assert!(enforcer.check_cpu("payments", 0).is_ok());
        assert!(enforcer.check_memory("payments", 0).is_ok());
        assert!(enforcer.check_disk("payments", 0).is_ok());
    }

    #[test]
    fn namespace_registry_default_creates_empty() {
        let registry = NamespaceRegistry::default();
        assert!(registry.list_namespaces().is_empty());
    }

    #[test]
    fn quota_enforcer_default_is_with_default_namespace() {
        let enforcer = QuotaEnforcer::default();
        assert!(enforcer.check_cpu("default", 1).is_ok());
    }

    #[test]
    fn enforcer_exposes_registry() {
        let enforcer = QuotaEnforcer::with_default_namespace();
        assert!(enforcer.registry().get("default").is_some());
    }

    #[test]
    fn overflow_cpu_quota_at_u64_max_request_u64_max_returns_ok() {
        let mut registry = NamespaceRegistry::new();
        let _ = registry.register(
            NamespaceQuota::new("max-ns")
                .with_cpu(CpuQuota::new(NonZeroU64::new(u64::MAX).unwrap())),
        );
        let enforcer = QuotaEnforcer::new(registry);
        assert!(enforcer.check_cpu("max-ns", u64::MAX).is_ok());
    }

    #[test]
    fn overflow_cpu_quota_at_u64_max_request_max_minus_one_returns_ok() {
        let mut registry = NamespaceRegistry::new();
        let _ = registry.register(
            NamespaceQuota::new("max-ns")
                .with_cpu(CpuQuota::new(NonZeroU64::new(u64::MAX).unwrap())),
        );
        let enforcer = QuotaEnforcer::new(registry);
        assert!(enforcer.check_cpu("max-ns", u64::MAX - 1).is_ok());
    }

    #[test]
    fn overflow_memory_quota_at_u64_max_request_u64_max_returns_ok() {
        let mut registry = NamespaceRegistry::new();
        let _ = registry.register(
            NamespaceQuota::new("max-ns")
                .with_memory(MemoryQuota::new(NonZeroU64::new(u64::MAX).unwrap())),
        );
        let enforcer = QuotaEnforcer::new(registry);
        assert!(enforcer.check_memory("max-ns", u64::MAX).is_ok());
    }

    #[test]
    fn overflow_disk_quota_at_u64_max_request_u64_max_returns_ok() {
        let mut registry = NamespaceRegistry::new();
        let _ = registry.register(
            NamespaceQuota::new("max-ns")
                .with_disk(DiskQuota::new(NonZeroU64::new(u64::MAX).unwrap())),
        );
        let enforcer = QuotaEnforcer::new(registry);
        assert!(enforcer.check_disk("max-ns", u64::MAX).is_ok());
    }

    #[test]
    fn overflow_cpu_quota_max_minus_one_request_u64_max_returns_exceeded() {
        let mut registry = NamespaceRegistry::new();
        let _ = registry.register(
            NamespaceQuota::new("max-ns")
                .with_cpu(CpuQuota::new(NonZeroU64::new(u64::MAX - 1).unwrap())),
        );
        let enforcer = QuotaEnforcer::new(registry);
        let err = enforcer.check_cpu("max-ns", u64::MAX).unwrap_err();
        assert!(matches!(err, QuotaError::QuotaExceeded { resource: ResourceKind::Cpu, requested: u64::MAX, available, .. } if available == u64::MAX - 1));
    }

    #[test]
    fn overflow_memory_quota_max_minus_one_request_u64_max_returns_exceeded() {
        let mut registry = NamespaceRegistry::new();
        let _ = registry.register(
            NamespaceQuota::new("max-ns")
                .with_memory(MemoryQuota::new(NonZeroU64::new(u64::MAX - 1).unwrap())),
        );
        let enforcer = QuotaEnforcer::new(registry);
        let err = enforcer.check_memory("max-ns", u64::MAX).unwrap_err();
        assert!(matches!(err, QuotaError::QuotaExceeded { resource: ResourceKind::Memory, requested: u64::MAX, available, .. } if available == u64::MAX - 1));
    }

    #[test]
    fn overflow_disk_quota_max_minus_one_request_u64_max_returns_exceeded() {
        let mut registry = NamespaceRegistry::new();
        let _ = registry.register(
            NamespaceQuota::new("max-ns")
                .with_disk(DiskQuota::new(NonZeroU64::new(u64::MAX - 1).unwrap())),
        );
        let enforcer = QuotaEnforcer::new(registry);
        let err = enforcer.check_disk("max-ns", u64::MAX).unwrap_err();
        assert!(matches!(err, QuotaError::QuotaExceeded { resource: ResourceKind::Disk, requested: u64::MAX, available, .. } if available == u64::MAX - 1));
    }

    #[test]
    fn overflow_all_resources_at_u64_max_with_overcommit_returns_ok() {
        let mut registry = NamespaceRegistry::new();
        let _ = registry.register(
            NamespaceQuota::new("max-ns")
                .with_cpu(CpuQuota::new(NonZeroU64::new(1).unwrap()))
                .with_memory(MemoryQuota::new(NonZeroU64::new(1).unwrap()))
                .with_disk(DiskQuota::new(NonZeroU64::new(1).unwrap()))
                .with_overcommit(OvercommitPolicy::AllowOvercommit),
        );
        let enforcer = QuotaEnforcer::new(registry);
        assert!(enforcer.check_cpu("max-ns", u64::MAX).is_ok());
        assert!(enforcer.check_memory("max-ns", u64::MAX).is_ok());
        assert!(enforcer.check_disk("max-ns", u64::MAX).is_ok());
    }

    #[test]
    fn overflow_quota_usage_holds_u64_max_values() {
        let usage = QuotaUsage::new()
            .with_cpu(u64::MAX)
            .with_memory(u64::MAX)
            .with_disk(u64::MAX);
        assert_eq!(usage.cpu_cores_used, u64::MAX);
        assert_eq!(usage.memory_bytes_used, u64::MAX);
        assert_eq!(usage.disk_bytes_used, u64::MAX);
    }

    #[test]
    fn overflow_cpu_quota_at_u64_max_request_zero_returns_ok() {
        let mut registry = NamespaceRegistry::new();
        let _ = registry.register(
            NamespaceQuota::new("max-ns")
                .with_cpu(CpuQuota::new(NonZeroU64::new(u64::MAX).unwrap())),
        );
        let enforcer = QuotaEnforcer::new(registry);
        assert!(enforcer.check_cpu("max-ns", 0).is_ok());
    }

    #[test]
    fn overflow_cpu_quota_at_one_request_u64_max_returns_exceeded() {
        let mut registry = NamespaceRegistry::new();
        let _ = registry.register(
            NamespaceQuota::new("max-ns")
                .with_cpu(CpuQuota::new(NonZeroU64::new(1).unwrap())),
        );
        let enforcer = QuotaEnforcer::new(registry);
        assert!(matches!(
            enforcer.check_cpu("max-ns", u64::MAX),
            Err(QuotaError::QuotaExceeded {
                resource: ResourceKind::Cpu,
                requested: u64::MAX,
                available: 1,
                ..
            })
        ));
    }
}
