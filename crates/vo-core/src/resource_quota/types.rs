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
