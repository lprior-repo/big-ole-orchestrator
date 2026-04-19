use crate::resource_quota::{
    CpuQuota, DiskQuota, MemoryQuota, NamespaceQuota, OvercommitPolicy, QuotaEnforcer, QuotaError,
    ResourceKind,
};
use std::num::NonZeroU64;

fn make_multi_tenant_enforcer() -> QuotaEnforcer {
    let mut registry = crate::resource_quota::NamespaceRegistry::new();
    let _ = registry.register(
        NamespaceQuota::new("tenant-a")
            .with_cpu(CpuQuota::new(NonZeroU64::new(4).unwrap()))
            .with_memory(MemoryQuota::new(NonZeroU64::new(1024).unwrap()))
            .with_disk(DiskQuota::new(NonZeroU64::new(10_000).unwrap()))
            .with_overcommit(OvercommitPolicy::NoOvercommit),
    );
    let _ = registry.register(
        NamespaceQuota::new("tenant-b")
            .with_cpu(CpuQuota::new(NonZeroU64::new(8).unwrap()))
            .with_memory(MemoryQuota::new(NonZeroU64::new(4096).unwrap()))
            .with_disk(DiskQuota::new(NonZeroU64::new(50_000).unwrap()))
            .with_overcommit(OvercommitPolicy::NoOvercommit),
    );
    let _ = registry.register(
        NamespaceQuota::new("tenant-c")
            .with_cpu(CpuQuota::new(NonZeroU64::new(2).unwrap()))
            .with_memory(MemoryQuota::new(NonZeroU64::new(512).unwrap()))
            .with_disk(DiskQuota::new(NonZeroU64::new(5_000).unwrap()))
            .with_overcommit(OvercommitPolicy::NoOvercommit),
    );
    QuotaEnforcer::new(registry)
}

#[test]
fn tenant_isolation_cpu_exhaustion_does_not_affect_other_tenants() {
    let enforcer = make_multi_tenant_enforcer();

    assert!(matches!(
        enforcer.check_cpu("tenant-a", 5),
        Err(QuotaError::QuotaExceeded { .. })
    ));

    assert!(enforcer.check_cpu("tenant-b", 8).is_ok());
    assert!(enforcer.check_cpu("tenant-c", 2).is_ok());
}

#[test]
fn tenant_isolation_memory_exhaustion_does_not_affect_other_tenants() {
    let enforcer = make_multi_tenant_enforcer();

    assert!(matches!(
        enforcer.check_memory("tenant-a", 1025),
        Err(QuotaError::QuotaExceeded { .. })
    ));

    assert!(enforcer.check_memory("tenant-b", 4096).is_ok());
    assert!(enforcer.check_memory("tenant-c", 512).is_ok());
}

#[test]
fn tenant_isolation_disk_exhaustion_does_not_affect_other_tenants() {
    let enforcer = make_multi_tenant_enforcer();

    assert!(matches!(
        enforcer.check_disk("tenant-a", 10_001),
        Err(QuotaError::QuotaExceeded { .. })
    ));

    assert!(enforcer.check_disk("tenant-b", 50_000).is_ok());
    assert!(enforcer.check_disk("tenant-c", 5_000).is_ok());
}

#[test]
fn tenant_isolation_all_resources_exhausted_one_tenant() {
    let enforcer = make_multi_tenant_enforcer();

    assert!(matches!(
        enforcer.check_cpu("tenant-c", 3),
        Err(QuotaError::QuotaExceeded { .. })
    ));
    assert!(matches!(
        enforcer.check_memory("tenant-c", 513),
        Err(QuotaError::QuotaExceeded { .. })
    ));
    assert!(matches!(
        enforcer.check_disk("tenant-c", 5_001),
        Err(QuotaError::QuotaExceeded { .. })
    ));

    assert!(enforcer.check_cpu("tenant-a", 4).is_ok());
    assert!(enforcer.check_memory("tenant-a", 1024).is_ok());
    assert!(enforcer.check_disk("tenant-a", 10_000).is_ok());
    assert!(enforcer.check_cpu("tenant-b", 8).is_ok());
    assert!(enforcer.check_memory("tenant-b", 4096).is_ok());
    assert!(enforcer.check_disk("tenant-b", 50_000).is_ok());
}

