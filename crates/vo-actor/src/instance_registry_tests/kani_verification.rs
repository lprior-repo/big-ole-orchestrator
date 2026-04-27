use super::*;

fn make_kani_id(n: u128) -> InstanceId {
    let ulid = ulid::Ulid::from(n);
    InstanceId::parse(&ulid.to_string()).unwrap()
}

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

    for _ in 0..5 {
        let op: u8 = kani::any();
        let id_idx: usize = kani::any();
        kani::assume(id_idx < 3);
        let id = ids[id_idx].clone();

        match op % 3 {
            0 | 1 => {
                let _ = registry.register(id, InstanceActorHandle::test(1), |_| Ok(()));
            }
            2 => {
                let _ = registry.deregister(&id);
            }
            _ => {}
        }
    }

    assert!(registry.active_count() <= 3);

    for id in &ids {
        let is_act = registry.is_active(id);
        let has_lookup = registry.lookup(id).is_some();
        assert_eq!(is_act, has_lookup);
    }
}

#[kani::proof]
fn verify_no_partial_mutation_on_stop_failed() {
    let config = RegistryConfig {
        stop_timeout: Duration::from_secs(5),
    };
    let mut registry = InstanceRegistry::new(config);

    let id = make_kani_id(1);

    registry
        .register(id.clone(), InstanceActorHandle::test(10), |_| Ok(()))
        .unwrap();
    let id2 = make_kani_id(2);
    registry
        .register(id2, InstanceActorHandle::test(20), |_| Ok(()))
        .unwrap();

    let count_before = registry.active_count();
    let is_active_before = registry.is_active(&id);
    let lookup_before = registry.lookup(&id).map(|h| h.handle_id());

    let result = registry.register(id.clone(), InstanceActorHandle::test(99), |_| {
        Err("fail".to_string())
    });

    assert!(matches!(result, Err(RegistryError::StopFailed { .. })));

    assert_eq!(registry.active_count(), count_before);
    assert_eq!(registry.is_active(&id), is_active_before);
    assert_eq!(registry.lookup(&id).map(|h| h.handle_id()), lookup_before);
}

#[kani::proof]
fn verify_active_count_never_underflows() {
    let config = RegistryConfig {
        stop_timeout: Duration::from_secs(5),
    };
    let mut registry = InstanceRegistry::new(config);

    let id1 = make_kani_id(1);
    let id2 = make_kani_id(2);

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

        assert!(registry.active_count() <= 2);
    }

    assert!(registry.active_count() <= 2);
}
