use super::*;
use crate::resource_quota::{
    CpuQuota, DiskQuota, MemoryQuota, NamespaceQuota, OvercommitPolicy, QuotaEnforcer, QuotaError,
};
use std::num::NonZeroU64;

fn make_test_enforcer() -> QuotaEnforcer {
    let mut registry = super::enforcer::NamespaceRegistry::new();
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
    let mut registry = super::enforcer::NamespaceRegistry::new();
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
fn red_queen_u64_max_requested_memory_with_overcommit_should_succeed() {
    let enforcer = make_overcommit_enforcer();
    let result = enforcer.check_memory("payments", u64::MAX);
    assert!(result.is_ok(), "u64::MAX with overcommit should be allowed");
}

#[test]
fn red_queen_u64_max_minus_one_requested_memory_with_overcommit_should_succeed() {
    let enforcer = make_overcommit_enforcer();
    let result = enforcer.check_memory("payments", u64::MAX - 1);
    assert!(
        result.is_ok(),
        "u64::MAX-1 with overcommit should be allowed"
    );
}

#[test]
fn red_queen_u64_max_requested_disk_with_overcommit_should_succeed() {
    let enforcer = make_overcommit_enforcer();
    let result = enforcer.check_disk("payments", u64::MAX);
    assert!(
        result.is_ok(),
        "u64::MAX disk with overcommit should be allowed"
    );
}

#[test]
fn red_queen_cpu_overcommit_does_not_affect_memory() {
    let mut registry = super::enforcer::NamespaceRegistry::new();
    let _ = registry.register(
        NamespaceQuota::new("payments")
            .with_cpu(CpuQuota::new(NonZeroU64::new(1).unwrap()))
            .with_memory(MemoryQuota::new(NonZeroU64::new(1024).unwrap()))
            .with_overcommit(OvercommitPolicy::AllowOvercommit),
    );
    let enforcer = QuotaEnforcer::new(registry);

    assert!(enforcer.check_cpu("payments", 100).is_ok());
    assert!(enforcer.check_memory("payments", 2048).is_err());
}

#[test]
fn red_queen_memory_overcommit_does_not_affect_disk() {
    let mut registry = super::enforcer::NamespaceRegistry::new();
    let _ = registry.register(
        NamespaceQuota::new("payments")
            .with_memory(MemoryQuota::new(NonZeroU64::new(1024).unwrap()))
            .with_disk(DiskQuota::new(NonZeroU64::new(10_000).unwrap()))
            .with_overcommit(OvercommitPolicy::AllowOvercommit),
    );
    let enforcer = QuotaEnforcer::new(registry);

    assert!(enforcer.check_memory("payments", u64::MAX).is_ok());
    assert!(enforcer.check_disk("payments", 20_000).is_err());
}

#[test]
fn red_queen_exactly_at_cpu_limit_should_succeed() {
    let enforcer = make_test_enforcer();
    assert!(enforcer.check_cpu("payments", 4).is_ok());
}

#[test]
fn red_queen_exactly_at_memory_limit_should_succeed() {
    let enforcer = make_test_enforcer();
    assert!(enforcer.check_memory("payments", 1024).is_ok());
}

#[test]
fn red_queen_exactly_at_disk_limit_should_succeed() {
    let enforcer = make_test_enforcer();
    assert!(enforcer.check_disk("payments", 10_000).is_ok());
}

#[test]
fn red_queen_one_over_cpu_limit_should_fail() {
    let enforcer = make_test_enforcer();
    let result = enforcer.check_cpu("payments", 5);
    assert!(result.is_err());
    if let Err(QuotaError::QuotaExceeded { available, .. }) = result {
        assert_eq!(available, 4);
    } else {
        panic!("Expected QuotaExceeded");
    }
}

#[test]
fn red_queen_one_over_memory_limit_should_fail() {
    let enforcer = make_test_enforcer();
    let result = enforcer.check_memory("payments", 1025);
    assert!(result.is_err());
    if let Err(QuotaError::QuotaExceeded { available, .. }) = result {
        assert_eq!(available, 1024);
    } else {
        panic!("Expected QuotaExceeded");
    }
}

#[test]
fn red_queen_one_over_disk_limit_should_fail() {
    let enforcer = make_test_enforcer();
    let result = enforcer.check_disk("payments", 10_001);
    assert!(result.is_err());
    if let Err(QuotaError::QuotaExceeded { available, .. }) = result {
        assert_eq!(available, 10_000);
    } else {
        panic!("Expected QuotaExceeded");
    }
}

#[test]
fn red_queen_zero_requested_all_resources_should_succeed() {
    let enforcer = make_test_enforcer();
    assert!(enforcer.check_cpu("payments", 0).is_ok());
    assert!(enforcer.check_memory("payments", 0).is_ok());
    assert!(enforcer.check_disk("payments", 0).is_ok());
}

#[test]
fn red_queen_unknown_namespace_returns_not_found() {
    let enforcer = make_test_enforcer();
    let result = enforcer.check_cpu("nonexistent", 1);
    assert!(matches!(result, Err(QuotaError::NamespaceNotFound(_))));
}

#[test]
fn red_queen_missing_cpu_quota_returns_not_configured() {
    let mut registry = super::enforcer::NamespaceRegistry::new();
    let _ = registry.register(
        NamespaceQuota::new("mem-only")
            .with_memory(MemoryQuota::new(NonZeroU64::new(1024).unwrap())),
    );
    let enforcer = QuotaEnforcer::new(registry);
    let result = enforcer.check_cpu("mem-only", 1);
    assert!(matches!(
        result,
        Err(QuotaError::QuotaNotConfigured {
            resource: ResourceKind::Cpu,
            ..
        })
    ));
}

#[test]
fn red_queen_missing_memory_quota_returns_not_configured() {
    let mut registry = super::enforcer::NamespaceRegistry::new();
    let _ = registry.register(
        NamespaceQuota::new("cpu-only").with_cpu(CpuQuota::new(NonZeroU64::new(4).unwrap())),
    );
    let enforcer = QuotaEnforcer::new(registry);
    let result = enforcer.check_memory("cpu-only", 1024);
    assert!(matches!(
        result,
        Err(QuotaError::QuotaNotConfigured {
            resource: ResourceKind::Memory,
            ..
        })
    ));
}

#[test]
fn red_queen_missing_disk_quota_returns_not_configured() {
    let mut registry = super::enforcer::NamespaceRegistry::new();
    let _ = registry.register(
        NamespaceQuota::new("cpu-only").with_cpu(CpuQuota::new(NonZeroU64::new(4).unwrap())),
    );
    let enforcer = QuotaEnforcer::new(registry);
    let result = enforcer.check_disk("cpu-only", 100);
    assert!(matches!(
        result,
        Err(QuotaError::QuotaNotConfigured {
            resource: ResourceKind::Disk,
            ..
        })
    ));
}

#[test]
fn red_queen_namespace_case_sensitivity_matters() {
    let mut registry = super::enforcer::NamespaceRegistry::new();
    let _ = registry.register(
        NamespaceQuota::new("Payments").with_cpu(CpuQuota::new(NonZeroU64::new(4).unwrap())),
    );
    let enforcer = QuotaEnforcer::new(registry);
    assert!(enforcer.check_cpu("Payments", 4).is_ok());
    assert!(enforcer.check_cpu("payments", 4).is_err());
}

#[test]
fn red_queen_unicode_namespace_names_allowed() {
    let mut registry = super::enforcer::NamespaceRegistry::new();
    let _ = registry.register(
        NamespaceQuota::new("payments-日本語").with_cpu(CpuQuota::new(NonZeroU64::new(4).unwrap())),
    );
    let enforcer = QuotaEnforcer::new(registry);
    assert!(enforcer.check_cpu("payments-日本語", 4).is_ok());
    assert!(enforcer.check_cpu("payments", 4).is_err());
}

#[test]
fn red_queen_empty_namespace_differs_from_default() {
    let mut registry = super::enforcer::NamespaceRegistry::new();
    let _ = registry
        .register(NamespaceQuota::new("").with_cpu(CpuQuota::new(NonZeroU64::new(4).unwrap())));
    let enforcer = QuotaEnforcer::new(registry);
    assert!(enforcer.check_cpu("", 4).is_ok());
    assert!(enforcer.check_cpu("default", 4).is_err());
}

#[test]
fn red_queen_registry_replace_namespace_updates_quota() {
    let mut registry = super::enforcer::NamespaceRegistry::new();
    let _ = registry.register(
        NamespaceQuota::new("payments").with_cpu(CpuQuota::new(NonZeroU64::new(2).unwrap())),
    );
    let _ = registry.register(
        NamespaceQuota::new("payments").with_cpu(CpuQuota::new(NonZeroU64::new(8).unwrap())),
    );
    let enforcer = QuotaEnforcer::new(registry);
    assert!(enforcer.check_cpu("payments", 8).is_ok());
    assert!(enforcer.check_cpu("payments", 9).is_err());
}

#[test]
fn red_queen_removing_namespace_then_checking_returns_not_found() {
    let mut registry = super::enforcer::NamespaceRegistry::new();
    let _ = registry
        .register(NamespaceQuota::new("temp").with_cpu(CpuQuota::new(NonZeroU64::new(4).unwrap())));
    let mut enforcer = QuotaEnforcer::new(registry);
    assert!(enforcer.check_cpu("temp", 4).is_ok());
    enforcer.registry_mut().remove("temp");
    let result = enforcer.check_cpu("temp", 4);
    assert!(matches!(result, Err(QuotaError::NamespaceNotFound(_))));
}

#[test]
fn red_queen_quota_error_display_includes_all_fields() {
    let err = QuotaError::QuotaExceeded {
        resource: ResourceKind::Cpu,
        namespace: "test-ns".to_string(),
        requested: 100,
        available: 50,
    };
    let display = format!("{}", err);
    assert!(display.contains("cpu"));
    assert!(display.contains("test-ns"));
    assert!(display.contains("100"));
    assert!(display.contains("50"));
}

#[test]
fn red_queen_namespace_not_found_error_display() {
    let err = QuotaError::NamespaceNotFound("ghost-ns".to_string());
    let display = format!("{}", err);
    assert!(display.contains("ghost-ns"));
}

#[test]
fn red_queen_quota_not_configured_error_display() {
    let err = QuotaError::QuotaNotConfigured {
        resource: ResourceKind::Memory,
        namespace: "no-mem-ns".to_string(),
    };
    let display = format!("{}", err);
    assert!(display.contains("memory"));
    assert!(display.contains("no-mem-ns"));
}

#[test]
fn red_queen_multiple_namespaces_isolated() {
    let mut registry = super::enforcer::NamespaceRegistry::new();
    let _ = registry
        .register(NamespaceQuota::new("ns1").with_cpu(CpuQuota::new(NonZeroU64::new(2).unwrap())));
    let _ = registry
        .register(NamespaceQuota::new("ns2").with_cpu(CpuQuota::new(NonZeroU64::new(8).unwrap())));
    let enforcer = QuotaEnforcer::new(registry);
    assert!(enforcer.check_cpu("ns1", 2).is_ok());
    assert!(enforcer.check_cpu("ns1", 3).is_err());
    assert!(enforcer.check_cpu("ns2", 8).is_ok());
    assert!(enforcer.check_cpu("ns2", 9).is_err());
}

#[test]
fn red_queen_is_overcommit_rejected_for_quota_exceeded() {
    let err = QuotaError::QuotaExceeded {
        resource: ResourceKind::Cpu,
        namespace: "test".to_string(),
        requested: 10,
        available: 5,
    };
    assert!(err.is_overcommit_rejected());
}

#[test]
fn red_queen_is_overcommit_rejected_for_quota_not_configured() {
    let err = QuotaError::QuotaNotConfigured {
        resource: ResourceKind::Memory,
        namespace: "test".to_string(),
    };
    assert!(err.is_overcommit_rejected());
}

#[test]
fn red_queen_is_not_overcommit_rejected_for_namespace_not_found() {
    let err = QuotaError::NamespaceNotFound("test".to_string());
    assert!(!err.is_overcommit_rejected());
}

#[test]
fn red_queen_quota_usage_builder_chain() {
    let usage = QuotaUsage::new()
        .with_cpu(4)
        .with_memory(8192)
        .with_disk(100_000);
    assert_eq!(usage.cpu_cores_used, 4);
    assert_eq!(usage.memory_bytes_used, 8192);
    assert_eq!(usage.disk_bytes_used, 100_000);
}

#[test]
fn red_queen_resource_kind_display() {
    assert_eq!(format!("{}", ResourceKind::Cpu), "cpu");
    assert_eq!(format!("{}", ResourceKind::Memory), "memory");
    assert_eq!(format!("{}", ResourceKind::Disk), "disk");
}

#[test]
fn red_queen_namespace_quota_builder_pattern() {
    let quota = NamespaceQuota::new("build-test")
        .with_cpu(CpuQuota::new(NonZeroU64::new(16).unwrap()))
        .with_memory(MemoryQuota::new(NonZeroU64::new(16384).unwrap()))
        .with_disk(DiskQuota::new(NonZeroU64::new(1_000_000).unwrap()))
        .with_overcommit(OvercommitPolicy::AllowOvercommit);
    assert_eq!(quota.namespace, "build-test");
    assert!(quota.cpu.is_some());
    assert!(quota.memory.is_some());
    assert!(quota.disk.is_some());
    assert_eq!(quota.overcommit, OvercommitPolicy::AllowOvercommit);
}

#[test]
fn red_queen_default_namespace_values() {
    let quota = NamespaceQuota::new("default-test");
    assert!(quota.cpu.is_none());
    assert!(quota.memory.is_none());
    assert!(quota.disk.is_none());
    assert_eq!(quota.overcommit, OvercommitPolicy::NoOvercommit);
}

#[test]
fn red_queen_quota_serialization_roundtrip() {
    let q = CpuQuota::new(NonZeroU64::new(8).unwrap());
    let json = serde_json::to_string(&q).unwrap();
    let q2: CpuQuota = serde_json::from_str(&json).unwrap();
    assert_eq!(q, q2);

    let m = MemoryQuota::new(NonZeroU64::new(4096).unwrap());
    let json = serde_json::to_string(&m).unwrap();
    let m2: MemoryQuota = serde_json::from_str(&json).unwrap();
    assert_eq!(m, m2);

    let d = DiskQuota::new(NonZeroU64::new(9999).unwrap());
    let json = serde_json::to_string(&d).unwrap();
    let d2: DiskQuota = serde_json::from_str(&json).unwrap();
    assert_eq!(d, d2);
}

#[test]
fn red_queen_namespace_quota_serialization_roundtrip() {
    let q1 = NamespaceQuota::new("ns1")
        .with_cpu(CpuQuota::new(NonZeroU64::new(2).unwrap()))
        .with_overcommit(OvercommitPolicy::AllowOvercommit);
    let json = serde_json::to_string(&q1).unwrap();
    let q2: NamespaceQuota = serde_json::from_str(&json).unwrap();
    assert_eq!(q1, q2);
}

#[test]
fn red_queen_resource_kind_serialization() {
    let cpu_json = serde_json::to_string(&ResourceKind::Cpu).unwrap();
    assert_eq!(cpu_json, "\"cpu\"");

    let mem_json = serde_json::to_string(&ResourceKind::Memory).unwrap();
    assert_eq!(mem_json, "\"memory\"");

    let disk_json = serde_json::to_string(&ResourceKind::Disk).unwrap();
    assert_eq!(disk_json, "\"disk\"");
}

#[test]
fn red_queen_overcommit_policy_serialization() {
    let no_json = serde_json::to_string(&OvercommitPolicy::NoOvercommit).unwrap();
    assert_eq!(no_json, "\"no_overcommit\"");

    let allow_json = serde_json::to_string(&OvercommitPolicy::AllowOvercommit).unwrap();
    assert_eq!(allow_json, "\"allow_overcommit\"");
}

#[test]
fn red_queen_registry_list_namespaces_returns_all() {
    let mut registry = super::enforcer::NamespaceRegistry::new();
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
fn red_queen_registry_remove_returns_previous() {
    let mut registry = super::enforcer::NamespaceRegistry::new();
    let quota =
        NamespaceQuota::new("to-remove").with_cpu(CpuQuota::new(NonZeroU64::new(4).unwrap()));
    let _ = registry.register(quota);
    let removed = registry.remove("to-remove");
    assert!(removed.is_some());
    assert!(registry.get("to-remove").is_none());
}

#[test]
fn red_queen_registry_remove_nonexistent_returns_none() {
    let mut registry = super::enforcer::NamespaceRegistry::new();
    let result = registry.remove("nonexistent");
    assert!(result.is_none());
}

#[test]
fn red_queen_default_enforcer_has_default_namespace() {
    let enforcer = QuotaEnforcer::default();
    assert!(enforcer.check_cpu("default", 1).is_ok());
}

#[test]
fn red_queen_quota_exceeded_error_contains_correct_requested_value() {
    let enforcer = make_test_enforcer();
    let result = enforcer.check_memory("payments", 9999);
    assert!(matches!(
        result,
        Err(QuotaError::QuotaExceeded {
            resource: ResourceKind::Memory,
            namespace,
            requested: 9999,
            available: 1024
        }) if namespace == "payments"
    ));
}

#[test]
fn red_queen_atomicity_register_does_not_leak_partial_state() {
    let mut registry = super::enforcer::NamespaceRegistry::new();
    let quota = NamespaceQuota::new("atomic-test")
        .with_cpu(CpuQuota::new(NonZeroU64::new(4).unwrap()))
        .with_memory(MemoryQuota::new(NonZeroU64::new(1024).unwrap()));
    let result = registry.register(quota);
    assert!(result.is_ok());
    let retrieved = registry.get("atomic-test").unwrap();
    assert!(retrieved.cpu.is_some());
    assert!(retrieved.memory.is_some());
}

#[test]
fn red_queen_quota_enforcer_exposes_registry() {
    let enforcer = QuotaEnforcer::with_default_namespace();
    assert!(enforcer.registry().get("default").is_some());
}

#[test]
fn red_queen_cpu_requested_zero_with_no_overcommit_still_ok() {
    let enforcer = make_test_enforcer();
    let result = enforcer.check_cpu("payments", 0);
    assert!(result.is_ok());
}

#[test]
fn red_queen_large_but_valid_cpu_request() {
    let enforcer = make_test_enforcer();
    let result = enforcer.check_cpu("payments", 1_000_000);
    assert!(result.is_err());
}

#[test]
fn red_queen_quota_not_configured_error_fields_correct() {
    let enforcer = make_test_enforcer();
    let result = enforcer.check_disk("payments", 1);
    match result {
        Err(QuotaError::QuotaNotConfigured {
            resource,
            namespace,
        }) => {
            assert_eq!(resource, ResourceKind::Disk);
            assert_eq!(namespace, "payments");
        }
        _ => panic!("Expected QuotaNotConfigured"),
    }
}
