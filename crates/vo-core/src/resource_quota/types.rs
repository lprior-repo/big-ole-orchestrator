use serde::{Deserialize, Serialize};
use std::num::NonZeroU64;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotaUsage {
    pub cpu_cores_used: u64,
    pub memory_bytes_used: u64,
    pub disk_bytes_used: u64,
}

impl QuotaUsage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            cpu_cores_used: 0,
            memory_bytes_used: 0,
            disk_bytes_used: 0,
        }
    }

    #[must_use]
    pub fn with_cpu(mut self, cores: u64) -> Self {
        self.cpu_cores_used = cores;
        self
    }

    #[must_use]
    pub fn with_memory(mut self, bytes: u64) -> Self {
        self.memory_bytes_used = bytes;
        self
    }

    #[must_use]
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
    #[must_use]
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
    #[must_use]
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
    #[must_use]
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
    #[must_use]
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
    pub overcommit: super::policy::OvercommitPolicy,
}

impl NamespaceQuota {
    #[must_use]
    pub fn new(namespace: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            cpu: None,
            memory: None,
            disk: None,
            overcommit: super::policy::OvercommitPolicy::default(),
        }
    }

    #[must_use]
    pub fn with_cpu(mut self, quota: CpuQuota) -> Self {
        self.cpu = Some(quota);
        self
    }

    #[must_use]
    pub fn with_memory(mut self, quota: MemoryQuota) -> Self {
        self.memory = Some(quota);
        self
    }

    #[must_use]
    pub fn with_disk(mut self, quota: DiskQuota) -> Self {
        self.disk = Some(quota);
        self
    }

    #[must_use]
    pub fn with_overcommit(mut self, policy: super::policy::OvercommitPolicy) -> Self {
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
    #[must_use]
    pub const fn is_overcommit_rejected(&self) -> bool {
        matches!(
            self,
            QuotaError::QuotaExceeded { .. } | QuotaError::QuotaNotConfigured { .. }
        )
    }
}

/// Soft limit percentage for quota warning thresholds.
///
/// Represents a percentage (0-100) of the hard limit at which a warning
/// should be emitted. For example, `SoftLimitPercent::new(80)` triggers
/// a warning when usage reaches 80% of the hard limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SoftLimitPercent(u8);

impl SoftLimitPercent {
    pub const DEFAULT: Self = Self(80);

    /// Creates a new soft limit percentage. Clamped to [1, 99].
    #[must_use]
    pub fn new(percent: u8) -> Self {
        Self(percent.clamp(1, 99))
    }

    #[must_use]
    pub fn value(&self) -> u8 {
        self.0
    }

    /// Returns the threshold value for a given hard limit.
    /// `soft_threshold = (hard_limit * percent) / 100`
    #[must_use]
    pub fn threshold_for(&self, hard_limit: u64) -> u64 {
        (hard_limit * u64::from(self.0)) / 100
    }
}

impl Default for SoftLimitPercent {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// A warning emitted when resource usage crosses the soft limit threshold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoftLimitWarning {
    pub resource: ResourceKind,
    pub namespace: String,
    pub requested: u64,
    pub soft_threshold: u64,
    pub hard_limit: u64,
    pub utilization_percent: u8,
}

impl SoftLimitWarning {
    #[must_use]
    pub fn new(
        resource: ResourceKind,
        namespace: impl Into<String>,
        requested: u64,
        soft_threshold: u64,
        hard_limit: u64,
    ) -> Self {
        let utilization_percent = if hard_limit > 0 {
            ((requested as u128 * 100) / hard_limit as u128).clamp(1, 100) as u8
        } else {
            100
        };
        Self {
            resource,
            namespace: namespace.into(),
            requested,
            soft_threshold,
            hard_limit,
            utilization_percent,
        }
    }
}

impl std::fmt::Display for SoftLimitWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "soft limit warning for {} in namespace {}: \
             requested {} ({}% of hard limit {}), \
             soft threshold at {}",
            self.resource,
            self.namespace,
            self.requested,
            self.utilization_percent,
            self.hard_limit,
            self.soft_threshold,
        )
    }
}

/// Result of a quota check with soft limit awareness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuotaCheckResult {
    /// Usage is within the soft limit threshold.
    WithinLimits,
    /// Usage has crossed the soft limit but is still under the hard limit.
    SoftLimitExceeded(SoftLimitWarning),
    /// Usage has exceeded the hard limit.
    HardLimitExceeded(QuotaError),
}

impl QuotaCheckResult {
    #[must_use]
    pub fn is_within_limits(&self) -> bool {
        matches!(self, QuotaCheckResult::WithinLimits)
    }

    #[must_use]
    pub fn is_soft_warning(&self) -> bool {
        matches!(self, QuotaCheckResult::SoftLimitExceeded(_))
    }

    #[must_use]
    pub fn is_hard_exceeded(&self) -> bool {
        matches!(self, QuotaCheckResult::HardLimitExceeded(_))
    }

    /// Converts to `Result<(), QuotaError>`, discarding soft warnings.
    #[must_use]
    pub fn into_hard_result(self) -> Result<(), QuotaError> {
        match self {
            QuotaCheckResult::WithinLimits | QuotaCheckResult::SoftLimitExceeded(_) => Ok(()),
            QuotaCheckResult::HardLimitExceeded(err) => Err(err),
        }
    }

    /// Extracts the soft warning, if any.
    #[must_use]
    pub fn soft_warning(&self) -> Option<&SoftLimitWarning> {
        match self {
            QuotaCheckResult::SoftLimitExceeded(w) => Some(w),
            _ => None,
        }
    }
}