#[test]
fn tenant_isolation_different_quotas_provide_fair_sharing() {
    let enforcer = make_multi_tenant_enforcer();

    assert!(enforcer.check_cpu("tenant-a", 4).is_ok());
    assert!(matches!(
        enforcer.check_cpu("tenant-a", 5),
        Err(QuotaError::QuotaExceeded { .. })
    ));

    assert!(enforcer.check_cpu("tenant-b", 8).is_ok());
    assert!(matches!(
        enforcer.check_cpu("tenant-b", 9),
        Err(QuotaError::QuotaExceeded { .. })
    ));

    assert!(enforcer.check_cpu("tenant-c", 2).is_ok());
    assert!(matches!(
        enforcer.check_cpu("tenant-c", 3),
        Err(QuotaError::QuotaExceeded { .. })
    ));
}

#[test]
fn tenant_isolation_memory_fair_sharing_proportional() {
    let enforcer = make_multi_tenant_enforcer();

    assert!(enforcer.check_memory("tenant-a", 1024).is_ok());
    assert!(enforcer.check_memory("tenant-b", 4096).is_ok());
    assert!(enforcer.check_memory("tenant-c", 512).is_ok());

    assert!(matches!(
        enforcer.check_memory("tenant-a", 1025),
        Err(QuotaError::QuotaExceeded { .. })
    ));
    assert!(matches!(
        enforcer.check_memory("tenant-b", 4097),
        Err(QuotaError::QuotaExceeded { .. })
    ));
    assert!(matches!(
        enforcer.check_memory("tenant-c", 513),
        Err(QuotaError::QuotaExceeded { .. })
    ));
}

#[test]
fn tenant_isolation_disk_fair_sharing_proportional() {
    let enforcer = make_multi_tenant_enforcer();

    assert!(enforcer.check_disk("tenant-a", 10_000).is_ok());
    assert!(enforcer.check_disk("tenant-b", 50_000).is_ok());
    assert!(enforcer.check_disk("tenant-c", 5_000).is_ok());

    assert!(matches!(
        enforcer.check_disk("tenant-a", 10_001),
        Err(QuotaError::QuotaExceeded { .. })
    ));
    assert!(matches!(
        enforcer.check_disk("tenant-b", 50_001),
        Err(QuotaError::QuotaExceeded { .. })
    ));
    assert!(matches!(
        enforcer.check_disk("tenant-c", 5_001),
        Err(QuotaError::QuotaExceeded { .. })
    ));
}

#[test]
fn tenant_isolation_registering_new_tenant_does_not_affect_existing() {
    let mut enforcer = make_multi_tenant_enforcer();

    assert!(enforcer.check_cpu("tenant-a", 4).is_ok());
    assert!(enforcer.check_cpu("tenant-b", 8).is_ok());

    enforcer
        .registry_mut()
        .register(
            NamespaceQuota::new("tenant-d")
                .with_cpu(CpuQuota::new(NonZeroU64::new(16).unwrap()))
                .with_memory(MemoryQuota::new(NonZeroU64::new(8192).unwrap()))
                .with_disk(DiskQuota::new(NonZeroU64::new(100_000).unwrap())),
        )
        .expect("register should succeed");

    assert!(enforcer.check_cpu("tenant-a", 4).is_ok());
    assert!(matches!(
        enforcer.check_cpu("tenant-a", 5),
        Err(QuotaError::QuotaExceeded { .. })
    ));
    assert!(enforcer.check_cpu("tenant-b", 8).is_ok());
    assert!(enforcer.check_cpu("tenant-d", 16).is_ok());
    assert!(matches!(
        enforcer.check_cpu("tenant-d", 17),
        Err(QuotaError::QuotaExceeded { .. })
    ));
}

#[test]
fn tenant_isolation_removing_tenant_does_not_affect_others() {
    let mut enforcer = make_multi_tenant_enforcer();

    assert!(enforcer.check_cpu("tenant-b", 8).is_ok());

    enforcer.registry_mut().remove("tenant-a");

    assert!(matches!(
        enforcer.check_cpu("tenant-a", 1),
        Err(QuotaError::NamespaceNotFound(_))
    ));
    assert!(enforcer.check_cpu("tenant-b", 8).is_ok());
    assert!(enforcer.check_cpu("tenant-c", 2).is_ok());
}

