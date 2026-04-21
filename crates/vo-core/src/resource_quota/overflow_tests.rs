use super::types::{QuotaUsage, QuotaUsageOverflow};
use super::QuotaError;
use super::ResourceKind;
use std::num::NonZeroU64;

#[test]
fn b060_add_cpu_within_bounds_succeeds() {
    let mut usage = QuotaUsage::new();
    assert!(usage.add_cpu(100).is_ok());
    assert_eq!(usage.cpu_cores_used, 100);
}

#[test]
fn b061_add_memory_within_bounds_succeeds() {
    let mut usage = QuotaUsage::new();
    assert!(usage.add_memory(1024).is_ok());
    assert_eq!(usage.memory_bytes_used, 1024);
}

#[test]
fn b062_add_disk_within_bounds_succeeds() {
    let mut usage = QuotaUsage::new();
    assert!(usage.add_disk(10_000).is_ok());
    assert_eq!(usage.disk_bytes_used, 10_000);
}

#[test]
fn b063_add_cpu_accumulates_correctly() {
    let mut usage = QuotaUsage::new();
    assert!(usage.add_cpu(1).is_ok());
    assert!(usage.add_cpu(2).is_ok());
    assert!(usage.add_cpu(3).is_ok());
    assert_eq!(usage.cpu_cores_used, 6);
}

#[test]
fn b064_add_cpu_at_u64_max_returns_overflow() {
    let mut usage = QuotaUsage::new().with_cpu(u64::MAX);
    let result = usage.add_cpu(1);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.resource, ResourceKind::Cpu);
    assert_eq!(err.current, u64::MAX);
    assert_eq!(err.attempted_addition, 1);
}

#[test]
fn b065_add_memory_at_u64_max_returns_overflow() {
    let mut usage = QuotaUsage::new().with_memory(u64::MAX);
    let result = usage.add_memory(1);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.resource, ResourceKind::Memory);
    assert_eq!(err.current, u64::MAX);
    assert_eq!(err.attempted_addition, 1);
}

#[test]
fn b066_add_disk_at_u64_max_returns_overflow() {
    let mut usage = QuotaUsage::new().with_disk(u64::MAX);
    let result = usage.add_disk(1);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.resource, ResourceKind::Disk);
    assert_eq!(err.current, u64::MAX);
    assert_eq!(err.attempted_addition, 1);
}

#[test]
fn b067_add_cpu_u64_max_to_zero_returns_overflow() {
    let mut usage = QuotaUsage::new();
    assert!(usage.add_cpu(u64::MAX).is_ok());
    assert_eq!(usage.cpu_cores_used, u64::MAX);
}

#[test]
fn b068_add_cpu_does_not_mutate_on_overflow() {
    let mut usage = QuotaUsage::new().with_cpu(u64::MAX - 5);
    let before = usage.cpu_cores_used;
    let result = usage.add_cpu(10);
    assert!(result.is_err());
    assert_eq!(usage.cpu_cores_used, before, "usage must not change on overflow");
}

#[test]
fn b069_add_memory_does_not_mutate_on_overflow() {
    let mut usage = QuotaUsage::new().with_memory(u64::MAX - 5);
    let before = usage.memory_bytes_used;
    let result = usage.add_memory(10);
    assert!(result.is_err());
    assert_eq!(usage.memory_bytes_used, before);
}

#[test]
fn b070_add_disk_does_not_mutate_on_overflow() {
    let mut usage = QuotaUsage::new().with_disk(u64::MAX - 5);
    let before = usage.disk_bytes_used;
    let result = usage.add_disk(10);
    assert!(result.is_err());
    assert_eq!(usage.disk_bytes_used, before);
}

#[test]
fn b071_add_cpu_zero_always_succeeds() {
    let mut usage = QuotaUsage::new().with_cpu(u64::MAX);
    assert!(usage.add_cpu(0).is_ok());
    assert_eq!(usage.cpu_cores_used, u64::MAX);
}

