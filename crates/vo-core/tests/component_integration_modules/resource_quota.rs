use vo_core::resource_quota::{
    CpuQuota, DiskQuota, MemoryQuota, NamespaceQuota, NamespaceRegistry, OvercommitPolicy,
    QuotaEnforcer, QuotaUsage, ResourceKind,
};

#[test]
fn resource_quota_usage_tracking() {
    let mut enforcer = QuotaEnforcer::with_default_namespace();

    let quota = NamespaceQuota::new("payments")
        .with_cpu(CpuQuota::new(std::num::NonZeroU64::new(8).expect("non-zero")))
        .with_memory(MemoryQuota::new(
            std::num::NonZeroU64::new(4096).expect("non-zero"),
        ))
        .with_disk(DiskQuota::new(std::num::NonZeroU64::new(10240).expect("non-zero")));

    enforcer
        .registry_mut()
        .register(quota)
        .expect("quota registration should succeed");

    let usage = QuotaUsage::new()
        .with_cpu(4)
        .with_memory(2048)
        .with_disk(5120);

    let cpu_result = enforcer.check_cpu("payments", usage.cpu_cores_used);
    assert!(cpu_result.is_ok(), "cpu usage within quota should pass");

    let mem_result = enforcer.check_memory("payments", usage.memory_bytes_used);
    assert!(mem_result.is_ok(), "memory usage within quota should pass");

    let disk_result = enforcer.check_disk("payments", usage.disk_bytes_used);
    assert!(disk_result.is_ok(), "disk usage within quota should pass");

    let over_limit_usage = QuotaUsage::new().with_cpu(16).with_memory(8192);

    let cpu_over = enforcer.check_cpu("payments", over_limit_usage.cpu_cores_used);
    assert!(cpu_over.is_err(), "cpu usage over quota should fail");

    let mem_over = enforcer.check_memory("payments", over_limit_usage.memory_bytes_used);
    assert!(mem_over.is_err(), "memory usage over quota should fail");
}

#[test]
fn quota_enforcer_default_namespace_works() {
    let enforcer = QuotaEnforcer::with_default_namespace();

    let result = enforcer.check_cpu("default", 0);
    assert!(result.is_ok(), "zero cpu check should always pass");

    let result = enforcer.check_memory("default", u64::MAX);
    assert!(result.is_err(), "max memory should exceed default quota");
}

#[test]
fn namespace_quota_isolation_between_namespaces() {
    let mut enforcer = QuotaEnforcer::with_default_namespace();

    let payments_quota = NamespaceQuota::new("payments")
        .with_cpu(CpuQuota::new(std::num::NonZeroU64::new(2).expect("non-zero")))
        .with_overcommit(OvercommitPolicy::NoOvercommit);

    let analytics_quota = NamespaceQuota::new("analytics")
        .with_cpu(CpuQuota::new(std::num::NonZeroU64::new(8).expect("non-zero")))
        .with_overcommit(OvercommitPolicy::NoOvercommit);

    enforcer
        .registry_mut()
        .register(payments_quota)
        .expect("payments quota registration should succeed");
    enforcer
        .registry_mut()
        .register(analytics_quota)
        .expect("analytics quota registration should succeed");

    let payments_result = enforcer.check_cpu("payments", 2);
    assert!(payments_result.is_ok(), "payments at limit should pass");

    let analytics_result = enforcer.check_cpu("analytics", 4);
    assert!(
        analytics_result.is_ok(),
        "analytics well under limit should pass"
    );

    let payments_over = enforcer.check_cpu("payments", 3);
    assert!(payments_over.is_err(), "payments over limit should fail");

    let unknown_result = enforcer.check_cpu("unknown-ns", 1);
    assert!(
        unknown_result.is_err(),
        "unknown namespace should return NamespaceNotFound error"
    );
}

#[test]
fn resource_kind_serialization_roundtrip() {
    let kinds = [ResourceKind::Cpu, ResourceKind::Memory, ResourceKind::Disk];

    for kind in kinds {
        let json = serde_json::to_string(&kind).expect("serialization should succeed");
        let parsed: ResourceKind =
            serde_json::from_str(&json).expect("deserialization should succeed");
        assert_eq!(
            parsed, kind,
            "ResourceKind {:?} should round-trip through JSON",
            kind
        );
    }

    assert_eq!(ResourceKind::Cpu.as_str(), "cpu");
    assert_eq!(ResourceKind::Memory.as_str(), "memory");
    assert_eq!(ResourceKind::Disk.as_str(), "disk");
}

#[test]
fn overcommit_policy_default_is_no_overcommit() {
    let policy = OvercommitPolicy::default();
    assert_eq!(
        policy,
        OvercommitPolicy::NoOvercommit,
        "default overcommit policy should be NoOvercommit"
    );
}

#[test]
fn overcommit_policy_variants() {
    let policies = [
        OvercommitPolicy::NoOvercommit,
        OvercommitPolicy::AllowOvercommit,
    ];

    assert_eq!(
        policies.len(),
        2,
        "should have exactly 2 overcommit policy variants"
    );

    for policy in policies {
        let json = serde_json::to_string(&policy).expect("serialization should succeed");
        let parsed: OvercommitPolicy =
            serde_json::from_str(&json).expect("deserialization should succeed");
        assert_eq!(parsed, policy, "OvercommitPolicy should round-trip");
    }
}

#[test]
fn quota_usage_builder_pattern() {
    let usage = QuotaUsage::new()
        .with_cpu(4)
        .with_memory(2048)
        .with_disk(4096);

    assert_eq!(usage.cpu_cores_used, 4);
    assert_eq!(usage.memory_bytes_used, 2048);
    assert_eq!(usage.disk_bytes_used, 4096);
}

#[test]
fn quota_usage_default_is_zero() {
    let usage = QuotaUsage::new();
    assert_eq!(usage.cpu_cores_used, 0);
    assert_eq!(usage.memory_bytes_used, 0);
    assert_eq!(usage.disk_bytes_used, 0);
}

#[test]
fn namespace_registry_registration_and_lookup() {
    let mut registry = NamespaceRegistry::new();

    let quota = NamespaceQuota::new("test-ns")
        .with_cpu(CpuQuota::new(std::num::NonZeroU64::new(4).expect("non-zero")));

    let result = registry.register(quota.clone());
    assert!(result.is_ok(), "quota registration should succeed");

    let looked_up = registry.get("test-ns");
    assert!(
        looked_up.is_some(),
        "registered namespace should be findable"
    );
    assert_eq!(
        looked_up.map(|q| q.namespace.as_str()),
        Some("test-ns"),
        "namespace name should match"
    );

    let not_found = registry.get("unknown-ns");
    assert!(not_found.is_none(), "unknown namespace should not be found");
}

#[test]
fn namespace_registry_duplicate_registration_fails() {
    let mut registry = NamespaceRegistry::new();

    let quota = NamespaceQuota::new("dup-ns")
        .with_cpu(CpuQuota::new(std::num::NonZeroU64::new(4).expect("non-zero")));

    let result1 = registry.register(quota.clone());
    assert!(result1.is_ok(), "first registration should succeed");

    let result2 = registry.register(quota);
    assert!(
        result2.is_ok(),
        "duplicate registration replaces existing (idempotent)"
    );

    let retrieved = registry.get("dup-ns").expect("namespace should exist");
    assert_eq!(
        retrieved.cpu.unwrap().max_cores.get(),
        4,
        "retrieved quota should be the new one"
    );
}