#[test]
fn tenant_isolation_updating_one_tenant_quota_does_not_affect_others() {
    let mut enforcer = make_multi_tenant_enforcer();

    enforcer.registry_mut().register(
        NamespaceQuota::new("tenant-a")
            .with_cpu(CpuQuota::new(NonZeroU64::new(32).unwrap()))
            .with_memory(MemoryQuota::new(NonZeroU64::new(32768).unwrap()))
            .with_disk(DiskQuota::new(NonZeroU64::new(200_000).unwrap()))
            .with_overcommit(OvercommitPolicy::NoOvercommit),
    ).expect("register should succeed");

    assert!(enforcer.check_cpu("tenant-a", 32).is_ok());
    assert!(enforcer.check_memory("tenant-a", 32768).is_ok());
    assert!(enforcer.check_disk("tenant-a", 200_000).is_ok());

    assert!(matches!(
        enforcer.check_cpu("tenant-b", 9),
        Err(QuotaError::QuotaExceeded { .. })
    ));
    assert!(matches!(
        enforcer.check_memory("tenant-b", 4097),
        Err(QuotaError::QuotaExceeded { .. })
    ));
    assert!(matches!(
        enforcer.check_disk("tenant-b", 50_001),
        Err(QuotaError::QuotaExceeded { .. })
    ));

    assert!(matches!(
        enforcer.check_cpu("tenant-c", 3),
        Err(QuotaError::QuotaExceeded { .. })
    ));
}

#[test]
fn tenant_isolation_overcommit_policy_isolated_per_tenant() {
    let mut registry = crate::resource_quota::NamespaceRegistry::new();
    let _ = registry.register(
        NamespaceQuota::new("overcommit-tenant")
            .with_cpu(CpuQuota::new(NonZeroU64::new(1).unwrap()))
            .with_memory(MemoryQuota::new(NonZeroU64::new(1).unwrap()))
            .with_disk(DiskQuota::new(NonZeroU64::new(1).unwrap()))
            .with_overcommit(OvercommitPolicy::AllowOvercommit),
    );
    let _ = registry.register(
        NamespaceQuota::new("strict-tenant")
            .with_cpu(CpuQuota::new(NonZeroU64::new(1).unwrap()))
            .with_memory(MemoryQuota::new(NonZeroU64::new(1).unwrap()))
            .with_disk(DiskQuota::new(NonZeroU64::new(1).unwrap()))
            .with_overcommit(OvercommitPolicy::NoOvercommit),
    );
    let enforcer = QuotaEnforcer::new(registry);

    assert!(enforcer.check_cpu("overcommit-tenant", 1000).is_ok());
    assert!(enforcer.check_memory("overcommit-tenant", 1000).is_ok());
    assert!(enforcer.check_disk("overcommit-tenant", 1000).is_ok());

    assert!(matches!(
        enforcer.check_cpu("strict-tenant", 2),
        Err(QuotaError::QuotaExceeded { .. })
    ));
    assert!(matches!(
        enforcer.check_memory("strict-tenant", 2),
        Err(QuotaError::QuotaExceeded { .. })
    ));
    assert!(matches!(
        enforcer.check_disk("strict-tenant", 2),
        Err(QuotaError::QuotaExceeded { .. })
    ));
}

#[test]
fn tenant_isolation_repeated_checks_do_not_accumulate() {
    let enforcer = make_multi_tenant_enforcer();

    for _ in 0..100 {
        assert!(enforcer.check_cpu("tenant-a", 4).is_ok());
    }

    assert!(matches!(
        enforcer.check_cpu("tenant-a", 5),
        Err(QuotaError::QuotaExceeded { .. })
    ));
}

#[test]
fn tenant_isolation_error_reports_correct_namespace() {
    let enforcer = make_multi_tenant_enforcer();

    let err = enforcer.check_cpu("tenant-b", 100).unwrap_err();
    match err {
        QuotaError::QuotaExceeded {
            resource,
            namespace,
            requested,
            available,
        } => {
            assert_eq!(resource, ResourceKind::Cpu);
            assert_eq!(namespace, "tenant-b");
            assert_eq!(requested, 100);
            assert_eq!(available, 8);
        }
        _ => panic!("Expected QuotaExceeded, got {:?}", err),
    }

    let err2 = enforcer.check_memory("tenant-c", 999).unwrap_err();
    match err2 {
        QuotaError::QuotaExceeded {
            resource,
            namespace,
            ..
        } => {
            assert_eq!(resource, ResourceKind::Memory);
            assert_eq!(namespace, "tenant-c");
        }
        _ => panic!("Expected QuotaExceeded, got {:?}", err2),
    }
}

