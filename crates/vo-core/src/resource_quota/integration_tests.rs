use super::*;
use crate::resource_quota::{
    CpuQuota, DiskQuota, MemoryQuota, NamespaceQuota, OvercommitPolicy, QuotaEnforcer, QuotaError,
    QuotaUsage,
};
use std::num::NonZeroU64;

fn make_full_enforcer() -> QuotaEnforcer {
    let mut registry = super::enforcer::NamespaceRegistry::new();
    let _ = registry.register(
        NamespaceQuota::new("payments")
            .with_cpu(CpuQuota::new(NonZeroU64::new(4).unwrap()))
            .with_memory(MemoryQuota::new(NonZeroU64::new(1024).unwrap()))
            .with_disk(DiskQuota::new(NonZeroU64::new(10_000).unwrap()))
            .with_overcommit(OvercommitPolicy::NoOvercommit),
    );
    let _ = registry.register(
        NamespaceQuota::new("analytics")
            .with_cpu(CpuQuota::new(NonZeroU64::new(16).unwrap()))
            .with_memory(MemoryQuota::new(NonZeroU64::new(8192).unwrap()))
            .with_disk(DiskQuota::new(NonZeroU64::new(100_000).unwrap()))
            .with_overcommit(OvercommitPolicy::AllowOvercommit),
    );
    let _ = registry.register(
        NamespaceQuota::new("partial").with_cpu(CpuQuota::new(NonZeroU64::new(2).unwrap())),
    );
    QuotaEnforcer::new(registry)
}

#[test]
fn integration_full_quota_check_workflow() {
    let enforcer = make_full_enforcer();

    assert!(enforcer.check_cpu("payments", 2).is_ok());
    assert!(enforcer.check_cpu("payments", 4).is_ok());
    assert!(matches!(
        enforcer.check_cpu("payments", 5),
        Err(QuotaError::QuotaExceeded { .. })
    ));

    assert!(enforcer.check_cpu("analytics", 100).is_ok());
    assert!(enforcer.check_memory("analytics", 999_999).is_ok());
    assert!(enforcer.check_disk("analytics", 999_999).is_ok());

    assert!(enforcer.check_cpu("partial", 1).is_ok());
    assert!(matches!(
        enforcer.check_memory("partial", 1),
        Err(QuotaError::QuotaNotConfigured { .. })
    ));
    assert!(matches!(
        enforcer.check_disk("partial", 1),
        Err(QuotaError::QuotaNotConfigured { .. })
    ));

    assert!(matches!(
        enforcer.check_cpu("nonexistent", 1),
        Err(QuotaError::NamespaceNotFound(_))
    ));
}

#[test]
fn integration_default_namespace_workflow() {
    let enforcer = QuotaEnforcer::with_default_namespace();

    assert!(enforcer.check_cpu("default", 4).is_ok());
    assert!(matches!(
        enforcer.check_cpu("default", 5),
        Err(QuotaError::QuotaExceeded { .. })
    ));

    let mem_limit = 8u64 * 1024 * 1024 * 1024;
    assert!(enforcer.check_memory("default", mem_limit).is_ok());
    assert!(matches!(
        enforcer.check_memory("default", mem_limit + 1),
        Err(QuotaError::QuotaExceeded { .. })
    ));

    let disk_limit = 100u64 * 1024 * 1024 * 1024;
    assert!(enforcer.check_disk("default", disk_limit).is_ok());
    assert!(matches!(
        enforcer.check_disk("default", disk_limit + 1),
        Err(QuotaError::QuotaExceeded { .. })
    ));
}

