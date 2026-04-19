//! Quota enforcer for resource quota checking and enforcement.

use super::registry::NamespaceRegistry;
use super::types::{
    CpuQuota, DiskQuota, MemoryQuota, NamespaceQuota, QuotaCheckResult, QuotaError,
    SoftLimitPercent, SoftLimitWarning,
};
use super::ResourceKind;
use super::warning_tracker::QuotaWarningTracker;

pub struct QuotaEnforcer {
    registry: NamespaceRegistry,
    warning_tracker: QuotaWarningTracker,
}

impl std::fmt::Debug for QuotaEnforcer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QuotaEnforcer")
            .field("registry", &self.registry)
            .field("warning_tracker", &self.warning_tracker.interval())
            .finish()
    }
}

impl QuotaEnforcer {
    #[must_use]
    pub fn new(registry: NamespaceRegistry) -> Self {
        Self {
            registry,
            warning_tracker: QuotaWarningTracker::new(),
        }
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
        Self {
            registry,
            warning_tracker: QuotaWarningTracker::new(),
        }
    }

    pub fn check_cpu(&self, namespace: &str, requested_cores: u64) -> Result<(), QuotaError> {
        self.check_cpu_with_soft_limit(namespace, requested_cores, SoftLimitPercent::DEFAULT)
            .into_hard_result()
    }

    pub fn check_cpu_with_soft_limit(
        &self,
        namespace: &str,
        requested_cores: u64,
        soft_percent: SoftLimitPercent,
    ) -> QuotaCheckResult {
        self.check_resource(namespace, ResourceKind::Cpu, requested_cores, soft_percent, |q| {
            q.cpu.as_ref().map(|c| c.max_cores.get())
        })
    }

    pub fn check_memory(&self, namespace: &str, requested_bytes: u64) -> Result<(), QuotaError> {
        self.check_memory_with_soft_limit(namespace, requested_bytes, SoftLimitPercent::DEFAULT)
            .into_hard_result()
    }

    pub fn check_memory_with_soft_limit(
        &self,
        namespace: &str,
        requested_bytes: u64,
        soft_percent: SoftLimitPercent,
    ) -> QuotaCheckResult {
        self.check_resource(namespace, ResourceKind::Memory, requested_bytes, soft_percent, |q| {
            q.memory.as_ref().map(|m| m.max_bytes.get())
        })
    }

    pub fn check_disk(&self, namespace: &str, requested_bytes: u64) -> Result<(), QuotaError> {
        self.check_disk_with_soft_limit(namespace, requested_bytes, SoftLimitPercent::DEFAULT)
            .into_hard_result()
    }

    pub fn check_disk_with_soft_limit(
        &self,
        namespace: &str,
        requested_bytes: u64,
        soft_percent: SoftLimitPercent,
    ) -> QuotaCheckResult {
        self.check_resource(namespace, ResourceKind::Disk, requested_bytes, soft_percent, |q| {
            q.disk.as_ref().map(|d| d.max_bytes.get())
        })
    }

    fn check_resource<F>(
        &self,
        namespace: &str,
        resource: ResourceKind,
        requested: u64,
        soft_percent: SoftLimitPercent,
        get_limit: F,
    ) -> QuotaCheckResult
    where
        F: Fn(&NamespaceQuota) -> Option<u64>,
    {
        let quota = match self.registry.get(namespace) {
            Some(q) => q,
            None => {
                return QuotaCheckResult::HardLimitExceeded(QuotaError::NamespaceNotFound(
                    namespace.to_string(),
                ));
            }
        };

        let hard_limit = match get_limit(quota) {
            Some(limit) => limit,
            None => {
                return QuotaCheckResult::HardLimitExceeded(QuotaError::QuotaNotConfigured {
                    resource,
                    namespace: namespace.to_string(),
                });
            }
        };

        if requested > hard_limit {
            if quota.overcommit.allows_overcommit() {
                return QuotaCheckResult::WithinLimits;
            }
            return QuotaCheckResult::HardLimitExceeded(QuotaError::QuotaExceeded {
                resource,
                namespace: namespace.to_string(),
                requested,
                available: hard_limit,
            });
        }

        let soft_threshold = soft_percent.threshold_for(hard_limit);
        if requested >= soft_threshold {
            let warning = SoftLimitWarning::new(
                resource,
                namespace,
                requested,
                soft_threshold,
                hard_limit,
            );
            self.warning_tracker.record_warning(&warning);
            return QuotaCheckResult::SoftLimitExceeded(warning);
        }

        self.warning_tracker.clear_warning(resource);
        QuotaCheckResult::WithinLimits
    }

    #[must_use]
    pub fn registry(&self) -> &NamespaceRegistry {
        &self.registry
    }

    pub fn registry_mut(&mut self) -> &mut NamespaceRegistry {
        &mut self.registry
    }

    #[must_use]
    pub fn warning_tracker(&self) -> &QuotaWarningTracker {
        &self.warning_tracker
    }
}

impl Default for QuotaEnforcer {
    fn default() -> Self {
        Self::with_default_namespace()
    }
}