#[test]
fn tenant_isolation_ten_many_tenants_exhaustion() {
    let mut registry = crate::resource_quota::NamespaceRegistry::new();
    for i in 0..50 {
        let ns = format!("tenant-{:03}", i);
        let _ = registry.register(
            NamespaceQuota::new(&ns)
                .with_cpu(CpuQuota::new(NonZeroU64::new(1).unwrap()))
                .with_memory(MemoryQuota::new(NonZeroU64::new(100).unwrap()))
                .with_disk(DiskQuota::new(NonZeroU64::new(1000).unwrap())),
        );
    }
    let enforcer = QuotaEnforcer::new(registry);

    for i in 0..50 {
        let ns = format!("tenant-{:03}", i);
        assert!(enforcer.check_cpu(&ns, 1).is_ok(), "{} cpu at limit", ns);
        assert!(
            enforcer.check_memory(&ns, 100).is_ok(),
            "{} memory at limit",
            ns
        );
        assert!(enforcer.check_disk(&ns, 1000).is_ok(), "{} disk at limit", ns);
    }

    for i in 0..50 {
        let ns = format!("tenant-{:03}", i);
        assert!(
            matches!(
                enforcer.check_cpu(&ns, 2),
                Err(QuotaError::QuotaExceeded { .. })
            ),
            "{} cpu over limit",
            ns
        );
        assert!(
            matches!(
                enforcer.check_memory(&ns, 101),
                Err(QuotaError::QuotaExceeded { .. })
            ),
            "{} memory over limit",
            ns
        );
        assert!(
            matches!(
                enforcer.check_disk(&ns, 1001),
                Err(QuotaError::QuotaExceeded { .. })
            ),
            "{} disk over limit",
            ns
        );
    }
}

#[test]
fn tenant_isolation_partial_config_tenants() {
    let mut registry = crate::resource_quota::NamespaceRegistry::new();
    let _ = registry.register(
        NamespaceQuota::new("cpu-only").with_cpu(CpuQuota::new(NonZeroU64::new(4).unwrap())),
    );
    let _ = registry.register(
        NamespaceQuota::new("mem-only").with_memory(MemoryQuota::new(NonZeroU64::new(1024).unwrap())),
    );
    let _ = registry.register(
        NamespaceQuota::new("disk-only").with_disk(DiskQuota::new(NonZeroU64::new(5000).unwrap())),
    );
    let _ = registry.register(
        NamespaceQuota::new("full")
            .with_cpu(CpuQuota::new(NonZeroU64::new(8).unwrap()))
            .with_memory(MemoryQuota::new(NonZeroU64::new(2048).unwrap()))
            .with_disk(DiskQuota::new(NonZeroU64::new(10_000).unwrap())),
    );
    let enforcer = QuotaEnforcer::new(registry);

    assert!(enforcer.check_cpu("cpu-only", 4).is_ok());
    assert!(matches!(
        enforcer.check_memory("cpu-only", 1),
        Err(QuotaError::QuotaNotConfigured { .. })
    ));

    assert!(enforcer.check_memory("mem-only", 1024).is_ok());
    assert!(matches!(
        enforcer.check_cpu("mem-only", 1),
        Err(QuotaError::QuotaNotConfigured { .. })
    ));

    assert!(enforcer.check_disk("disk-only", 5000).is_ok());
    assert!(matches!(
        enforcer.check_cpu("disk-only", 1),
        Err(QuotaError::QuotaNotConfigured { .. })
    ));

    assert!(enforcer.check_cpu("full", 8).is_ok());
    assert!(enforcer.check_memory("full", 2048).is_ok());
    assert!(enforcer.check_disk("full", 10_000).is_ok());
}

#[test]
fn tenant_isolation_zero_request_all_tenants() {
    let enforcer = make_multi_tenant_enforcer();

    assert!(enforcer.check_cpu("tenant-a", 0).is_ok());
    assert!(enforcer.check_cpu("tenant-b", 0).is_ok());
    assert!(enforcer.check_cpu("tenant-c", 0).is_ok());
    assert!(enforcer.check_memory("tenant-a", 0).is_ok());
    assert!(enforcer.check_memory("tenant-b", 0).is_ok());
    assert!(enforcer.check_memory("tenant-c", 0).is_ok());
    assert!(enforcer.check_disk("tenant-a", 0).is_ok());
    assert!(enforcer.check_disk("tenant-b", 0).is_ok());
    assert!(enforcer.check_disk("tenant-c", 0).is_ok());
}

