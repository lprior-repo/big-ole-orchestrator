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
            .with_cpu(CpuQuota::new(std::num::NonZeroU64::new(4).unwrap()))
            .with_memory(MemoryQuota::new(
                std::num::NonZeroU64::new(8 * 1024 * 1024 * 1024).unwrap(),
            ))
            .with_disk(DiskQuota::new(
                std::num::NonZeroU64::new(100 * 1024 * 1024 * 1024).unwrap(),
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
    use crate::resource_quota::OvercommitPolicy;
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

    #[test]
    fn quota_enforcer_with_default_namespace_creates_with_defaults() {
        let enforcer = QuotaEnforcer::with_default_namespace();
        let result = enforcer.check_cpu("default", 2);
        assert!(result.is_ok());
    }

    #[test]
    fn check_cpu_returns_ok_when_under_limit() {
        let enforcer = make_test_enforcer();
        let result = enforcer.check_cpu("payments", 2);
        assert!(result.is_ok());
    }

    #[test]
    fn check_cpu_returns_ok_when_at_limit() {
        let enforcer = make_test_enforcer();
        let result = enforcer.check_cpu("payments", 4);
        assert!(result.is_ok());
    }

    #[test]
    fn check_cpu_returns_quota_exceeded_when_over_limit() {
        let enforcer = make_test_enforcer();
        let result = enforcer.check_cpu("payments", 8);
        assert!(matches!(
            result,
            Err(QuotaError::QuotaExceeded {
                resource: ResourceKind::Cpu,
                ..
            })
        ));
    }

    #[test]
    fn check_cpu_returns_namespace_not_found_when_unknown_namespace() {
        let enforcer = make_test_enforcer();
        let result = enforcer.check_cpu("unknown", 2);
        assert!(matches!(
            result,
            Err(QuotaError::NamespaceNotFound(ns)) if ns == "unknown"
        ));
    }

    #[test]
    fn check_cpu_returns_quota_not_configured_when_no_cpu_quota() {
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
                ..
            })
        ));
    }

    #[test]
    fn check_memory_returns_ok_when_under_limit() {
        let enforcer = make_test_enforcer();
        let result = enforcer.check_memory("payments", 512);
        assert!(result.is_ok());
    }

    #[test]
    fn check_memory_returns_ok_when_at_limit() {
        let enforcer = make_test_enforcer();
        let result = enforcer.check_memory("payments", 1024);
        assert!(result.is_ok());
    }

    #[test]
    fn check_memory_returns_quota_exceeded_when_over_limit() {
        let enforcer = make_test_enforcer();
        let result = enforcer.check_memory("payments", 2048);
        assert!(matches!(
            result,
            Err(QuotaError::QuotaExceeded {
                resource: ResourceKind::Memory,
                ..
            })
        ));
    }

    #[test]
    fn check_disk_returns_ok_when_under_limit() {
        let enforcer = make_test_enforcer();
        let result = enforcer.check_disk("payments", 5000);
        assert!(result.is_ok());
    }

    #[test]
    fn check_disk_returns_ok_when_at_limit() {
        let enforcer = make_test_enforcer();
        let result = enforcer.check_disk("payments", 10_000);
        assert!(result.is_ok());
    }

    #[test]
    fn check_disk_returns_quota_exceeded_when_over_limit() {
        let enforcer = make_test_enforcer();
        let result = enforcer.check_disk("payments", 20_000);
        assert!(matches!(
            result,
            Err(QuotaError::QuotaExceeded {
                resource: ResourceKind::Disk,
                ..
            })
        ));
    }

    #[test]
    fn check_allows_overcommit_when_policy_is_allow_overcommit() {
        let mut registry = NamespaceRegistry::new();
        let _ = registry.register(
            NamespaceQuota::new("overcommit-ns")
                .with_cpu(CpuQuota::new(NonZeroU64::new(4).unwrap()))
                .with_overcommit(OvercommitPolicy::AllowOvercommit),
        );
        let enforcer = QuotaEnforcer::new(registry);
        let result = enforcer.check_cpu("overcommit-ns", 100);
        assert!(result.is_ok());
    }

    #[test]
    fn namespace_registry_register_inserts_quota() {
        let mut registry = NamespaceRegistry::new();
        let quota =
            NamespaceQuota::new("test").with_cpu(CpuQuota::new(NonZeroU64::new(2).unwrap()));
        let result = registry.register(quota);
        assert!(result.is_ok());
        assert!(registry.get("test").is_some());
    }

    #[test]
    fn namespace_registry_remove_returns_and_removes_quota() {
        let mut registry = NamespaceRegistry::new();
        let quota =
            NamespaceQuota::new("test").with_cpu(CpuQuota::new(NonZeroU64::new(2).unwrap()));
        let _ = registry.register(quota);
        let removed = registry.remove("test");
        assert!(removed.is_some());
        assert!(registry.get("test").is_none());
    }

    #[test]
    fn namespace_registry_list_namespaces_returns_all() {
        let mut registry = NamespaceRegistry::new();
        let _ = registry.register(NamespaceQuota::new("a"));
        let _ = registry.register(NamespaceQuota::new("b"));
        let namespaces = registry.list_namespaces();
        assert!(namespaces.contains(&"a"));
        assert!(namespaces.contains(&"b"));
    }
}
