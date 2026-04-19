use super::{
    CpuQuota, DiskQuota, MemoryQuota, NamespaceQuota, NamespaceRegistry, OvercommitPolicy,
    QuotaEnforcer, QuotaUsage, ResourceKind,
};
use std::num::NonZeroU64;

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
    let quota = NamespaceQuota::new("payments").with_overcommit(OvercommitPolicy::AllowOvercommit);
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
    use crate::resource_quota::types::QuotaError;
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
    use crate::resource_quota::types::QuotaError;
    let err = QuotaError::QuotaNotConfigured {
        resource: ResourceKind::Memory,
        namespace: "payments".to_string(),
    };
    assert!(err.is_overcommit_rejected());
}

#[test]
fn b055_quota_error_is_not_overcommit_rejected_for_namespace_not_found() {
    use crate::resource_quota::types::QuotaError;
    let err = QuotaError::NamespaceNotFound("unknown".to_string());
    assert!(!err.is_overcommit_rejected());
}

#[test]
fn b056_quota_exceeded_error_display_includes_resource_namespace_requested_available() {
    use crate::resource_quota::types::QuotaError;
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
    use crate::resource_quota::types::QuotaError;
    let err = QuotaError::NamespaceNotFound("my-ns".to_string());
    let display = err.to_string();
    assert!(
        display.contains("my-ns"),
        "display should contain namespace name"
    );
}

#[test]
fn b058_quota_not_configured_error_display_includes_resource_and_namespace() {
    use crate::resource_quota::types::QuotaError;
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
fn edge_zero_requested_cores_returns_ok() {
    let enforcer = QuotaEnforcer::with_default_namespace();
    let result = enforcer.check_cpu("default", 0);
    assert!(result.is_ok());
}

#[test]
fn edge_u64_max_requested_no_overcommit_returns_quota_exceeded() {
    let enforcer = QuotaEnforcer::with_default_namespace();
    let result = enforcer.check_memory("default", u64::MAX);
    assert!(result.is_err());
}

#[test]
fn edge_empty_namespace_string_is_allowed() {
    let mut registry = NamespaceRegistry::new();
    let quota = NamespaceQuota::new("").with_cpu(CpuQuota::new(NonZeroU64::new(1).unwrap()));
    let result = registry.register(quota);
    assert!(result.is_ok());
    assert!(registry.get("").is_some());
}

#[test]
fn edge_special_characters_in_namespace() {
    let mut registry = NamespaceRegistry::new();
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
