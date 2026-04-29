use super::*;

fn acceptance_registry_config() -> RegistryConfig {
    RegistryConfig {
        stop_timeout: Duration::from_secs(5),
    }
}

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
