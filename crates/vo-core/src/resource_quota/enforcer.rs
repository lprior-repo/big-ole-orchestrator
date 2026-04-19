//! Quota enforcer for resource quota checking and enforcement.

use super::registry::NamespaceRegistry;
use super::types::{CpuQuota, DiskQuota, MemoryQuota, NamespaceQuota, QuotaError, ResourceKind};

#[derive(Debug, Clone)]
pub struct QuotaEnforcer {
    registry: NamespaceRegistry,
}

impl QuotaEnforcer {
    #[must_use]
    pub fn new(registry: NamespaceRegistry) -> Self {
        Self { registry }
    }

    #[must_use]
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

    #[must_use]
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
