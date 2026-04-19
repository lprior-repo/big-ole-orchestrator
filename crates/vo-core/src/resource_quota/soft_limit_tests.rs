use super::policy::OvercommitPolicy;
use super::registry::NamespaceRegistry;
use super::types::{
    CpuQuota, DiskQuota, MemoryQuota, NamespaceQuota, QuotaCheckResult, QuotaError,
    SoftLimitPercent, SoftLimitWarning,
};
use super::{QuotaEnforcer, QuotaWarningTracker, ResourceKind};
use std::num::NonZeroU64;

fn make_soft_limit_enforcer(_soft_percent: SoftLimitPercent) -> QuotaEnforcer {
    let mut registry = NamespaceRegistry::new();
    let _ = registry.register(
        NamespaceQuota::new("payments")
            .with_cpu(CpuQuota::new(NonZeroU64::new(100).unwrap()))
            .with_memory(MemoryQuota::new(NonZeroU64::new(1000).unwrap()))
            .with_disk(DiskQuota::new(NonZeroU64::new(10_000).unwrap()))
            .with_overcommit(OvercommitPolicy::NoOvercommit),
    );
    let enforcer = QuotaEnforcer::new(registry);
    enforcer.warning_tracker().set_interval(0);
    enforcer
}

// --- SoftLimitPercent tests ---

#[test]
fn soft_limit_percent_default_is_80() {
    assert_eq!(SoftLimitPercent::DEFAULT.value(), 80);
}

#[test]
fn soft_limit_percent_default_trait() {
    assert_eq!(SoftLimitPercent::default().value(), 80);
}

#[test]
fn soft_limit_percent_new_clamps_to_1_99() {
    assert_eq!(SoftLimitPercent::new(0).value(), 1);
    assert_eq!(SoftLimitPercent::new(1).value(), 1);
    assert_eq!(SoftLimitPercent::new(50).value(), 50);
    assert_eq!(SoftLimitPercent::new(99).value(), 99);
    assert_eq!(SoftLimitPercent::new(100).value(), 99);
    assert_eq!(SoftLimitPercent::new(255).value(), 99);
}

#[test]
fn soft_limit_percent_threshold_for_calculates_correctly() {
    let p80 = SoftLimitPercent::new(80);
    assert_eq!(p80.threshold_for(100), 80);
    assert_eq!(p80.threshold_for(1000), 800);
    assert_eq!(p80.threshold_for(10), 8);

    let p50 = SoftLimitPercent::new(50);
    assert_eq!(p50.threshold_for(100), 50);
    assert_eq!(p50.threshold_for(7), 3); // 7 * 50 / 100 = 3 (integer division)
}

#[test]
fn soft_limit_percent_threshold_for_zero_hard_limit() {
    let p = SoftLimitPercent::new(80);
    assert_eq!(p.threshold_for(0), 0);
}

// --- SoftLimitWarning tests ---

#[test]
fn soft_limit_warning_new_calculates_utilization() {
    let w = SoftLimitWarning::new(ResourceKind::Cpu, "ns", 85, 80, 100);
    assert_eq!(w.resource, ResourceKind::Cpu);
    assert_eq!(w.namespace, "ns");
    assert_eq!(w.requested, 85);
    assert_eq!(w.soft_threshold, 80);
    assert_eq!(w.hard_limit, 100);
    assert_eq!(w.utilization_percent, 85);
}

#[test]
fn soft_limit_warning_new_clamps_utilization_to_100() {
    let w = SoftLimitWarning::new(ResourceKind::Memory, "ns", 150, 80, 100);
    assert_eq!(w.utilization_percent, 100);
}

#[test]
fn soft_limit_warning_display_contains_key_fields() {
    let w = SoftLimitWarning::new(ResourceKind::Disk, "analytics", 900, 800, 1000);
    let display = w.to_string();
    assert!(display.contains("disk"));
    assert!(display.contains("analytics"));
    assert!(display.contains("900"));
    assert!(display.contains("1000"));
}

// --- QuotaCheckResult tests ---

#[test]
fn quota_check_result_is_within_limits() {
    assert!(QuotaCheckResult::WithinLimits.is_within_limits());
    assert!(!QuotaCheckResult::WithinLimits.is_soft_warning());
    assert!(!QuotaCheckResult::WithinLimits.is_hard_exceeded());
}

#[test]
fn quota_check_result_soft_limit_exceeded() {
    let warning = SoftLimitWarning::new(ResourceKind::Cpu, "ns", 85, 80, 100);
    let result = QuotaCheckResult::SoftLimitExceeded(warning);
    assert!(!result.is_within_limits());
    assert!(result.is_soft_warning());
    assert!(!result.is_hard_exceeded());
}

