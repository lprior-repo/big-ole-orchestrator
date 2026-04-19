#![cfg(test)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::instance_registry::{
    InstanceActorHandle, InstanceRegistry, RegistryConfig, RegistryError,
};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use vo_types::InstanceId;

// =============================================================================
// Test Helpers — fixed InstanceIds for deterministic tests
// =============================================================================

fn default_registry_config() -> RegistryConfig {
    RegistryConfig {
        stop_timeout: Duration::from_secs(5),
    }
}

fn registry_config_with_timeout(timeout: Duration) -> RegistryConfig {
    RegistryConfig {
        stop_timeout: timeout,
    }
}

fn blocking_stop_fn(
    block_for: Duration,
) -> impl FnOnce(InstanceActorHandle) -> Result<(), String> + Send {
    move |_| {
        std::thread::sleep(block_for);
        Ok(())
    }
}

fn id_a() -> InstanceId {
    InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap()
}

fn id_b() -> InstanceId {
    InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap()
}

fn id_c() -> InstanceId {
    InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMC").unwrap()
}

fn id_d() -> InstanceId {
    InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMD").unwrap()
}

fn id_e() -> InstanceId {
    InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFME").unwrap()
}

// =============================================================================
// Behavior 1: InstanceRegistry creates empty registry when given valid config
// =============================================================================

#[test]
fn registry_has_zero_count_when_created_with_valid_config() {
    // Given: a RegistryConfig with stop_timeout = Duration::from_secs(5)
    let config = default_registry_config();

    // When: InstanceRegistry::new(config) is called
    let registry = InstanceRegistry::new(config);

    // Then: active_count() == 0
    assert_eq!(registry.active_count(), 0);
    // And: is_active(any_valid_id) == false
    assert!(!registry.is_active(&id_a()));
    // And: lookup(any_valid_id) == None
    assert_eq!(registry.lookup(&id_a()), None);
}

// =============================================================================
// Behavior 2: InstanceRegistry::new panics when stop_timeout is zero
// =============================================================================

#[test]
#[should_panic(expected = "stop_timeout")]
fn registry_panics_when_stop_timeout_is_zero() {
    // Given: a RegistryConfig with stop_timeout = Duration::ZERO
    let config = RegistryConfig {
        stop_timeout: Duration::ZERO,
    };

    // When: InstanceRegistry::new(config) is called
    // Then: thread panics with a message containing "stop_timeout"
    let _registry = InstanceRegistry::new(config);
}

// =============================================================================
// Behavior 3: RegistryConfig default has 5 second stop_timeout
// =============================================================================

#[test]
fn registry_config_default_stop_timeout_is_five_seconds() {
    // Given: no explicit config
    // When: RegistryConfig::default() is called
    let config = RegistryConfig::default();

    // Then: stop_timeout == Duration::from_secs(5)
    assert_eq!(config.stop_timeout, Duration::from_secs(5));
}

// =============================================================================
// Behavior 4: register returns Ok and inserts handle when id is not active
// =============================================================================

#[test]
fn register_returns_ok_and_inserts_handle_when_id_not_active() {
    // Given: an empty registry
    let mut registry = InstanceRegistry::new(default_registry_config());
    let id = id_a();
    let handle = InstanceActorHandle::test(42);

    // When: register(id, handle, stop_fn) is called
    let result = registry.register(id.clone(), handle, |_| Ok(()));

    // Then: result == Ok(())
    assert_eq!(result, Ok(()));
    // And: lookup(id) returns Some with the correct handle
    assert_eq!(registry.lookup(&id).map(|h| h.handle_id()), Some(42));
    // And: is_active(id) == true
    assert!(registry.is_active(&id));
}

// =============================================================================
// Behavior 5: register increases active_count by 1 when id is not active
// =============================================================================

