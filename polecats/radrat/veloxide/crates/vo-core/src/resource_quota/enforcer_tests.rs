use super::policy::OvercommitPolicy;
use super::registry::NamespaceRegistry;
use super::types::{CpuQuota, DiskQuota, MemoryQuota, NamespaceQuota};
use super::QuotaEnforcer;
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
fn b033_quota_enforcer_new_constructs_with_registry() {
    let mut registry = NamespaceRegistry::new();
    let _ = registry
        .register(NamespaceQuota::new("ns").with_cpu(CpuQuota::new(NonZeroU64::new(1).unwrap())));
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
    use super::types::QuotaError;
    use super::ResourceKind;
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
    use super::types::QuotaError;
    let enforcer = make_test_enforcer();
    let result = enforcer.check_cpu("unknown", 2);
    assert!(matches!(
        result,
        Err(QuotaError::NamespaceNotFound(ns)) if ns == "unknown"
    ));
}

#[test]
fn b041_check_cpu_returns_quota_not_configured_when_no_cpu() {
    use super::types::{NamespaceQuota, QuotaError, ResourceKind};
    let mut registry = NamespaceRegistry::new();
    let _ = registry.register(
        NamespaceQuota::new("no-cpu").with_memory(MemoryQuota::new(NonZeroU64::new(1024).unwrap())),
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
    use super::types::{NamespaceQuota, QuotaError, ResourceKind};
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
    use super::types::QuotaError;
    let enforcer = make_test_enforcer();
    let result = enforcer.check_memory("unknown", 100);
    assert!(matches!(
        result,
        Err(QuotaError::NamespaceNotFound(ns)) if ns == "unknown"
    ));
}

#[test]
fn b046_check_memory_returns_quota_not_configured_when_no_memory() {
    use super::types::{CpuQuota, NamespaceQuota, QuotaError, ResourceKind};
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
    use super::types::{NamespaceQuota, QuotaError, ResourceKind};
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
    use super::types::QuotaError;
    let enforcer = make_test_enforcer();
    let result = enforcer.check_disk("unknown", 100);
    assert!(matches!(
        result,
        Err(QuotaError::NamespaceNotFound(ns)) if ns == "unknown"
    ));
}

#[test]
fn b051_check_disk_returns_quota_not_configured_when_no_disk() {
    use super::types::{CpuQuota, NamespaceQuota, QuotaError, ResourceKind};
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
    use super::policy::OvercommitPolicy;
    use super::registry::NamespaceRegistry;
    use super::types::{CpuQuota, DiskQuota, MemoryQuota, NamespaceQuota};
    use super::QuotaEnforcer;
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
    use super::types::{CpuQuota, NamespaceQuota, QuotaError};
    let mut registry = NamespaceRegistry::new();
    let _ = registry
        .register(NamespaceQuota::new("temp").with_cpu(CpuQuota::new(NonZeroU64::new(4).unwrap())));
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
fn quota_enforcer_default_is_with_default_namespace() {
    let enforcer = QuotaEnforcer::default();
    assert!(enforcer.check_cpu("default", 1).is_ok());
}

#[test]
fn enforcer_exposes_registry() {
    let enforcer = QuotaEnforcer::with_default_namespace();
    assert!(enforcer.registry().get("default").is_some());
}