#[test]
fn integration_register_replace_remove_lifecycle() {
    let mut registry = super::enforcer::NamespaceRegistry::new();

    let _ = registry.register(
        NamespaceQuota::new("svc")
            .with_cpu(CpuQuota::new(NonZeroU64::new(2).unwrap()))
            .with_memory(MemoryQuota::new(NonZeroU64::new(512).unwrap())),
    );

    let enforcer = QuotaEnforcer::new(registry);
    assert!(enforcer.check_cpu("svc", 2).is_ok());
    assert!(matches!(
        enforcer.check_disk("svc", 1),
        Err(QuotaError::QuotaNotConfigured { .. })
    ));

    let mut registry2 = super::enforcer::NamespaceRegistry::new();
    let _ = registry2.register(
        NamespaceQuota::new("svc")
            .with_cpu(CpuQuota::new(NonZeroU64::new(8).unwrap()))
            .with_disk(DiskQuota::new(NonZeroU64::new(5000).unwrap())),
    );
    let enforcer2 = QuotaEnforcer::new(registry2);
    assert!(enforcer2.check_cpu("svc", 8).is_ok());
    assert!(enforcer2.check_disk("svc", 5000).is_ok());
}

#[test]
fn integration_error_taxonomy_complete() {
    let enforcer = make_full_enforcer();

    let cpu_exceeded = enforcer.check_cpu("payments", 100);
    assert!(matches!(
        cpu_exceeded,
        Err(QuotaError::QuotaExceeded {
            resource: ResourceKind::Cpu,
            namespace: ref ns,
            requested: 100,
            available: 4
        }) if ns == "payments"
    ));
    if let Err(ref e) = cpu_exceeded {
        assert!(e.is_overcommit_rejected());
        let display = e.to_string();
        assert!(display.contains("cpu"));
        assert!(display.contains("payments"));
        assert!(display.contains("100"));
        assert!(display.contains("4"));
    }

    let mem_not_configured = enforcer.check_memory("partial", 1);
    assert!(matches!(
        mem_not_configured,
        Err(QuotaError::QuotaNotConfigured {
            resource: ResourceKind::Memory,
            namespace: ref ns
        }) if ns == "partial"
    ));
    if let Err(ref e) = mem_not_configured {
        assert!(e.is_overcommit_rejected());
    }

    let not_found = enforcer.check_disk("ghost", 1);
    assert!(matches!(
        not_found,
        Err(QuotaError::NamespaceNotFound(ref ns)) if ns == "ghost"
    ));
    if let Err(ref e) = not_found {
        assert!(!e.is_overcommit_rejected());
        let display = e.to_string();
        assert!(display.contains("ghost"));
    }
}

#[test]
fn integration_overcommit_policy_uniform_across_resources() {
    let mut registry = super::enforcer::NamespaceRegistry::new();
    let _ = registry.register(
        NamespaceQuota::new("elastic")
            .with_cpu(CpuQuota::new(NonZeroU64::new(1).unwrap()))
            .with_memory(MemoryQuota::new(NonZeroU64::new(1).unwrap()))
            .with_disk(DiskQuota::new(NonZeroU64::new(1).unwrap()))
            .with_overcommit(OvercommitPolicy::AllowOvercommit),
    );
    let enforcer = QuotaEnforcer::new(registry);

    assert!(enforcer.check_cpu("elastic", u64::MAX).is_ok());
    assert!(enforcer.check_memory("elastic", u64::MAX).is_ok());
    assert!(enforcer.check_disk("elastic", u64::MAX).is_ok());

    let mut registry2 = super::enforcer::NamespaceRegistry::new();
    let _ = registry2.register(
        NamespaceQuota::new("strict")
            .with_cpu(CpuQuota::new(NonZeroU64::new(1).unwrap()))
            .with_memory(MemoryQuota::new(NonZeroU64::new(1).unwrap()))
            .with_disk(DiskQuota::new(NonZeroU64::new(1).unwrap()))
            .with_overcommit(OvercommitPolicy::NoOvercommit),
    );
    let enforcer2 = QuotaEnforcer::new(registry2);

    assert!(matches!(
        enforcer2.check_cpu("strict", 2),
        Err(QuotaError::QuotaExceeded { .. })
    ));
    assert!(matches!(
        enforcer2.check_memory("strict", 2),
        Err(QuotaError::QuotaExceeded { .. })
    ));
    assert!(matches!(
        enforcer2.check_disk("strict", 2),
        Err(QuotaError::QuotaExceeded { .. })
    ));
}