#[test]
fn tenant_isolation_boundary_one_over_each_tenant() {
    let enforcer = make_multi_tenant_enforcer();

    assert!(matches!(
        enforcer.check_cpu("tenant-a", 5),
        Err(QuotaError::QuotaExceeded {
            resource: ResourceKind::Cpu,
            namespace: ref ns,
            requested: 5,
            available: 4
        }) if ns == "tenant-a"
    ));
    assert!(matches!(
        enforcer.check_memory("tenant-b", 4097),
        Err(QuotaError::QuotaExceeded {
            resource: ResourceKind::Memory,
            namespace: ref ns,
            requested: 4097,
            available: 4096
        }) if ns == "tenant-b"
    ));
    assert!(matches!(
        enforcer.check_disk("tenant-c", 5001),
        Err(QuotaError::QuotaExceeded {
            resource: ResourceKind::Disk,
            namespace: ref ns,
            requested: 5001,
            available: 5000
        }) if ns == "tenant-c"
    ));
}

#[test]
fn tenant_isolation_unknown_tenant_does_not_affect_known() {
    let enforcer = make_multi_tenant_enforcer();

    assert!(matches!(
        enforcer.check_cpu("unknown-tenant", 1),
        Err(QuotaError::NamespaceNotFound(_))
    ));

    assert!(enforcer.check_cpu("tenant-a", 4).is_ok());
    assert!(enforcer.check_cpu("tenant-b", 8).is_ok());
    assert!(enforcer.check_cpu("tenant-c", 2).is_ok());
}

#[test]
fn tenant_isolation_sequential_exhaustion_all_resources() {
    let enforcer = make_multi_tenant_enforcer();

    assert!(enforcer.check_cpu("tenant-a", 4).is_ok());
    assert!(enforcer.check_memory("tenant-a", 1024).is_ok());
    assert!(enforcer.check_disk("tenant-a", 10_000).is_ok());

    assert!(matches!(
        enforcer.check_cpu("tenant-a", 5),
        Err(QuotaError::QuotaExceeded { .. })
    ));
    assert!(matches!(
        enforcer.check_memory("tenant-a", 1025),
        Err(QuotaError::QuotaExceeded { .. })
    ));
    assert!(matches!(
        enforcer.check_disk("tenant-a", 10_001),
        Err(QuotaError::QuotaExceeded { .. })
    ));

    assert!(enforcer.check_cpu("tenant-b", 8).is_ok());
    assert!(enforcer.check_memory("tenant-b", 4096).is_ok());
    assert!(enforcer.check_disk("tenant-b", 50_000).is_ok());

    assert!(enforcer.check_cpu("tenant-c", 2).is_ok());
    assert!(enforcer.check_memory("tenant-c", 512).is_ok());
    assert!(enforcer.check_disk("tenant-c", 5_000).is_ok());
}

#[test]
fn tenant_isolation_overcommit_tenant_does_not_steal_from_strict() {
    let mut registry = crate::resource_quota::NamespaceRegistry::new();
    let _ = registry.register(
        NamespaceQuota::new("elastic")
            .with_cpu(CpuQuota::new(NonZeroU64::new(2).unwrap()))
            .with_memory(MemoryQuota::new(NonZeroU64::new(256).unwrap()))
            .with_overcommit(OvercommitPolicy::AllowOvercommit),
    );
    let _ = registry.register(
        NamespaceQuota::new("strict")
            .with_cpu(CpuQuota::new(NonZeroU64::new(2).unwrap()))
            .with_memory(MemoryQuota::new(NonZeroU64::new(256).unwrap()))
            .with_overcommit(OvercommitPolicy::NoOvercommit),
    );
    let enforcer = QuotaEnforcer::new(registry);

    assert!(enforcer.check_cpu("elastic", 1000).is_ok());
    assert!(enforcer.check_memory("elastic", 100_000).is_ok());

    assert!(matches!(
        enforcer.check_cpu("strict", 3),
        Err(QuotaError::QuotaExceeded { .. })
    ));
    assert!(matches!(
        enforcer.check_memory("strict", 257),
        Err(QuotaError::QuotaExceeded { .. })
    ));
}