#[test]
fn quota_check_result_hard_limit_exceeded() {
    let err = QuotaError::QuotaExceeded {
        resource: ResourceKind::Cpu,
        namespace: "ns".to_string(),
        requested: 150,
        available: 100,
    };
    let result = QuotaCheckResult::HardLimitExceeded(err);
    assert!(!result.is_within_limits());
    assert!(!result.is_soft_warning());
    assert!(result.is_hard_exceeded());
}

#[test]
fn quota_check_result_into_hard_result_ok_for_within_and_soft() {
    let warning = SoftLimitWarning::new(ResourceKind::Cpu, "ns", 85, 80, 100);
    assert!(QuotaCheckResult::WithinLimits.into_hard_result().is_ok());
    assert!(QuotaCheckResult::SoftLimitExceeded(warning).into_hard_result().is_ok());
}

#[test]
fn quota_check_result_into_hard_result_err_for_hard() {
    let err = QuotaError::QuotaExceeded {
        resource: ResourceKind::Cpu,
        namespace: "ns".to_string(),
        requested: 150,
        available: 100,
    };
    let result = QuotaCheckResult::HardLimitExceeded(err);
    assert!(result.into_hard_result().is_err());
}

#[test]
fn quota_check_result_soft_warning_extracts_warning() {
    let warning = SoftLimitWarning::new(ResourceKind::Memory, "ns", 90, 80, 100);
    let result = QuotaCheckResult::SoftLimitExceeded(warning);
    let extracted = result.soft_warning().unwrap();
    assert_eq!(extracted.resource, ResourceKind::Memory);
    assert_eq!(extracted.utilization_percent, 90);
}

#[test]
fn quota_check_result_soft_warning_returns_none_for_other_variants() {
    assert!(QuotaCheckResult::WithinLimits.soft_warning().is_none());
    let err = QuotaError::NamespaceNotFound("x".to_string());
    assert!(QuotaCheckResult::HardLimitExceeded(err).soft_warning().is_none());
}

// --- Enforcer soft limit integration tests ---

#[test]
fn soft_limit_below_threshold_returns_within_limits() {
    let enforcer = make_soft_limit_enforcer(SoftLimitPercent::new(80));
    let result = enforcer.check_cpu_with_soft_limit("payments", 50, SoftLimitPercent::new(80));
    assert_eq!(result, QuotaCheckResult::WithinLimits);
}

#[test]
fn soft_limit_at_threshold_returns_soft_warning() {
    let enforcer = make_soft_limit_enforcer(SoftLimitPercent::new(80));
    // 80 cores out of 100 = exactly at 80% threshold
    let result = enforcer.check_cpu_with_soft_limit("payments", 80, SoftLimitPercent::new(80));
    assert!(result.is_soft_warning());
}

#[test]
fn soft_limit_above_threshold_below_hard_returns_soft_warning() {
    let enforcer = make_soft_limit_enforcer(SoftLimitPercent::new(80));
    let result = enforcer.check_cpu_with_soft_limit("payments", 90, SoftLimitPercent::new(80));
    assert!(result.is_soft_warning());
    let warning = result.soft_warning().unwrap();
    assert_eq!(warning.requested, 90);
    assert_eq!(warning.soft_threshold, 80);
    assert_eq!(warning.hard_limit, 100);
}

#[test]
fn soft_limit_at_hard_limit_returns_soft_warning_not_error() {
    let enforcer = make_soft_limit_enforcer(SoftLimitPercent::new(80));
    let result = enforcer.check_cpu_with_soft_limit("payments", 100, SoftLimitPercent::new(80));
    assert!(result.is_soft_warning());
}

#[test]
fn soft_limit_above_hard_limit_returns_hard_exceeded() {
    let enforcer = make_soft_limit_enforcer(SoftLimitPercent::new(80));
    let result = enforcer.check_cpu_with_soft_limit("payments", 101, SoftLimitPercent::new(80));
    assert!(result.is_hard_exceeded());
}

#[test]
fn soft_limit_zero_requested_returns_within_limits() {
    let enforcer = make_soft_limit_enforcer(SoftLimitPercent::new(80));
    let result = enforcer.check_cpu_with_soft_limit("payments", 0, SoftLimitPercent::new(80));
    assert!(result.is_within_limits());
}

