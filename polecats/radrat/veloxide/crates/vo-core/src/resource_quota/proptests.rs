use super::*;
use crate::resource_quota::{CpuQuota, DiskQuota, MemoryQuota};
use crate::resource_quota::{NamespaceQuota, OvercommitPolicy, QuotaEnforcer, QuotaError};
use proptest::prelude::*;
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

fn is_quota_exceeded_cpu(result: &Result<(), QuotaError>) -> bool {
    matches!(
        result,
        Err(QuotaError::QuotaExceeded {
            resource: ResourceKind::Cpu,
            ..
        })
    )
}

fn is_quota_exceeded_memory(result: &Result<(), QuotaError>) -> bool {
    matches!(
        result,
        Err(QuotaError::QuotaExceeded {
            resource: ResourceKind::Memory,
            ..
        })
    )
}

fn is_quota_exceeded_disk(result: &Result<(), QuotaError>) -> bool {
    matches!(
        result,
        Err(QuotaError::QuotaExceeded {
            resource: ResourceKind::Disk,
            ..
        })
    )
}

fn is_namespace_not_found(result: &Result<(), QuotaError>) -> bool {
    matches!(result, Err(QuotaError::NamespaceNotFound(_)))
}

proptest! {
    #[test]
    fn inv002_cpu_quota_always_ge_one(n in 1u64..) {
        let quota = CpuQuota::new(NonZeroU64::new(n).unwrap());
        prop_assert!(quota.max_cores.get() >= 1);
    }

    #[test]
    fn inv002_memory_quota_always_ge_one(n in 1u64..) {
        let quota = MemoryQuota::new(NonZeroU64::new(n).unwrap());
        prop_assert!(quota.max_bytes.get() >= 1);
    }

    #[test]
    fn inv002_disk_quota_always_ge_one(n in 1u64..) {
        let quota = DiskQuota::new(NonZeroU64::new(n).unwrap());
        prop_assert!(quota.max_bytes.get() >= 1);
    }

    #[test]
    fn inv003_check_cpu_ok_when_within_limit(requested in 0u64..=4) {
        let enforcer = make_test_enforcer();
        let result = enforcer.check_cpu("payments", requested);
        prop_assert!(result.is_ok());
    }

    #[test]
    fn inv003_check_memory_ok_when_within_limit(requested in 0u64..=1024) {
        let enforcer = make_test_enforcer();
        let result = enforcer.check_memory("payments", requested);
        prop_assert!(result.is_ok());
    }

    #[test]
    fn inv003_check_disk_ok_when_within_limit(requested in 0u64..=10_000) {
        let enforcer = make_test_enforcer();
        let result = enforcer.check_disk("payments", requested);
        prop_assert!(result.is_ok());
    }

    #[test]
    fn inv004_check_cpu_quota_exceeded_when_over_no_overcommit(requested in 5u64..100) {
        let enforcer = make_test_enforcer();
        let result = enforcer.check_cpu("payments", requested);
        prop_assert!(is_quota_exceeded_cpu(&result));
    }

    #[test]
    fn inv004_check_memory_quota_exceeded_when_over_no_overcommit(requested in 1025u64..2000) {
        let enforcer = make_test_enforcer();
        let result = enforcer.check_memory("payments", requested);
        prop_assert!(is_quota_exceeded_memory(&result));
    }

    #[test]
    fn inv004_check_disk_quota_exceeded_when_over_no_overcommit(requested in 10_001u64..20_000) {
        let enforcer = make_test_enforcer();
        let result = enforcer.check_disk("payments", requested);
        prop_assert!(is_quota_exceeded_disk(&result));
    }

    #[test]
    fn inv005_check_cpu_ok_when_over_with_allow_overcommit(requested in 5u64..100) {
        let enforcer = make_overcommit_enforcer();
        let result = enforcer.check_cpu("payments", requested);
        prop_assert!(result.is_ok());
    }

    #[test]
    fn inv005_check_memory_ok_when_over_with_allow_overcommit(requested in 1025u64..2000) {
        let enforcer = make_overcommit_enforcer();
        let result = enforcer.check_memory("payments", requested);
        prop_assert!(result.is_ok());
    }

    #[test]
    fn inv005_check_disk_ok_when_over_with_allow_overcommit(requested in 10_001u64..20_000) {
        let enforcer = make_overcommit_enforcer();
        let result = enforcer.check_disk("payments", requested);
        prop_assert!(result.is_ok());
    }

    #[test]
    fn inv006_check_unknown_namespace_always_returns_not_found(
        namespace in "[a-z]{1,10}",
        requested in 0u64..100,
    ) {
        let enforcer = make_test_enforcer();
        prop_assume!(namespace != "payments");
        let result = enforcer.check_cpu(&namespace, requested);
        prop_assert!(is_namespace_not_found(&result));
    }

    #[test]
    fn inv008_namespace_quota_new_has_no_quotas(
        namespace in "[a-z]{1,20}",
    ) {
        let quota = NamespaceQuota::new(&namespace);
        prop_assert!(quota.cpu.is_none());
        prop_assert!(quota.memory.is_none());
        prop_assert!(quota.disk.is_none());
    }

    #[test]
    fn inv009_quota_usage_builder_preserves_values(
        cpu in 0u64..1000,
        mem in 0u64..1000,
        disk in 0u64..1000,
    ) {
        let usage = QuotaUsage::new()
            .with_cpu(cpu)
            .with_memory(mem)
            .with_disk(disk);
        prop_assert_eq!(usage.cpu_cores_used, cpu);
        prop_assert_eq!(usage.memory_bytes_used, mem);
        prop_assert_eq!(usage.disk_bytes_used, disk);
    }

    #[test]
    fn inv012_is_overcommit_rejected_for_all_resource_kinds(
        idx in 0usize..3,
    ) {
        let kinds = [ResourceKind::Cpu, ResourceKind::Memory, ResourceKind::Disk];
        let resource = kinds[idx];
        let exceeded = QuotaError::QuotaExceeded {
            resource,
            namespace: "test".to_string(),
            requested: 10,
            available: 5,
        };
        prop_assert!(exceeded.is_overcommit_rejected());

        let not_configured = QuotaError::QuotaNotConfigured {
            resource,
            namespace: "test".to_string(),
        };
        prop_assert!(not_configured.is_overcommit_rejected());

        let not_found = QuotaError::NamespaceNotFound("test".to_string());
        prop_assert!(!not_found.is_overcommit_rejected());
    }

    #[test]
    fn inv001_registry_insert_and_retrieve_roundtrip(
        cpu in 1u64..1000u64,
        mem in 1u64..1000u64,
        disk in 1u64..1000u64,
    ) {
        let mut registry = super::enforcer::NamespaceRegistry::new();
        let quota = NamespaceQuota::new("test-ns")
            .with_cpu(CpuQuota::new(NonZeroU64::new(cpu).unwrap()))
            .with_memory(MemoryQuota::new(NonZeroU64::new(mem).unwrap()))
            .with_disk(DiskQuota::new(NonZeroU64::new(disk).unwrap()));
        let _ = registry.register(quota);
        let retrieved = registry.get("test-ns");
        prop_assert!(retrieved.is_some());
        let r = retrieved.unwrap();
        prop_assert_eq!(r.cpu.unwrap().max_cores.get(), cpu);
        prop_assert_eq!(r.memory.unwrap().max_bytes.get(), mem);
        prop_assert_eq!(r.disk.unwrap().max_bytes.get(), disk);
    }
}
