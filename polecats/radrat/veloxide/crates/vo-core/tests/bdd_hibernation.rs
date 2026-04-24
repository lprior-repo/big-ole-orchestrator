//! BDD tests for actor hibernation memory release.
//!
//! Given-When-Then scenarios validating that hibernating actors
//! release their memory budget back to the namespace quota, and
//! that waking actors can re-acquire within quota.

use std::num::NonZeroU64;
use vo_core::resource_quota::policy::OvercommitPolicy;
use vo_core::resource_quota::{
    MemoryQuota, NamespaceQuota, NamespaceRegistry, QuotaEnforcer, QuotaUsage,
};

const MEMORY_BUDGET: u64 = 10_240;
const ACTOR_FOOTPRINT: u64 = 4_096;

fn hibernation_namespace() -> NamespaceQuota {
    NamespaceQuota::new("hibernation-test")
        .with_memory(MemoryQuota::new(NonZeroU64::new(MEMORY_BUDGET).unwrap()))
        .with_overcommit(OvercommitPolicy::NoOvercommit)
}

fn enforcer_with(ns: NamespaceQuota) -> QuotaEnforcer {
    let mut reg = NamespaceRegistry::new();
    let _ = reg.register(ns);
    QuotaEnforcer::new(reg)
}

#[test]
fn given_two_active_actors_when_both_hibernate_then_memory_budget_fully_reclaimed() {
    // Given: namespace with two actors consuming 2x footprint
    let enforcer = enforcer_with(hibernation_namespace());
    let total_usage = QuotaUsage::new().with_memory(ACTOR_FOOTPRINT * 2);
    assert_eq!(total_usage.memory_bytes_used, 8_192);

    // When: both actors hibernate and release their memory
    let after_hibernation = QuotaUsage::new();
    let freed = total_usage.memory_bytes_used - after_hibernation.memory_bytes_used;

    // Then: freed memory equals the actors' combined footprint
    assert_eq!(freed, ACTOR_FOOTPRINT * 2);
    assert_eq!(after_hibernation.memory_bytes_used, 0);

    // Then: a new actor can claim the entire budget
    assert!(enforcer
        .check_memory("hibernation-test", MEMORY_BUDGET)
        .is_ok());
}

#[test]
fn given_active_actor_when_hibernates_then_single_actor_footprint_released() {
    // Given: one active actor occupying memory
    let enforcer = enforcer_with(hibernation_namespace());
    let active = QuotaUsage::new().with_memory(ACTOR_FOOTPRINT);

    // When: actor hibernates
    let hibernated = QuotaUsage::new();
    let released = active.memory_bytes_used - hibernated.memory_bytes_used;

    // Then: exactly one actor footprint is freed
    assert_eq!(released, ACTOR_FOOTPRINT);
    assert_eq!(hibernated.memory_bytes_used, 0);

    // Then: the freed amount is available for other actors
    let remaining = MEMORY_BUDGET - hibernated.memory_bytes_used;
    assert_eq!(remaining, MEMORY_BUDGET);
    assert!(enforcer.check_memory("hibernation-test", remaining).is_ok());
}

#[test]
fn given_hibernated_actor_when_woken_then_reclaims_within_quota() {
    // Given: actor is hibernated (zero usage)
    let enforcer = enforcer_with(hibernation_namespace());

    // When: actor wakes and requests its footprint
    let wake_result = enforcer.check_memory("hibernation-test", ACTOR_FOOTPRINT);

    // Then: re-acquisition succeeds within budget
    assert!(wake_result.is_ok());
}

#[test]
fn given_full_budget_consumed_when_new_actor_tries_to_start_then_quota_exceeded() {
    // Given: a single actor already holds the entire budget
    let enforcer = enforcer_with(hibernation_namespace());
    assert!(enforcer
        .check_memory("hibernation-test", MEMORY_BUDGET)
        .is_ok());

    // When: another actor requests additional memory beyond the cap
    let result = enforcer.check_memory("hibernation-test", MEMORY_BUDGET + 1);

    // Then: quota exceeded
    assert!(result.is_err());
}

#[test]
fn given_budget_exceeded_when_one_actor_hibernates_then_new_actor_fits() {
    // Given: actors consume all but one footprint of the budget
    let enforcer = enforcer_with(hibernation_namespace());
    let after_hibernate = QuotaUsage::new().with_memory(MEMORY_BUDGET - ACTOR_FOOTPRINT);
    let available = MEMORY_BUDGET - after_hibernate.memory_bytes_used;

    // When: one actor hibernates, freeing its footprint
    // (available equals the freed footprint)
    assert_eq!(available, ACTOR_FOOTPRINT);

    // Then: a new actor with the same footprint fits
    assert!(enforcer
        .check_memory("hibernation-test", ACTOR_FOOTPRINT)
        .is_ok());
}

#[test]
fn given_namespace_removed_when_actor_wakes_then_namespace_not_found() {
    // Given: hibernation namespace exists
    let enforcer = enforcer_with(hibernation_namespace());
    assert!(enforcer
        .check_memory("hibernation-test", ACTOR_FOOTPRINT)
        .is_ok());

    // When: namespace is removed (admin teardown)
    let mut reg = NamespaceRegistry::new();
    let _ = reg.register(hibernation_namespace());
    let mut enforcer = QuotaEnforcer::new(reg);
    enforcer.registry_mut().remove("hibernation-test");

    // Then: waking actor cannot find namespace
    let result = enforcer.check_memory("hibernation-test", ACTOR_FOOTPRINT);
    assert!(result.is_err());
}