#[test]
fn soft_limit_memory_check_works() {
    let enforcer = make_soft_limit_enforcer(SoftLimitPercent::new(80));
    // 800 bytes out of 1000 = at 80% threshold
    let result = enforcer.check_memory_with_soft_limit("payments", 800, SoftLimitPercent::new(80));
    assert!(result.is_soft_warning());

    let result = enforcer.check_memory_with_soft_limit("payments", 500, SoftLimitPercent::new(80));
    assert!(result.is_within_limits());

    let result = enforcer.check_memory_with_soft_limit("payments", 1001, SoftLimitPercent::new(80));
    assert!(result.is_hard_exceeded());
}

#[test]
fn soft_limit_disk_check_works() {
    let enforcer = make_soft_limit_enforcer(SoftLimitPercent::new(80));
    // 8000 out of 10000 = at 80%
    let result = enforcer.check_disk_with_soft_limit("payments", 8000, SoftLimitPercent::new(80));
    assert!(result.is_soft_warning());

    let result = enforcer.check_disk_with_soft_limit("payments", 2000, SoftLimitPercent::new(80));
    assert!(result.is_within_limits());

    let result = enforcer.check_disk_with_soft_limit("payments", 10001, SoftLimitPercent::new(80));
    assert!(result.is_hard_exceeded());
}

#[test]
fn soft_limit_overcommit_bypasses_hard_limit() {
    let mut registry = NamespaceRegistry::new();
    let _ = registry.register(
        NamespaceQuota::new("flex")
            .with_cpu(CpuQuota::new(NonZeroU64::new(100).unwrap()))
            .with_overcommit(OvercommitPolicy::AllowOvercommit),
    );
    let enforcer = QuotaEnforcer::new(registry);
    enforcer.warning_tracker().set_interval(0);

    // Even with overcommit, soft warning still fires
    let result = enforcer.check_cpu_with_soft_limit("flex", 150, SoftLimitPercent::new(80));
    assert!(result.is_within_limits());
}

#[test]
fn soft_limit_namespace_not_found_returns_hard_exceeded() {
    let enforcer = make_soft_limit_enforcer(SoftLimitPercent::new(80));
    let result = enforcer.check_cpu_with_soft_limit("unknown", 50, SoftLimitPercent::new(80));
    assert!(matches!(
        result,
        QuotaCheckResult::HardLimitExceeded(QuotaError::NamespaceNotFound(_))
    ));
}

#[test]
fn soft_limit_resource_not_configured_returns_hard_exceeded() {
    let mut registry = NamespaceRegistry::new();
    let _ = registry.register(
        NamespaceQuota::new("no-cpu").with_memory(MemoryQuota::new(NonZeroU64::new(100).unwrap())),
    );
    let enforcer = QuotaEnforcer::new(registry);
    enforcer.warning_tracker().set_interval(0);

    let result = enforcer.check_cpu_with_soft_limit("no-cpu", 50, SoftLimitPercent::new(80));
    assert!(matches!(
        result,
        QuotaCheckResult::HardLimitExceeded(QuotaError::QuotaNotConfigured { .. })
    ));
}

// --- Threshold crossing test (boundary) ---

#[test]
fn soft_limit_crossing_just_below_threshold() {
    let enforcer = make_soft_limit_enforcer(SoftLimitPercent::new(80));
    // 79 out of 100 = 79%, just below 80%
    let result = enforcer.check_cpu_with_soft_limit("payments", 79, SoftLimitPercent::new(80));
    assert!(result.is_within_limits());
}

#[test]
fn soft_limit_crossing_just_at_threshold() {
    let enforcer = make_soft_limit_enforcer(SoftLimitPercent::new(80));
    let result = enforcer.check_cpu_with_soft_limit("payments", 80, SoftLimitPercent::new(80));
    assert!(result.is_soft_warning());
}

// --- Warning tracker integration ---

#[test]
fn soft_limit_activates_warning_tracker() {
    let enforcer = make_soft_limit_enforcer(SoftLimitPercent::new(80));
    assert!(!enforcer.warning_tracker().is_active(ResourceKind::Cpu));

    enforcer.check_cpu_with_soft_limit("payments", 85, SoftLimitPercent::new(80));
    assert!(enforcer.warning_tracker().is_active(ResourceKind::Cpu));
}

#[test]
fn soft_limit_below_threshold_clears_warning() {
    let enforcer = make_soft_limit_enforcer(SoftLimitPercent::new(80));
    // Trigger warning
    enforcer.check_cpu_with_soft_limit("payments", 85, SoftLimitPercent::new(80));
    assert!(enforcer.warning_tracker().is_active(ResourceKind::Cpu));

    // Drop below threshold
    enforcer.check_cpu_with_soft_limit("payments", 50, SoftLimitPercent::new(80));
    assert!(!enforcer.warning_tracker().is_active(ResourceKind::Cpu));
}