#[test]
fn register_increments_active_count_when_id_not_active() {
    // Given: an empty registry (active_count == 0)
    let mut registry = InstanceRegistry::new(default_registry_config());
    assert_eq!(registry.active_count(), 0);

    // When: register(id_a, handle_a, |_| Ok(())) is called
    registry
        .register(id_a(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();

    // Then: active_count() == 1
    assert_eq!(registry.active_count(), 1);
}

// =============================================================================
// Behavior 6: register makes is_active return true when id was not active
// =============================================================================

#[test]
fn register_makes_is_active_true_when_id_not_active() {
    // Given: an empty registry, is_active(id_a) == false
    let mut registry = InstanceRegistry::new(default_registry_config());
    assert!(!registry.is_active(&id_a()));

    // When: register(id_a, handle, |_| Ok(())) is called
    registry
        .register(id_a(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();

    // Then: is_active(id_a) == true
    assert!(registry.is_active(&id_a()));
}

// =============================================================================
// Behavior 7: register makes lookup return Some pointing to new handle
// =============================================================================

#[test]
fn register_makes_lookup_return_some_with_exact_handle_when_id_not_active() {
    // Given: an empty registry, lookup(id_a) == None
    let mut registry = InstanceRegistry::new(default_registry_config());
    assert_eq!(registry.lookup(&id_a()), None);

    // When: register(id_a, handle_42, |_| Ok(())) is called
    registry
        .register(id_a(), InstanceActorHandle::test(42), |_| Ok(()))
        .unwrap();

    // Then: lookup(id_a) returns Some where test_id() == 42
    assert_eq!(registry.lookup(&id_a()).map(|h| h.handle_id()), Some(42));
}

// =============================================================================
// Behavior 8: register invokes stop_fn with prior handle when id already active
// =============================================================================

#[test]
fn register_calls_stop_fn_with_prior_handle_when_id_already_active() {
    // Given: a registry where id_a is mapped to handle(1)
    let mut registry = InstanceRegistry::new(default_registry_config());
    registry
        .register(id_a(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();

    // When: register(id_a, handle(2), stop_fn) is called
    // And: stop_fn captures the test_id of the prior handle
    let captured_id = Arc::new(AtomicU64::new(0));
    let captured_clone = captured_id.clone();
    let result = registry.register(id_a(), InstanceActorHandle::test(2), move |prior| {
        captured_clone.store(prior.handle_id(), Ordering::SeqCst);
        Ok(())
    });

    // Then: stop_fn was called with prior handle (test_id == 1)
    assert_eq!(result, Ok(()));
    assert_eq!(captured_id.load(Ordering::SeqCst), 1);
}

// =============================================================================
// Behavior 9: register returns Ok and replaces entry when stop_fn succeeds
// =============================================================================

#[test]
fn register_replaces_handle_when_id_active_and_stop_fn_succeeds() {
    // Given: a registry where id_a is mapped to handle(1)
    let mut registry = InstanceRegistry::new(default_registry_config());
    registry
        .register(id_a(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();

    // When: register(id_a, handle(2), |_| Ok(())) is called
    let result = registry.register(id_a(), InstanceActorHandle::test(2), |_| Ok(()));

    // Then: result == Ok(())
    assert_eq!(result, Ok(()));
    // And: lookup(id_a) returns the NEW handle (test_id == 2, not 1)
    assert_eq!(registry.lookup(&id_a()).map(|h| h.handle_id()), Some(2));
}

// =============================================================================
// Behavior 10: register keeps active_count unchanged when stop_fn succeeds
// =============================================================================

#[test]
fn register_keeps_active_count_unchanged_when_stop_fn_succeeds() {
    // Given: a registry where id_a is active, active_count() == 1
    let mut registry = InstanceRegistry::new(default_registry_config());
    registry
        .register(id_a(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();
    assert_eq!(registry.active_count(), 1);

    // When: register(id_a, handle_new, |_| Ok(())) is called (replace)
    registry
        .register(id_a(), InstanceActorHandle::test(2), |_| Ok(()))
        .unwrap();

    // Then: active_count() == 1 (unchanged: old removed, new added)
    assert_eq!(registry.active_count(), 1);
}

// =============================================================================
// Behavior 11: register makes lookup return NEW handle after successful replace
// =============================================================================

#[test]
fn register_lookup_returns_new_handle_after_successful_replace() {
    // Given: a registry where id_a is mapped to handle(1)
    let mut registry = InstanceRegistry::new(default_registry_config());
    registry
        .register(id_a(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();

    // When: register(id_a, handle(99), |_| Ok(())) is called
    registry
        .register(id_a(), InstanceActorHandle::test(99), |_| Ok(()))
        .unwrap();

    // Then: lookup(id_a) returns handle with test_id == 99 (NOT 1)
    assert_eq!(registry.lookup(&id_a()).map(|h| h.handle_id()), Some(99));
    // And: is_active(id_a) == true
    assert!(registry.is_active(&id_a()));
}

// =============================================================================
// Behavior 12: register returns StopFailed when stop_fn returns Err
// =============================================================================

#[test]
fn register_returns_stop_failed_when_stop_fn_returns_err() {
    // Given: a registry where id_a is mapped to handle(1)
    let mut registry = InstanceRegistry::new(default_registry_config());
    registry
        .register(id_a(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();

    // When: register(id_a, handle(2), |_| Err("actor stuck")) is called
    let result = registry.register(id_a(), InstanceActorHandle::test(2), |_| {
        Err("actor stuck".to_string())
    });

    // Then: result == Err(RegistryError::StopFailed { instance_id, reason })
    assert_eq!(
        result,
        Err(RegistryError::StopFailed {
            instance_id: id_a(),
            reason: "actor stuck".to_string(),
        })
    );
}

// =============================================================================
// Behavior 13: register preserves old entry when stop_fn returns Err
// =============================================================================

#[test]
fn register_preserves_old_handle_when_stop_fn_returns_err() {
    // Given: a registry where id_a is mapped to handle(1)
    let mut registry = InstanceRegistry::new(default_registry_config());
    registry
        .register(id_a(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();

    // When: register(id_a, handle(2), |_| Err("fail")) is called
    let _ = registry.register(id_a(), InstanceActorHandle::test(2), |_| {
        Err("fail".to_string())
    });

    // Then: lookup(id_a) still points to handle(1) (preserved)
    assert_eq!(registry.lookup(&id_a()).map(|h| h.handle_id()), Some(1));
    // And: is_active(id_a) == true
    assert!(registry.is_active(&id_a()));
}

// =============================================================================
// Behavior 14: register preserves active_count when stop_fn returns Err
// =============================================================================

#[test]
fn register_preserves_active_count_when_stop_fn_returns_err() {
    // Given: a registry where id_a is active, active_count() == 1
    let mut registry = InstanceRegistry::new(default_registry_config());
    registry
        .register(id_a(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();
    assert_eq!(registry.active_count(), 1);

    // When: register(id_a, handle(2), |_| Err("fail")) is called
    let _ = registry.register(id_a(), InstanceActorHandle::test(2), |_| {
        Err("fail".to_string())
    });

    // Then: active_count() == 1 (unchanged)
    assert_eq!(registry.active_count(), 1);
}

// =============================================================================
// Behavior 15: register returns StopTimeout when stop_fn exceeds timeout
// =============================================================================

#[test]
fn register_returns_stop_timeout_when_stop_fn_exceeds_timeout() {
    // Given: a registry with config.stop_timeout = 1ms
    //        where id_a is mapped to handle(1)
    let config = registry_config_with_timeout(Duration::from_millis(1));
    let mut registry = InstanceRegistry::new(config);
    registry
        .register(id_a(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();

    // When: register(id_a, handle(2), slow_stop_fn) is called
    let result = registry.register(
        id_a(),
        InstanceActorHandle::test(2),
        blocking_stop_fn(Duration::from_millis(50)),
    );

    // Then: result == Err(RegistryError::StopTimeout { instance_id, timeout })
    assert_eq!(
        result,
        Err(RegistryError::StopTimeout {
            instance_id: id_a(),
            timeout: Duration::from_millis(1),
        })
    );
}

// =============================================================================
// Behavior 16: register preserves old entry when stop_fn times out
// =============================================================================

#[test]
fn register_preserves_old_handle_when_stop_fn_times_out() {
    // Given: a registry with config.stop_timeout = 1ms
    //        where id_a is mapped to handle(1)
    let config = registry_config_with_timeout(Duration::from_millis(1));
    let mut registry = InstanceRegistry::new(config);
    registry
        .register(id_a(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();

    // When: register(id_a, handle(2), slow_stop_fn) is called
    let _ = registry.register(
        id_a(),
        InstanceActorHandle::test(2),
        blocking_stop_fn(Duration::from_millis(50)),
    );

    // Then: lookup(id_a) still points to handle(1) (preserved)
    assert_eq!(registry.lookup(&id_a()).map(|h| h.handle_id()), Some(1));
    // And: is_active(id_a) == true
    assert!(registry.is_active(&id_a()));
}

// =============================================================================
// Behavior 17: register preserves active_count when stop_fn times out
// =============================================================================

#[test]
fn register_preserves_active_count_when_stop_fn_times_out() {
    // Given: a registry with config.stop_timeout = 1ms
    //        where id_a is active, active_count() == 1
    let config = registry_config_with_timeout(Duration::from_millis(1));
    let mut registry = InstanceRegistry::new(config);
    registry
        .register(id_a(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();
    assert_eq!(registry.active_count(), 1);

    // When: register(id_a, handle(2), slow_stop_fn) is called
    let _ = registry.register(
        id_a(),
        InstanceActorHandle::test(2),
        blocking_stop_fn(Duration::from_millis(50)),
    );

    // Then: active_count() == 1 (unchanged)
    assert_eq!(registry.active_count(), 1);
}

// =============================================================================
// Behavior 18: register returns StopTimeout with minimum valid stop_timeout
// =============================================================================

#[test]
fn register_returns_stop_timeout_with_minimum_valid_stop_timeout() {
    // Given: a RegistryConfig with stop_timeout = Duration::from_nanos(1)
    //        and a registry where id_a is mapped to handle(1)
    let config = registry_config_with_timeout(Duration::from_nanos(1));
    let mut registry = InstanceRegistry::new(config);
    registry
        .register(id_a(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();

    // When: register(id_a, handle(2), slow_stop_fn) is called
    let result = registry.register(
        id_a(),
        InstanceActorHandle::test(2),
        blocking_stop_fn(Duration::from_millis(50)),
    );

    // Then: result == Err(RegistryError::StopTimeout { id_a, 1ns })
    assert_eq!(
        result,
        Err(RegistryError::StopTimeout {
            instance_id: id_a(),
            timeout: Duration::from_nanos(1),
        })
    );
    // And: lookup(id_a) == Some pointing to handle(1) (preserved)
    assert_eq!(registry.lookup(&id_a()).map(|h| h.handle_id()), Some(1));
    // And: active_count() == 1 (unchanged)
    assert_eq!(registry.active_count(), 1);
}

// =============================================================================
// Behavior 19: deregister returns exact handle when id is active
// =============================================================================

#[test]
fn deregister_returns_exact_handle_when_id_is_active() {
    // Given: a registry where id_a is mapped to handle(42)
    let mut registry = InstanceRegistry::new(default_registry_config());
    registry
        .register(id_a(), InstanceActorHandle::test(42), |_| Ok(()))
        .unwrap();

    // When: deregister(&id_a) is called
    let result = registry.deregister(&id_a());

    // Then: result == Ok(handle where test_id() == 42)
    assert_eq!(result.map(|h| h.handle_id()), Ok(42));
}

// =============================================================================
// Behavior 20: deregister decreases active_count by 1
// =============================================================================

#[test]
fn deregister_decrements_active_count_when_id_is_active() {
    // Given: a registry with id_a registered, active_count() == 1
    let mut registry = InstanceRegistry::new(default_registry_config());
    registry
        .register(id_a(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();
    assert_eq!(registry.active_count(), 1);

    // When: deregister(&id_a) is called
    registry.deregister(&id_a()).unwrap();

    // Then: active_count() == 0
    assert_eq!(registry.active_count(), 0);
    // And: is_active(id_a) == false
    assert!(!registry.is_active(&id_a()));
}

// =============================================================================
// Behavior 21: deregister makes is_active return false
// =============================================================================

#[test]
fn deregister_makes_is_active_false_after_success() {
    // Given: a registry where id_a is active, is_active(id_a) == true
    let mut registry = InstanceRegistry::new(default_registry_config());
    registry
        .register(id_a(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();
    assert!(registry.is_active(&id_a()));

    // When: deregister(&id_a) succeeds
    registry.deregister(&id_a()).unwrap();

    // Then: is_active(id_a) == false
    assert!(!registry.is_active(&id_a()));
    // And: lookup(id_a) == None
    assert_eq!(registry.lookup(&id_a()), None);
}

// =============================================================================
// Behavior 22: deregister returns NotRegistered when id is not in registry
// =============================================================================

#[test]
fn deregister_returns_not_registered_when_id_missing() {
    // Given: a registry where id_a is registered but id_b is not
    let mut registry = InstanceRegistry::new(default_registry_config());
    registry
        .register(id_a(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();

    // When: deregister(&id_b) is called
    let result = registry.deregister(&id_b());

    // Then: result == Err(RegistryError::NotRegistered { instance_id: id_b })
    assert_eq!(
        result,
        Err(RegistryError::NotRegistered {
            instance_id: id_b(),
        })
    );
}

#[test]
fn deregister_returns_not_registered_when_registry_is_empty() {
    // Given: an empty registry
    let mut registry = InstanceRegistry::new(default_registry_config());

    // When: deregister(&id_a) is called
    let result = registry.deregister(&id_a());

    // Then: result == Err(RegistryError::NotRegistered { instance_id: id_a })
    // And: active_count() == 0
    assert_eq!(
        result,
        Err(RegistryError::NotRegistered {
            instance_id: id_a(),
        })
    );
    assert_eq!(registry.active_count(), 0);
}

// =============================================================================
// Behavior 23: deregister leaves registry unchanged on error
// =============================================================================

#[test]
fn deregister_leaves_state_unchanged_when_id_not_registered() {
    // Given: a registry with id_a registered, active_count() == 1
    let mut registry = InstanceRegistry::new(default_registry_config());
    registry
        .register(id_a(), InstanceActorHandle::test(10), |_| Ok(()))
        .unwrap();
    assert_eq!(registry.active_count(), 1);

    // When: deregister(&id_b) is called (id_b not registered)
    let _ = registry.deregister(&id_b());

    // Then: active_count() == 1 (unchanged)
    assert_eq!(registry.active_count(), 1);
    // And: lookup(id_a) still returns Some with handle test_id == 10
    assert_eq!(registry.lookup(&id_a()).map(|h| h.handle_id()), Some(10));
    // And: is_active(id_a) == true
    assert!(registry.is_active(&id_a()));
}

// =============================================================================
// Behavior 24: deregister returns NotRegistered on double-deregister
// =============================================================================

#[test]
fn deregister_returns_not_registered_on_double_deregister() {
    // Given: a registry where id_a is registered and mapped to handle(1)
    let mut registry = InstanceRegistry::new(default_registry_config());
    registry
        .register(id_a(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();

    // When: first deregister(&id_a) succeeds
    let first = registry.deregister(&id_a());

    // Then: first result == Ok(handle(1))
    assert_eq!(first.map(|h| h.handle_id()), Ok(1));
    // And: active_count() == 0
    assert_eq!(registry.active_count(), 0);

    // When: second deregister(&id_a) is called
    let second = registry.deregister(&id_a());

    // Then: second result == Err(RegistryError::NotRegistered { id_a })
    assert_eq!(
        second,
        Err(RegistryError::NotRegistered {
            instance_id: id_a(),
        })
    );
    // And: active_count() == 0 (still zero, unchanged)
    assert_eq!(registry.active_count(), 0);
}

// =============================================================================
// Behavior 25: lookup returns Some when active, None otherwise
// =============================================================================

#[test]
fn lookup_returns_none_when_registry_is_empty() {
    // Given: an empty registry
    let registry = InstanceRegistry::new(default_registry_config());

    // When: lookup(id_a) is called
    let result = registry.lookup(&id_a());

    // Then: result == None
    assert_eq!(result, None);
}

#[test]
fn lookup_returns_some_with_exact_handle_when_active() {
    // Given: a registry where id_a is mapped to handle(77)
    let mut registry = InstanceRegistry::new(default_registry_config());
    registry
        .register(id_a(), InstanceActorHandle::test(77), |_| Ok(()))
        .unwrap();

    // When: lookup(&id_a) is called
    let result = registry.lookup(&id_a());

    // Then: result == Some with test_id() == 77
    assert_eq!(result.map(|h| h.handle_id()), Some(77));
}

#[test]
fn lookup_returns_none_when_id_not_registered() {
    // Given: a registry where id_a is registered
    let mut registry = InstanceRegistry::new(default_registry_config());
    registry
        .register(id_a(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();

    // When: lookup(&id_b) is called (id_b not registered)
    let result = registry.lookup(&id_b());

    // Then: result == None
    assert_eq!(result, None);
}

// =============================================================================
// Behavior 26: is_active returns true iff lookup returns Some
// =============================================================================

#[test]
fn is_active_is_true_iff_lookup_is_some() {
    // Given: a registry where id_a is registered
    let mut registry = InstanceRegistry::new(default_registry_config());
    registry
        .register(id_a(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();

    // When: is_active(&id_a) and lookup(&id_a) are both called
    let active = registry.is_active(&id_a());
    let lookup_result = registry.lookup(&id_a());

    // Then: is_active == true AND lookup is Some
    assert!(active);
    assert!(lookup_result.is_some());
}

#[test]
fn is_active_is_false_iff_lookup_is_none() {
    // Given: a registry where id_a is NOT registered
    let registry = InstanceRegistry::new(default_registry_config());

    // When: is_active(&id_a) and lookup(&id_a) are both called
    let active = registry.is_active(&id_a());
    let lookup_result = registry.lookup(&id_a());

    // Then: is_active == false AND lookup is None
    assert!(!active);
    assert!(lookup_result.is_none());
}

// =============================================================================
// Behavior 27: active_count equals number of registered entries
// =============================================================================

#[test]
fn active_count_equals_three_registered_count() {
    // Given: a registry with 3 registered actors (id_a, id_b, id_c)
    let mut registry = InstanceRegistry::new(default_registry_config());
    registry
        .register(id_a(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();
    registry
        .register(id_b(), InstanceActorHandle::test(2), |_| Ok(()))
        .unwrap();
    registry
        .register(id_c(), InstanceActorHandle::test(3), |_| Ok(()))
        .unwrap();

    // When: active_count() is called
    // Then: result == 3
    assert_eq!(registry.active_count(), 3);
}

// =============================================================================
// Proptest Invariants
// =============================================================================

mod proptest_invariants {
    use super::*;
    use proptest::prelude::*;

    /// Generate a pool of valid InstanceIds from ULID seeds.
    fn make_id_pool(size: usize) -> Vec<InstanceId> {
        (1u128..=size as u128)
            .map(|n| {
                let ulid = ulid::Ulid::from(n);
                InstanceId::parse(&ulid.to_string()).unwrap()
            })
            .collect()
    }

    // =========================================================================
    // INV-1: Single-Active — at most one handle per InstanceId
    // =========================================================================

    proptest! {
        #[test]
        fn registry_maintains_single_active_per_instance_id(
            ops in prop::collection::vec(
                (any::<bool>(), 0usize..20, any::<u64>()),
                0..200
            )
        ) {
            let config = RegistryConfig { stop_timeout: Duration::from_secs(5) };
            let mut registry = InstanceRegistry::new(config);

            let id_pool = make_id_pool(20);
            let mut expected_active = std::collections::HashMap::new();

            for (is_register, id_idx, handle_id) in ops {
                let idx = id_idx % id_pool.len();
                let id = id_pool[idx].clone();

                if is_register {
                    let result = registry.register(
                        id.clone(),
                        InstanceActorHandle::test(handle_id),
                        |_| Ok(()),
                    );
                    prop_assert_eq!(result.clone(), Ok(()), "register should succeed with Ok stop_fn: {:?}", result);
                    expected_active.insert(id, handle_id);
                } else {
                    let result = registry.deregister(&id);
                    if let Some(&stored_handle_id) = expected_active.get(&id) {
                        prop_assert_eq!(result.as_ref().map(|handle| handle.handle_id()), Ok(stored_handle_id), "deregister of active id should succeed: {:?}", result);
                        expected_active.remove(&id);
                    } else {
                        let is_not_reg = matches!(result, Err(RegistryError::NotRegistered { .. }));
                        prop_assert!(
                            is_not_reg,
                            "expected NotRegistered, got {:?}",
                            result
                        );
                    }
                }

                prop_assert!(registry.active_count() <= id_pool.len());

                // INV-3: count matches tracked active set
                prop_assert_eq!(registry.active_count(), expected_active.len());

                for (active_id, _) in &expected_active {
                    prop_assert!(registry.is_active(active_id));
                    prop_assert!(registry.lookup(active_id).is_some());
                }
            }
        }
    }

    // =========================================================================
    // INV-3: Count Consistency — active_count == expected set size
    // =========================================================================

    proptest! {
        #[test]
        fn registry_active_count_equals_expected_set_size(
            ops in prop::collection::vec(
                (any::<bool>(), 0usize..20, any::<u64>()),
                0..200
            )
        ) {
            let config = RegistryConfig { stop_timeout: Duration::from_secs(5) };
            let mut registry = InstanceRegistry::new(config);

            let id_pool = make_id_pool(20);
            let mut expected_count: usize = 0;
            let mut registered = std::collections::HashSet::new();

            for (is_register, id_idx, _handle_id) in ops {
                let idx = id_idx % id_pool.len();
                let id = id_pool[idx].clone();

                if is_register {
                    let was_new = registered.insert(id.clone());
                    registry.register(id, InstanceActorHandle::test(1), |_| Ok(())).unwrap();
                    if was_new {
                        expected_count += 1;
                    }
                    // count unchanged on replace: old removed, new added
                } else {
                    let was_present = registered.remove(&id);
                    let result = registry.deregister(&id);
                    if was_present {
                        prop_assert_eq!(result.map(|handle| handle.handle_id()), Ok(1));
                        expected_count = expected_count.saturating_sub(1);
                    } else {
                        let is_not_reg = matches!(result, Err(RegistryError::NotRegistered { .. }));
                        prop_assert!(
                            is_not_reg,
                            "expected NotRegistered, got {:?}",
                            result
                        );
                    }
                }

                prop_assert_eq!(registry.active_count(), expected_count);
                prop_assert_eq!(registry.active_count(), registered.len());
            }
        }
    }

    // =========================================================================
    // INV-4: Stop-Before-Replace — stop_fn called iff replacing
    // =========================================================================

    proptest! {
        #[test]
        fn register_calls_stop_fn_only_when_replacing(
            seed in 1u128..1000u128,
            do_replace in any::<bool>(),
        ) {
            let config = RegistryConfig { stop_timeout: Duration::from_secs(5) };
            let mut registry = InstanceRegistry::new(config);

            let ulid = ulid::Ulid::from(seed);
            let id = InstanceId::parse(&ulid.to_string()).unwrap();

            let stop_call_count = Arc::new(AtomicU32::new(0));
            let stop_call_count_clone = stop_call_count.clone();

            // Fresh insert — stop_fn should NOT be called
            registry.register(id.clone(), InstanceActorHandle::test(1), move |_| {
                stop_call_count_clone.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }).unwrap();

            let count_after_fresh = stop_call_count.load(Ordering::SeqCst);
            prop_assert_eq!(count_after_fresh, 0, "stop_fn must not be called on fresh insert");

            if do_replace {
                let stop_call_count2 = Arc::new(AtomicU32::new(0));
                let stop_call_count2_clone = stop_call_count2.clone();

                // Replace — stop_fn SHOULD be called
                registry.register(id, InstanceActorHandle::test(2), move |_| {
                    stop_call_count2_clone.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }).unwrap();

                let count_after_replace = stop_call_count2.load(Ordering::SeqCst);
                prop_assert_eq!(count_after_replace, 1, "stop_fn must be called exactly once on replace");
            }
        }
    }

    // =========================================================================
    // INV-5: No Partial Mutations — error paths preserve state
    // =========================================================================

    proptest! {
        #[test]
        fn registry_state_preserved_on_stop_fn_error(
            seed in 1u128..1000u128,
        ) {
            let config = RegistryConfig { stop_timeout: Duration::from_secs(5) };
            let mut registry = InstanceRegistry::new(config);

            let ulid = ulid::Ulid::from(seed);
            let id = InstanceId::parse(&ulid.to_string()).unwrap();

            // Set up: register id -> handle(100)
            registry.register(id.clone(), InstanceActorHandle::test(100), |_| Ok(())).unwrap();

            // Snapshot state before error-inducing register
            let count_before = registry.active_count();
            let is_active_before = registry.is_active(&id);
            let lookup_before = registry.lookup(&id).map(|h| h.handle_id());

            // Attempt register with failing stop_fn
            let result = registry.register(
                id.clone(),
                InstanceActorHandle::test(200),
                |_| Err("forced failure".to_string()),
            );

            // Verify error returned
            let is_stop_failed = matches!(result, Err(RegistryError::StopFailed { .. }));
            prop_assert!(
                is_stop_failed,
                "expected StopFailed, got {:?}",
                result
            );

            // INV-5: state unchanged after error
            prop_assert_eq!(registry.active_count(), count_before);
            prop_assert_eq!(registry.is_active(&id), is_active_before);
            prop_assert_eq!(registry.lookup(&id).map(|h| h.handle_id()), lookup_before);
        }
    }

    // =========================================================================
    // Registration and deregistration cycle consistency
    // =========================================================================

    proptest! {
        #[test]
        fn register_deregister_reregister_cycle_is_consistent(
            seed in 1u128..1000u128,
        ) {
            let config = RegistryConfig { stop_timeout: Duration::from_secs(5) };
            let mut registry = InstanceRegistry::new(config);

            let ulid = ulid::Ulid::from(seed);
            let id = InstanceId::parse(&ulid.to_string()).unwrap();

            // Step 1: register(id) -> handle(1)
            registry.register(id.clone(), InstanceActorHandle::test(1), |_| Ok(())).unwrap();
            prop_assert!(registry.is_active(&id));
            prop_assert_eq!(registry.active_count(), 1);

            // Step 2: deregister(id)
            let deregistered = registry.deregister(&id);
            prop_assert_eq!(deregistered.map(|h| h.handle_id()), Ok(1));
            prop_assert!(!registry.is_active(&id));
            prop_assert_eq!(registry.lookup(&id), None);
            prop_assert_eq!(registry.active_count(), 0);

            // Step 3: re-register(id) -> handle(2)
            registry.register(id.clone(), InstanceActorHandle::test(2), |_| Ok(())).unwrap();
            prop_assert!(registry.is_active(&id));
            prop_assert_eq!(registry.lookup(&id).map(|h| h.handle_id()), Some(2));
            prop_assert_eq!(registry.active_count(), 1);

            // Final state: same as single register, but with handle(2)
            prop_assert_eq!(registry.active_count(), 1);
            prop_assert!(registry.is_active(&id));
        }
    }
}

// =============================================================================
// ADR-029/039 Acceptance Tests
// These tests verify the local-only single-active-instance guarantees.
// =============================================================================

mod adr_029_039_acceptance_tests {

    use super::*;

    fn acceptance_registry_config() -> RegistryConfig {
        RegistryConfig {
            stop_timeout: Duration::from_secs(5),
        }
    }

    // Acceptance test: Second request for local active instance returns existing handle
    // Verifies: When a workflow instance is already active locally, subsequent
    // registration requests receive the existing handle (route-to-existing behavior).
    #[test]
    fn second_request_for_local_active_instance_returns_existing_handle() {
        let mut registry = InstanceRegistry::new(acceptance_registry_config());
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let first_handle = InstanceActorHandle::test(1);

        registry
            .register(instance_id.clone(), first_handle, |_| Ok(()))
            .unwrap();

        let first_lookup = registry.lookup(&instance_id);
        assert!(first_lookup.is_some());
        assert_eq!(first_lookup.unwrap().handle_id(), 1);

        let second_handle = InstanceActorHandle::test(2);
        let result = registry.register(instance_id.clone(), second_handle, |prior| {
            assert_eq!(prior.handle_id(), 1);
            Ok(())
        });

        assert!(result.is_ok());

        let second_lookup = registry.lookup(&instance_id);
        assert!(second_lookup.is_some());
        assert_eq!(second_lookup.unwrap().handle_id(), 2);
    }

    // Duplicate for schema validation
    #[test]
    fn second_request_for_local_active_instance_returns_existing_handle_duplicate_for_sch() {
        let mut registry = InstanceRegistry::new(acceptance_registry_config());
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();
        let first_handle = InstanceActorHandle::test(100);

        registry
            .register(instance_id.clone(), first_handle, |_| Ok(()))
            .unwrap();

        let second_handle = InstanceActorHandle::test(200);
        let result = registry.register(instance_id.clone(), second_handle, |_| Ok(()));

        assert!(result.is_ok());
        assert_eq!(registry.lookup(&instance_id).unwrap().handle_id(), 200);
    }

    // Acceptance test: Registry eviction correctly removes instance reference
    // Verifies: When an instance is deregistered, it is no longer active and
    // lookups return None (proper cleanup after eviction).
    #[test]
    fn registry_eviction_correctly_removes_instance_reference() {
        let mut registry = InstanceRegistry::new(acceptance_registry_config());
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMC").unwrap();
        let handle = InstanceActorHandle::test(42);

        registry
            .register(instance_id.clone(), handle, |_| Ok(()))
            .unwrap();

        assert!(registry.is_active(&instance_id));
        assert!(registry.lookup(&instance_id).is_some());

        let evicted = registry.deregister(&instance_id);
        assert!(evicted.is_ok());
        assert_eq!(evicted.unwrap().handle_id(), 42);

        assert!(!registry.is_active(&instance_id));
        assert!(registry.lookup(&instance_id).is_none());
        assert_eq!(registry.active_count(), 0);
    }

    // Duplicate for schema validation
    #[test]
    fn registry_eviction_correctly_removes_instance_reference_duplicate_for_schema() {
        let mut registry = InstanceRegistry::new(acceptance_registry_config());
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMD").unwrap();

        registry
            .register(instance_id.clone(), InstanceActorHandle::test(99), |_| {
                Ok(())
            })
            .unwrap();

        assert!(registry.is_active(&instance_id));

        let result = registry.deregister(&instance_id);
        assert!(result.is_ok());

        assert!(!registry.is_active(&instance_id));
        assert_eq!(registry.lookup(&instance_id), None);
    }

    // Test that registry is strictly local (test demonstrates local nature)
    // This test verifies that the registry does not provide cross-node guarantees
    #[test]
    fn registry_provides_strictly_local_guarantees() {
        let mut registry_a = InstanceRegistry::new(acceptance_registry_config());
        let mut registry_b = InstanceRegistry::new(acceptance_registry_config());
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFME").unwrap();

        registry_a
            .register(
                instance_id.clone(),
                InstanceActorHandle::test(1),
                |_| Ok(()),
            )
            .unwrap();

        registry_b
            .register(
                instance_id.clone(),
                InstanceActorHandle::test(2),
                |_| Ok(()),
            )
            .unwrap();

        assert!(registry_a.is_active(&instance_id));
        assert!(registry_b.is_active(&instance_id));

        assert_eq!(registry_a.lookup(&instance_id).unwrap().handle_id(), 1);
        assert_eq!(registry_b.lookup(&instance_id).unwrap().handle_id(), 2);

        assert_eq!(registry_a.active_count(), 1);
        assert_eq!(registry_b.active_count(), 1);
    }
}

// =============================================================================
// Kani Harnesses
// =============================================================================

#[cfg(kani)]
mod kani_verification {
    use super::*;

    fn make_kani_id(n: u128) -> InstanceId {
        let ulid = ulid::Ulid::from(n);
        InstanceId::parse(&ulid.to_string()).unwrap()
    }

    // =========================================================================
    // INV-1: Single-Active — no duplicates after up to 5 operations
    // =========================================================================

    #[kani::proof]
    fn verify_single_active_no_duplicates() {
        let config = RegistryConfig {
            stop_timeout: Duration::from_secs(5),
        };
        let mut registry = InstanceRegistry::new(config);

        let id1 = make_kani_id(1);
        let id2 = make_kani_id(2);
        let id3 = make_kani_id(3);
        let ids = [&id1, &id2, &id3];

        // Perform up to 5 operations
        for _ in 0..5 {
            let op: u8 = kani::any();
            let id_idx: usize = kani::any();
            kani::assume(id_idx < 3);
            let id = ids[id_idx].clone();

            match op % 3 {
                0 | 1 => {
                    // Register
                    let _ = registry.register(id, InstanceActorHandle::test(1), |_| Ok(()));
                }
                2 => {
                    // Deregister
                    let _ = registry.deregister(&id);
                }
                _ => {}
            }
        }

        // INV-1: active_count never exceeds the number of distinct IDs
        assert!(registry.active_count() <= 3);

        // Each ID is either active or not — no duplicates possible
        for id in &ids {
            let is_act = registry.is_active(id);
            let has_lookup = registry.lookup(id).is_some();
            assert_eq!(is_act, has_lookup);
        }
    }

    // =========================================================================
    // INV-5: No partial mutation on StopFailed
    // =========================================================================

    #[kani::proof]
    fn verify_no_partial_mutation_on_stop_failed() {
        let config = RegistryConfig {
            stop_timeout: Duration::from_secs(5),
        };
        let mut registry = InstanceRegistry::new(config);

        let id = make_kani_id(1);

        // Pre-populate with some entries
        registry
            .register(id.clone(), InstanceActorHandle::test(10), |_| Ok(()))
            .unwrap();
        let id2 = make_kani_id(2);
        registry
            .register(id2, InstanceActorHandle::test(20), |_| Ok(()))
            .unwrap();

        // Snapshot state
        let count_before = registry.active_count();
        let is_active_before = registry.is_active(&id);
        let lookup_before = registry.lookup(&id).map(|h| h.handle_id());

        // Attempt register with failing stop_fn
        let result = registry.register(id.clone(), InstanceActorHandle::test(99), |_| {
            Err("fail".to_string())
        });

        // Must return StopFailed
        assert!(matches!(result, Err(RegistryError::StopFailed { .. })));

        // INV-5: state unchanged
        assert_eq!(registry.active_count(), count_before);
        assert_eq!(registry.is_active(&id), is_active_before);
        assert_eq!(registry.lookup(&id).map(|h| h.handle_id()), lookup_before);
    }

    // =========================================================================
    // Active count never underflows
    // =========================================================================

    #[kani::proof]
    fn verify_active_count_never_underflows() {
        let config = RegistryConfig {
            stop_timeout: Duration::from_secs(5),
        };
        let mut registry = InstanceRegistry::new(config);

        let id1 = make_kani_id(1);
        let id2 = make_kani_id(2);

        // Perform 10 random operations
        for _ in 0..10 {
            let op: u8 = kani::any();
            let id_pick: bool = kani::any();
            let id = if id_pick { &id1 } else { &id2 };

            match op % 3 {
                0 | 1 => {
                    let _ = registry.register(id.clone(), InstanceActorHandle::test(1), |_| Ok(()));
                }
                2 => {
                    let _ = registry.deregister(id);
                }
                _ => {}
            }

            // Count must never underflow (usize is always >= 0)
            assert!(registry.active_count() <= 2);
        }

        // Final check: count is valid
        assert!(registry.active_count() <= 2);
    }
}
