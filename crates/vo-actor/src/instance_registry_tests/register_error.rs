use super::*;

#[test]
fn register_returns_stop_failed_when_stop_fn_returns_err() {
    let mut registry = InstanceRegistry::new(default_registry_config());
    registry
        .register(id_a(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();
    let result = registry.register(id_a(), InstanceActorHandle::test(2), |_| {
        Err("actor stuck".to_string())
    });
    assert_eq!(
        result,
        Err(RegistryError::StopFailed {
            instance_id: id_a(),
            reason: "actor stuck".to_string(),
        })
    );
}

#[test]
fn register_preserves_old_handle_when_stop_fn_returns_err() {
    let mut registry = InstanceRegistry::new(default_registry_config());
    registry
        .register(id_a(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();
    let _ = registry.register(id_a(), InstanceActorHandle::test(2), |_| {
        Err("fail".to_string())
    });
    assert_eq!(registry.lookup(&id_a()).map(|h| h.handle_id()), Some(1));
    assert!(registry.is_active(&id_a()));
}

#[test]
fn register_preserves_active_count_when_stop_fn_returns_err() {
    let mut registry = InstanceRegistry::new(default_registry_config());
    registry
        .register(id_a(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();
    assert_eq!(registry.active_count(), 1);
    let _ = registry.register(id_a(), InstanceActorHandle::test(2), |_| {
        Err("fail".to_string())
    });
    assert_eq!(registry.active_count(), 1);
}

#[test]
fn register_returns_stop_timeout_when_stop_fn_exceeds_timeout() {
    let config = registry_config_with_timeout(Duration::from_millis(1));
    let mut registry = InstanceRegistry::new(config);
    registry
        .register(id_a(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();
    let result = registry.register(
        id_a(),
        InstanceActorHandle::test(2),
        blocking_stop_fn(Duration::from_millis(50)),
    );
    assert_eq!(
        result,
        Err(RegistryError::StopTimeout {
            instance_id: id_a(),
            timeout: Duration::from_millis(1),
        })
    );
}

#[test]
fn register_preserves_old_handle_when_stop_fn_times_out() {
    let config = registry_config_with_timeout(Duration::from_millis(1));
    let mut registry = InstanceRegistry::new(config);
    registry
        .register(id_a(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();
    let _ = registry.register(
        id_a(),
        InstanceActorHandle::test(2),
        blocking_stop_fn(Duration::from_millis(50)),
    );
    assert_eq!(registry.lookup(&id_a()).map(|h| h.handle_id()), Some(1));
    assert!(registry.is_active(&id_a()));
}

#[test]
fn register_preserves_active_count_when_stop_fn_times_out() {
    let config = registry_config_with_timeout(Duration::from_millis(1));
    let mut registry = InstanceRegistry::new(config);
    registry
        .register(id_a(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();
    assert_eq!(registry.active_count(), 1);
    let _ = registry.register(
        id_a(),
        InstanceActorHandle::test(2),
        blocking_stop_fn(Duration::from_millis(50)),
    );
    assert_eq!(registry.active_count(), 1);
}

#[test]
fn register_returns_stop_timeout_with_minimum_valid_stop_timeout() {
    let config = registry_config_with_timeout(Duration::from_nanos(1));
    let mut registry = InstanceRegistry::new(config);
    registry
        .register(id_a(), InstanceActorHandle::test(1), |_| Ok(()))
        .unwrap();
    let result = registry.register(
        id_a(),
        InstanceActorHandle::test(2),
        blocking_stop_fn(Duration::from_millis(50)),
    );
    assert_eq!(
        result,
        Err(RegistryError::StopTimeout {
            instance_id: id_a(),
            timeout: Duration::from_nanos(1),
        })
    );
    assert_eq!(registry.lookup(&id_a()).map(|h| h.handle_id()), Some(1));
    assert_eq!(registry.active_count(), 1);
}