#[test]
fn integration_multiple_namespaces_independent() {
    let enforcer = make_full_enforcer();

    assert!(matches!(
        enforcer.check_cpu("payments", 5),
        Err(QuotaError::QuotaExceeded { .. })
    ));
    assert!(enforcer.check_cpu("analytics", 5).is_ok());
    assert!(enforcer.check_cpu("partial", 1).is_ok());
    assert!(matches!(
        enforcer.check_cpu("partial", 3),
        Err(QuotaError::QuotaExceeded { .. })
    ));
}

#[test]
fn integration_boundary_exact_limits() {
    let enforcer = make_full_enforcer();

    assert!(enforcer.check_cpu("payments", 4).is_ok());
    assert!(enforcer.check_memory("payments", 1024).is_ok());
    assert!(enforcer.check_disk("payments", 10_000).is_ok());

    assert!(matches!(
        enforcer.check_cpu("payments", 5),
        Err(QuotaError::QuotaExceeded { .. })
    ));
    assert!(matches!(
        enforcer.check_memory("payments", 1025),
        Err(QuotaError::QuotaExceeded { .. })
    ));
    assert!(matches!(
        enforcer.check_disk("payments", 10_001),
        Err(QuotaError::QuotaExceeded { .. })
    ));
}

#[test]
fn integration_overflow_accumulate_and_check_enforcement() {
    let mut usage = QuotaUsage::new();
    usage.add_cpu(2);
    usage.add_memory(512);
    usage.add_disk(3000);

    let enforcer = make_full_enforcer();
    assert!(enforcer.check_cpu("payments", usage.cpu_cores_used).is_ok());
    assert!(enforcer.check_memory("payments", usage.memory_bytes_used).is_ok());
    assert!(enforcer.check_disk("payments", usage.disk_bytes_used).is_ok());

    usage.add_cpu(10);
    assert!(matches!(
        enforcer.check_cpu("payments", usage.cpu_cores_used),
        Err(QuotaError::QuotaExceeded { .. })
    ));
}

#[test]
fn integration_overflow_release_and_check() {
    let mut usage = QuotaUsage::new();
    usage.add_cpu(8);
    usage.add_memory(2048);

    let enforcer = make_full_enforcer();
    assert!(matches!(
        enforcer.check_cpu("payments", usage.cpu_cores_used),
        Err(QuotaError::QuotaExceeded { .. })
    ));

    let released = usage.release_cpu(5);
    assert_eq!(released, 5);
    assert!(enforcer.check_cpu("payments", usage.cpu_cores_used).is_ok());

    let released_mem = usage.release_memory(2048);
    assert_eq!(released_mem, 2048);
    assert!(enforcer.check_memory("payments", usage.memory_bytes_used).is_ok());
}

#[test]
fn integration_overflow_saturate_at_u64_max_preserves_enforcement() {
    let mut usage = QuotaUsage::new();
    usage.add_cpu(u64::MAX);
    usage.add_memory(u64::MAX);
    usage.add_disk(u64::MAX);

    assert_eq!(usage.cpu_cores_used, u64::MAX);
    assert_eq!(usage.memory_bytes_used, u64::MAX);
    assert_eq!(usage.disk_bytes_used, u64::MAX);

    let enforcer = make_full_enforcer();
    assert!(matches!(
        enforcer.check_cpu("payments", usage.cpu_cores_used),
        Err(QuotaError::QuotaExceeded { .. })
    ));
    assert!(matches!(
        enforcer.check_memory("payments", usage.memory_bytes_used),
        Err(QuotaError::QuotaExceeded { .. })
    ));
    assert!(matches!(
        enforcer.check_disk("payments", usage.disk_bytes_used),
        Err(QuotaError::QuotaExceeded { .. })
    ));
}