#[test]
fn b072_release_cpu_saturates_at_zero() {
    let mut usage = QuotaUsage::new().with_cpu(5);
    usage.release_cpu(10);
    assert_eq!(usage.cpu_cores_used, 0);
}

#[test]
fn b073_release_memory_saturates_at_zero() {
    let mut usage = QuotaUsage::new().with_memory(100);
    usage.release_memory(200);
    assert_eq!(usage.memory_bytes_used, 0);
}

#[test]
fn b074_release_disk_saturates_at_zero() {
    let mut usage = QuotaUsage::new().with_disk(500);
    usage.release_disk(1000);
    assert_eq!(usage.disk_bytes_used, 0);
}

#[test]
fn b075_wraparound_attempt_cpu_fails_with_checked_add() {
    let mut usage = QuotaUsage::new().with_cpu(u64::MAX - 1);
    let result = usage.add_cpu(2);
    assert!(result.is_err());
    assert_eq!(usage.cpu_cores_used, u64::MAX - 1, "must not wrap to 0");
}

#[test]
fn b076_wraparound_attempt_memory_fails_with_checked_add() {
    let mut usage = QuotaUsage::new().with_memory(u64::MAX - 1);
    let result = usage.add_memory(2);
    assert!(result.is_err());
    assert_eq!(usage.memory_bytes_used, u64::MAX - 1, "must not wrap to 0");
}

#[test]
fn b077_wraparound_attempt_disk_fails_with_checked_add() {
    let mut usage = QuotaUsage::new().with_disk(u64::MAX - 1);
    let result = usage.add_disk(2);
    assert!(result.is_err());
    assert_eq!(usage.disk_bytes_used, u64::MAX - 1, "must not wrap to 0");
}

#[test]
fn b078_quota_usage_overflow_display_includes_details() {
    let err = QuotaUsageOverflow {
        resource: ResourceKind::Cpu,
        current: u64::MAX,
        attempted_addition: 1,
    };
    let display = err.to_string();
    assert!(display.contains("cpu"));
    assert!(display.contains("overflow"));
}

#[test]
fn b079_quota_error_usage_overflow_variant_exists() {
    let err = QuotaError::UsageOverflow {
        resource: ResourceKind::Memory,
        current: u64::MAX,
        attempted: 1,
    };
    let display = err.to_string();
    assert!(display.contains("memory"));
    assert!(display.contains("overflow"));
    assert!(display.contains("18446744073709551615"));
}

#[test]
fn b080_quota_usage_overflow_implements_clone_copy_partial_eq() {
    let o1 = QuotaUsageOverflow {
        resource: ResourceKind::Disk,
        current: 100,
        attempted_addition: 50,
    };
    let o2 = o1;
    assert_eq!(o1, o2);
    let o3 = o1.clone();
    assert_eq!(o1, o3);
}

#[test]
fn b081_release_exact_amount_brings_to_zero() {
    let mut usage = QuotaUsage::new().with_cpu(42).with_memory(1024).with_disk(5000);
    usage.release_cpu(42);
    usage.release_memory(1024);
    usage.release_disk(5000);
    assert_eq!(usage.cpu_cores_used, 0);
    assert_eq!(usage.memory_bytes_used, 0);
    assert_eq!(usage.disk_bytes_used, 0);
}

#[test]
fn b082_accumulate_many_small_adds_up_to_limit() {
    let mut usage = QuotaUsage::new();
    let target = u64::MAX / 3;
    for _ in 0..3 {
        assert!(usage.add_cpu(target).is_ok());
    }
    assert!(usage.add_cpu(1).is_err(), "should overflow after 3 * (MAX/3)");
}

#[test]
fn b083_cross_resource_overflow_does_not_affect_other_fields() {
    let mut usage = QuotaUsage::new().with_cpu(100);
    let cpu_before = usage.cpu_cores_used;
    let result = usage.add_memory(u64::MAX);
    assert!(result.is_ok());
    assert_eq!(usage.cpu_cores_used, cpu_before, "cpu should be unaffected by memory overflow add");
}