#[test]
fn tenant_isolation_interleaved_checks_maintain_independence() {
    let enforcer = make_multi_tenant_enforcer();

    for round in 0..20 {
        let a_req = round % 5;
        let b_req = round * 100;
        let c_req = round % 3;

        if a_req <= 4 {
            assert!(enforcer.check_cpu("tenant-a", a_req).is_ok());
        }
        if b_req <= 4096 {
            assert!(enforcer.check_memory("tenant-b", b_req).is_ok());
        }
        if c_req <= 2 {
            assert!(enforcer.check_cpu("tenant-c", c_req).is_ok());
        }
    }

    assert!(matches!(
        enforcer.check_cpu("tenant-a", 5),
        Err(QuotaError::QuotaExceeded { .. })
    ));
    assert!(matches!(
        enforcer.check_memory("tenant-b", 4097),
        Err(QuotaError::QuotaExceeded { .. })
    ));
    assert!(matches!(
        enforcer.check_cpu("tenant-c", 3),
        Err(QuotaError::QuotaExceeded { .. })
    ));
}

#[test]
fn tenant_isolation_identical_quota_configs_still_isolated() {
    let mut registry = crate::resource_quota::NamespaceRegistry::new();
    for ns in &["clone-a", "clone-b", "clone-c"] {
        let _ = registry.register(
            NamespaceQuota::new(*ns)
                .with_cpu(CpuQuota::new(NonZeroU64::new(4).unwrap()))
                .with_memory(MemoryQuota::new(NonZeroU64::new(1024).unwrap()))
                .with_disk(DiskQuota::new(NonZeroU64::new(10_000).unwrap())),
        );
    }
    let mut enforcer = QuotaEnforcer::new(registry);

    assert!(enforcer.check_cpu("clone-a", 5).is_err());
    assert!(enforcer.check_cpu("clone-b", 5).is_err());
    assert!(enforcer.check_cpu("clone-c", 5).is_err());

    enforcer.registry_mut().register(
        NamespaceQuota::new("clone-a")
            .with_cpu(CpuQuota::new(NonZeroU64::new(100).unwrap()))
            .with_memory(MemoryQuota::new(NonZeroU64::new(100_000).unwrap()))
            .with_disk(DiskQuota::new(NonZeroU64::new(1_000_000).unwrap())),
    ).expect("register should succeed");

    assert!(enforcer.check_cpu("clone-a", 100).is_ok());
    assert!(enforcer.check_cpu("clone-b", 5).is_err());
    assert!(enforcer.check_cpu("clone-c", 5).is_err());
}

#[test]
fn tenant_isolation_remove_all_except_one() {
    let mut enforcer = make_multi_tenant_enforcer();

    enforcer.registry_mut().remove("tenant-a");
    enforcer.registry_mut().remove("tenant-b");

    assert!(matches!(
        enforcer.check_cpu("tenant-a", 1),
        Err(QuotaError::NamespaceNotFound(_))
    ));
    assert!(matches!(
        enforcer.check_cpu("tenant-b", 1),
        Err(QuotaError::NamespaceNotFound(_))
    ));
    assert!(enforcer.check_cpu("tenant-c", 2).is_ok());
    assert!(enforcer.check_memory("tenant-c", 512).is_ok());
    assert!(enforcer.check_disk("tenant-c", 5_000).is_ok());
}

#[test]
fn tenant_isolation_fair_sharing_with_wide_range() {
    let mut registry = crate::resource_quota::NamespaceRegistry::new();
    let quotas = [
        ("micro", 1u64, 64u64, 100u64),
        ("small", 2u64, 256u64, 1000u64),
        ("medium", 4u64, 1024u64, 10000u64),
        ("large", 8u64, 4096u64, 100000u64),
        ("xlarge", 16u64, 16384u64, 1000000u64),
    ];
    for (name, cpu, mem, disk) in &quotas {
        let _ = registry.register(
            NamespaceQuota::new(*name)
                .with_cpu(CpuQuota::new(NonZeroU64::new(*cpu).unwrap()))
                .with_memory(MemoryQuota::new(NonZeroU64::new(*mem).unwrap()))
                .with_disk(DiskQuota::new(NonZeroU64::new(*disk).unwrap())),
        );
    }
    let enforcer = QuotaEnforcer::new(registry);

    for (name, cpu, mem, disk) in &quotas {
        assert!(enforcer.check_cpu(name, *cpu).is_ok());
        assert!(enforcer.check_memory(name, *mem).is_ok());
        assert!(enforcer.check_disk(name, *disk).is_ok());

        assert!(matches!(
            enforcer.check_cpu(name, cpu + 1),
            Err(QuotaError::QuotaExceeded { .. })
        ));
        assert!(matches!(
            enforcer.check_memory(name, mem + 1),
            Err(QuotaError::QuotaExceeded { .. })
        ));
        assert!(matches!(
            enforcer.check_disk(name, disk + 1),
            Err(QuotaError::QuotaExceeded { .. })
        ));
    }
}
