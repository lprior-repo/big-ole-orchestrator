//! WriteClass and WriteBudget integration tests.

use vo_core::write_class::{WriteBudget, WriteClass};

use crate::helpers::{make_wf, make_hash};

#[test]
fn write_class_and_resource_quota_compose_correctly() {
    let mut enforcer = vo_core::resource_quota::QuotaEnforcer::with_default_namespace();
    let quota = vo_core::resource_quota::NamespaceQuota::new("test-ns")
        .with_cpu(vo_core::resource_quota::CpuQuota::new(
            std::num::NonZeroU64::new(4).expect("non-zero"),
        ))
        .with_memory(vo_core::resource_quota::MemoryQuota::new(
            std::num::NonZeroU64::new(1024).expect("non-zero"),
        ))
        .with_overcommit(vo_core::resource_quota::OvercommitPolicy::AllowOvercommit);

    enforcer
        .registry_mut()
        .register(quota)
        .expect("quota registration should succeed");

    let result = enforcer.check_cpu("test-ns", 2);
    assert!(result.is_ok(), "cpu check within quota should succeed");

    let result = enforcer.check_memory("test-ns", 512);
    assert!(result.is_ok(), "memory check within quota should succeed");

    let budget = WriteBudget::new(100, 200, 300);
    assert!(budget.can_write(WriteClass::CriticalControlPlane, 50));
    assert!(budget.reserve(WriteClass::CriticalControlPlane, 50).is_ok());
    assert_eq!(
        budget.remaining(WriteClass::CriticalControlPlane),
        50,
        "remaining budget should be 50 after reserve"
    );
}

#[test]
fn write_class_tier_influence_resource_allocation() {
    let budget = WriteBudget::new(100, 200, 300);

    let critical_reserve = budget.reserve(WriteClass::CriticalControlPlane, 100);
    assert!(
        critical_reserve.is_ok(),
        "critical tier should allow full allocation"
    );

    let projection_reserve = budget.reserve(WriteClass::OperatorProjection, 200);
    assert!(
        projection_reserve.is_ok(),
        "projection tier should allow full allocation"
    );

    let bulk_reserve = budget.reserve(WriteClass::BulkBlob, 300);
    assert!(
        bulk_reserve.is_ok(),
        "bulk tier should allow full allocation"
    );

    assert_eq!(
        budget.remaining(WriteClass::CriticalControlPlane),
        0,
        "critical tier should be exhausted"
    );
    assert_eq!(
        budget.remaining(WriteClass::OperatorProjection),
        0,
        "projection tier should be exhausted"
    );
    assert_eq!(
        budget.remaining(WriteClass::BulkBlob),
        0,
        "bulk tier should be exhausted"
    );
}

#[test]
fn write_budget_tier_isolation() {
    let budget = WriteBudget::new(100, 200, 300);

    assert_eq!(
        budget.remaining(WriteClass::CriticalControlPlane),
        100,
        "critical tier starts with full budget"
    );
    assert_eq!(
        budget.remaining(WriteClass::OperatorProjection),
        200,
        "projection tier starts with full budget"
    );
    assert_eq!(
        budget.remaining(WriteClass::BulkBlob),
        300,
        "bulk tier starts with full budget"
    );

    budget
        .reserve(WriteClass::CriticalControlPlane, 50)
        .expect("reserve should succeed");
    assert_eq!(
        budget.remaining(WriteClass::CriticalControlPlane),
        50,
        "critical tier should have 50 remaining"
    );

    assert_eq!(
        budget.remaining(WriteClass::OperatorProjection),
        200,
        "projection tier should be unaffected"
    );
    assert_eq!(
        budget.remaining(WriteClass::BulkBlob),
        300,
        "bulk tier should be unaffected"
    );
}

#[test]
fn write_budget_critical_never_drops_flag() {
    let critical = WriteClass::CriticalControlPlane;
    let projection = WriteClass::OperatorProjection;
    let bulk = WriteClass::BulkBlob;

    assert!(
        critical.never_drops(),
        "critical control plane writes should never be dropped"
    );
    assert!(
        !projection.never_drops(),
        "operator projection writes may be dropped under pressure"
    );
    assert!(
        !bulk.never_drops(),
        "bulk blob writes may be deferred under pressure"
    );
}

#[test]
fn write_budget_can_write_consistency() {
    let budget = WriteBudget::new(100, 200, 300);

    let test_cases = [
        (WriteClass::CriticalControlPlane, 0u64),
        (WriteClass::CriticalControlPlane, 50),
        (WriteClass::CriticalControlPlane, 100),
        (WriteClass::OperatorProjection, 150),
        (WriteClass::BulkBlob, 250),
    ];

    for (class, size) in test_cases {
        let can_write = budget.can_write(class, size);
        let reserve_result = budget.reserve(class, size);
        assert_eq!(
            can_write,
            reserve_result.is_ok(),
            "can_write({:?}, {}) should match reserve result",
            class,
            size
        );
    }
}

#[test]
fn write_class_json_serialization_roundtrip() {
    let classes = [
        WriteClass::CriticalControlPlane,
        WriteClass::OperatorProjection,
        WriteClass::BulkBlob,
    ];

    for class in classes {
        let json = serde_json::to_string(&class).expect("serialization should succeed");
        let parsed: WriteClass =
            serde_json::from_str(&json).expect("deserialization should succeed");
        assert_eq!(
            parsed, class,
            "WriteClass {:?} should round-trip through JSON",
            class
        );
    }
}

#[test]
fn write_class_parse_and_as_str_consistency() {
    let classes = [
        (WriteClass::CriticalControlPlane, "critical_control_plane"),
        (WriteClass::OperatorProjection, "operator_projection"),
        (WriteClass::BulkBlob, "bulk_blob"),
    ];

    for (class, expected_str) in classes {
        assert_eq!(
            class.as_str(),
            expected_str,
            "as_str() should return canonical string"
        );
        let parsed = WriteClass::parse(expected_str).expect("parse should succeed");
        assert_eq!(
            parsed, class,
            "parse(\"{}\") should return {:?}",
            expected_str, class
        );
    }
}