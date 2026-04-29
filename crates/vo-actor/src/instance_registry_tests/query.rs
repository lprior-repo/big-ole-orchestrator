use super::*;

#[test]
fn lookup_returns_none_when_registry_is_empty() {
    let registry = InstanceRegistry::new(default_registry_config());
    let result = registry.lookup(&id_a());
    assert_eq!(result, None);
}

#[test]
fn lookup_returns_some_with_exact_handle_when_active() {
    let mut registry = InstanceRegistry::new(default_registry_config());
    registry
        .register(id_a(), InstanceActorHandle::test(77), |_| Ok(()))
        .unwrap();
    let result = registry.lookup(&id_a());
    assert_eq!(result.map(|h| h.handle_id()), Some(77));
}

#[test]
fn lookup_returns_none_when_id_not_registered() {
    let mut registry = InstanceRegistry::new(default_registry_config());
    registry
        .register(id_a(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();
    let result = registry.lookup(&id_b());
    assert_eq!(result, None);
}

#[test]
fn is_active_is_true_iff_lookup_is_some() {
    let mut registry = InstanceRegistry::new(default_registry_config());
    registry
        .register(id_a(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();
    let active = registry.is_active(&id_a());
    let lookup_result = registry.lookup(&id_a());
    assert!(active);
    assert!(lookup_result.is_some());
}

#[test]
fn is_active_is_false_iff_lookup_is_none() {
    let registry = InstanceRegistry::new(default_registry_config());
    let active = registry.is_active(&id_a());
    let lookup_result = registry.lookup(&id_a());
    assert!(!active);
    assert!(lookup_result.is_none());
}

#[test]
fn active_count_equals_three_registered_count() {
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
    assert_eq!(registry.active_count(), 3);
}
