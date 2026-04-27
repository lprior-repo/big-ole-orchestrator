use super::*;

#[test]
fn register_returns_ok_and_inserts_handle_when_id_not_active() {
    let mut registry = InstanceRegistry::new(default_registry_config());
    let id = id_a();
    let handle = InstanceActorHandle::test(42);
    let result = registry.register(id.clone(), handle, |_| Ok(()));
    assert_eq!(result, Ok(()));
    assert_eq!(registry.lookup(&id).map(|h| h.handle_id()), Some(42));
    assert!(registry.is_active(&id));
}

#[test]
fn register_increments_active_count_when_id_not_active() {
    let mut registry = InstanceRegistry::new(default_registry_config());
    assert_eq!(registry.active_count(), 0);
    registry
        .register(id_a(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();
    assert_eq!(registry.active_count(), 1);
}

#[test]
fn register_makes_is_active_true_when_id_not_active() {
    let mut registry = InstanceRegistry::new(default_registry_config());
    assert!(!registry.is_active(&id_a()));
    registry
        .register(id_a(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();
    assert!(registry.is_active(&id_a()));
}

#[test]
fn register_makes_lookup_return_some_with_exact_handle_when_id_not_active() {
    let mut registry = InstanceRegistry::new(default_registry_config());
    assert_eq!(registry.lookup(&id_a()), None);
    registry
        .register(id_a(), InstanceActorHandle::test(42), |_| Ok(()))
        .unwrap();
    assert_eq!(registry.lookup(&id_a()).map(|h| h.handle_id()), Some(42));
}

#[test]
fn register_calls_stop_fn_with_prior_handle_when_id_already_active() {
    let mut registry = InstanceRegistry::new(default_registry_config());
    registry
        .register(id_a(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();
    let captured_id = Arc::new(AtomicU64::new(0));
    let captured_clone = captured_id.clone();
    let result = registry.register(id_a(), InstanceActorHandle::test(2), move |prior| {
        captured_clone.store(prior.handle_id(), Ordering::SeqCst);
        Ok(())
    });
    assert_eq!(result, Ok(()));
    assert_eq!(captured_id.load(Ordering::SeqCst), 1);
}

#[test]
fn register_replaces_handle_when_id_active_and_stop_fn_succeeds() {
    let mut registry = InstanceRegistry::new(default_registry_config());
    registry
        .register(id_a(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();
    let result = registry.register(id_a(), InstanceActorHandle::test(2), |_| Ok(()));
    assert_eq!(result, Ok(()));
    assert_eq!(registry.lookup(&id_a()).map(|h| h.handle_id()), Some(2));
}

#[test]
fn register_keeps_active_count_unchanged_when_stop_fn_succeeds() {
    let mut registry = InstanceRegistry::new(default_registry_config());
    registry
        .register(id_a(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();
    assert_eq!(registry.active_count(), 1);
    registry
        .register(id_a(), InstanceActorHandle::test(2), |_| Ok(()))
        .unwrap();
    assert_eq!(registry.active_count(), 1);
}

#[test]
fn register_lookup_returns_new_handle_after_successful_replace() {
    let mut registry = InstanceRegistry::new(default_registry_config());
    registry
        .register(id_a(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();
    registry
        .register(id_a(), InstanceActorHandle::test(99), |_| Ok(()))
        .unwrap();
    assert_eq!(registry.lookup(&id_a()).map(|h| h.handle_id()), Some(99));
    assert!(registry.is_active(&id_a()));
}
