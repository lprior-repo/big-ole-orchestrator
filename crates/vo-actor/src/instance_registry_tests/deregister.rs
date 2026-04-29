use super::*;

#[test]
fn deregister_returns_exact_handle_when_id_is_active() {
    let mut registry = InstanceRegistry::new(default_registry_config());
    registry
        .register(id_a(), InstanceActorHandle::test(42), |_| Ok(()))
        .unwrap();
    let result = registry.deregister(&id_a());
    assert_eq!(result.map(|h| h.handle_id()), Ok(42));
}

#[test]
fn deregister_decrements_active_count_when_id_is_active() {
    let mut registry = InstanceRegistry::new(default_registry_config());
    registry
        .register(id_a(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();
    assert_eq!(registry.active_count(), 1);
    registry.deregister(&id_a()).unwrap();
    assert_eq!(registry.active_count(), 0);
    assert!(!registry.is_active(&id_a()));
}

#[test]
fn deregister_makes_is_active_false_after_success() {
    let mut registry = InstanceRegistry::new(default_registry_config());
    registry
        .register(id_a(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();
    assert!(registry.is_active(&id_a()));
    registry.deregister(&id_a()).unwrap();
    assert!(!registry.is_active(&id_a()));
    assert_eq!(registry.lookup(&id_a()), None);
}

#[test]
fn deregister_returns_not_registered_when_id_missing() {
    let mut registry = InstanceRegistry::new(default_registry_config());
    registry
        .register(id_a(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();
    let result = registry.deregister(&id_b());
    assert_eq!(
        result,
        Err(RegistryError::NotRegistered {
            instance_id: id_b(),
        })
    );
}

#[test]
fn deregister_returns_not_registered_when_registry_is_empty() {
    let mut registry = InstanceRegistry::new(default_registry_config());
    let result = registry.deregister(&id_a());
    assert_eq!(
        result,
        Err(RegistryError::NotRegistered {
            instance_id: id_a(),
        })
    );
    assert_eq!(registry.active_count(), 0);
}

#[test]
fn deregister_leaves_state_unchanged_when_id_not_registered() {
    let mut registry = InstanceRegistry::new(default_registry_config());
    registry
        .register(id_a(), InstanceActorHandle::test(10), |_| Ok(()))
        .unwrap();
    assert_eq!(registry.active_count(), 1);
    let _ = registry.deregister(&id_b());
    assert_eq!(registry.active_count(), 1);
    assert_eq!(registry.lookup(&id_a()).map(|h| h.handle_id()), Some(10));
    assert!(registry.is_active(&id_a()));
}

#[test]
fn deregister_returns_not_registered_on_double_deregister() {
    let mut registry = InstanceRegistry::new(default_registry_config());
    registry
        .register(id_a(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();
    let first = registry.deregister(&id_a());
    assert_eq!(first.map(|h| h.handle_id()), Ok(1));
    assert_eq!(registry.active_count(), 0);
    let second = registry.deregister(&id_a());
    assert_eq!(
        second,
        Err(RegistryError::NotRegistered {
            instance_id: id_a(),
        })
    );
    assert_eq!(registry.active_count(), 0);
}