#[test]
fn soft_limit_warning_frequency_rate_limits() {
    let mut registry = NamespaceRegistry::new();
    let _ = registry.register(
        NamespaceQuota::new("payments")
            .with_cpu(CpuQuota::new(NonZeroU64::new(100).unwrap()))
            .with_overcommit(OvercommitPolicy::NoOvercommit),
    );
    let enforcer = QuotaEnforcer::new(registry);

    let soft = SoftLimitPercent::new(80);
    let r1 = enforcer.check_cpu_with_soft_limit("payments", 85, soft);
    assert!(r1.is_soft_warning());

    // Second call within rate limit window should still return SoftLimitExceeded
    // but the tracker won't re-emit the underlying warning
    let count_after_first = enforcer.warning_tracker().recent_count();
    enforcer.check_cpu_with_soft_limit("payments", 86, soft);
    let count_after_second = enforcer.warning_tracker().recent_count();
    assert_eq!(count_after_first, count_after_second);
}

#[test]
fn soft_limit_all_resources_track_independently() {
    let enforcer = make_soft_limit_enforcer(SoftLimitPercent::new(80));

    enforcer.check_cpu_with_soft_limit("payments", 85, SoftLimitPercent::new(80));
    assert!(enforcer.warning_tracker().is_active(ResourceKind::Cpu));
    assert!(!enforcer.warning_tracker().is_active(ResourceKind::Memory));
    assert!(!enforcer.warning_tracker().is_active(ResourceKind::Disk));

    enforcer.check_memory_with_soft_limit("payments", 850, SoftLimitPercent::new(80));
    assert!(enforcer.warning_tracker().is_active(ResourceKind::Cpu));
    assert!(enforcer.warning_tracker().is_active(ResourceKind::Memory));
    assert!(!enforcer.warning_tracker().is_active(ResourceKind::Disk));
}

#[test]
fn soft_limit_any_active_detects_multiple() {
    let enforcer = make_soft_limit_enforcer(SoftLimitPercent::new(80));
    assert!(!enforcer.warning_tracker().any_active());

    enforcer.check_cpu_with_soft_limit("payments", 85, SoftLimitPercent::new(80));
    assert!(enforcer.warning_tracker().any_active());
}

// --- Backward compatibility: check_* without soft limit ---

#[test]
fn check_cpu_backward_compat_returns_ok_under_limit() {
    let enforcer = make_soft_limit_enforcer(SoftLimitPercent::new(80));
    assert!(enforcer.check_cpu("payments", 50).is_ok());
}

#[test]
fn check_cpu_backward_compat_returns_ok_at_limit() {
    let enforcer = make_soft_limit_enforcer(SoftLimitPercent::new(80));
    assert!(enforcer.check_cpu("payments", 100).is_ok());
}

#[test]
fn check_cpu_backward_compat_returns_error_over_limit() {
    let enforcer = make_soft_limit_enforcer(SoftLimitPercent::new(80));
    assert!(enforcer.check_cpu("payments", 101).is_err());
}

// --- Serialization tests ---

#[test]
fn soft_limit_percent_serializes_and_deserializes() {
    let p = SoftLimitPercent::new(75);
    let json = serde_json::to_string(&p).unwrap();
    let p2: SoftLimitPercent = serde_json::from_str(&json).unwrap();
    assert_eq!(p, p2);
}

#[test]
fn soft_limit_percent_default_serializes() {
    let p = SoftLimitPercent::default();
    let json = serde_json::to_string(&p).unwrap();
    let p2: SoftLimitPercent = serde_json::from_str(&json).unwrap();
    assert_eq!(p, p2);
}

// --- QuotaWarningTracker standalone tests ---

#[test]
fn warning_tracker_default_interval_is_60_seconds() {
    let tracker = QuotaWarningTracker::new();
    assert_eq!(tracker.interval(), 60);
}

#[test]
fn warning_tracker_clear_resets_state() {
    let tracker = QuotaWarningTracker::with_interval(0);
    let w = SoftLimitWarning::new(ResourceKind::Cpu, "ns", 85, 80, 100);
    tracker.record_warning(&w);
    assert!(tracker.is_active(ResourceKind::Cpu));
    tracker.clear_warning(ResourceKind::Cpu);
    assert!(!tracker.is_active(ResourceKind::Cpu));
}

#[test]
fn warning_tracker_set_interval_updates() {
    let tracker = QuotaWarningTracker::new();
    tracker.set_interval(300);
    assert_eq!(tracker.interval(), 300);
}
