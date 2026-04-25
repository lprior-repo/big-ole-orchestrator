use super::*;
use proptest::prelude::*;

fn make_id_pool(size: usize) -> Vec<InstanceId> {
    (1u128..=size as u128)
        .map(|n| {
            let ulid = ulid::Ulid::from(n);
            InstanceId::parse(&ulid.to_string()).unwrap()
        })
        .collect()
}

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
            prop_assert_eq!(registry.active_count(), expected_active.len());

            for (active_id, _) in &expected_active {
                prop_assert!(registry.is_active(active_id));
                prop_assert!(registry.lookup(active_id).is_some());
            }
        }
    }
}

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

        registry.register(id.clone(), InstanceActorHandle::test(1), move |_| {
            stop_call_count_clone.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }).unwrap();

        let count_after_fresh = stop_call_count.load(Ordering::SeqCst);
        prop_assert_eq!(count_after_fresh, 0, "stop_fn must not be called on fresh insert");

        if do_replace {
            let stop_call_count2 = Arc::new(AtomicU32::new(0));
            let stop_call_count2_clone = stop_call_count2.clone();

            registry.register(id, InstanceActorHandle::test(2), move |_| {
                stop_call_count2_clone.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }).unwrap();

            let count_after_replace = stop_call_count2.load(Ordering::SeqCst);
            prop_assert_eq!(count_after_replace, 1, "stop_fn must be called exactly once on replace");
        }
    }
}

proptest! {
    #[test]
    fn registry_state_preserved_on_stop_fn_error(
        seed in 1u128..1000u128,
    ) {
        let config = RegistryConfig { stop_timeout: Duration::from_secs(5) };
        let mut registry = InstanceRegistry::new(config);

        let ulid = ulid::Ulid::from(seed);
        let id = InstanceId::parse(&ulid.to_string()).unwrap();

        registry.register(id.clone(), InstanceActorHandle::test(100), |_| Ok(())).unwrap();

        let count_before = registry.active_count();
        let is_active_before = registry.is_active(&id);
        let lookup_before = registry.lookup(&id).map(|h| h.handle_id());

        let result = registry.register(
            id.clone(),
            InstanceActorHandle::test(200),
            |_| Err("forced failure".to_string()),
        );

        let is_stop_failed = matches!(result, Err(RegistryError::StopFailed { .. }));
        prop_assert!(
            is_stop_failed,
            "expected StopFailed, got {:?}",
            result
        );

        prop_assert_eq!(registry.active_count(), count_before);
        prop_assert_eq!(registry.is_active(&id), is_active_before);
        prop_assert_eq!(registry.lookup(&id).map(|h| h.handle_id()), lookup_before);
    }
}

proptest! {
    #[test]
    fn register_deregister_reregister_cycle_is_consistent(
        seed in 1u128..1000u128,
    ) {
        let config = RegistryConfig { stop_timeout: Duration::from_secs(5) };
        let mut registry = InstanceRegistry::new(config);

        let ulid = ulid::Ulid::from(seed);
        let id = InstanceId::parse(&ulid.to_string()).unwrap();

        registry.register(id.clone(), InstanceActorHandle::test(1), |_| Ok(())).unwrap();
        prop_assert!(registry.is_active(&id));
        prop_assert_eq!(registry.active_count(), 1);

        let deregistered = registry.deregister(&id);
        prop_assert_eq!(deregistered.map(|h| h.handle_id()), Ok(1));
        prop_assert!(!registry.is_active(&id));
        prop_assert_eq!(registry.lookup(&id), None);
        prop_assert_eq!(registry.active_count(), 0);

        registry.register(id.clone(), InstanceActorHandle::test(2), |_| Ok(())).unwrap();
        prop_assert!(registry.is_active(&id));
        prop_assert_eq!(registry.lookup(&id).map(|h| h.handle_id()), Some(2));
        prop_assert_eq!(registry.active_count(), 1);

        prop_assert_eq!(registry.active_count(), 1);
        prop_assert!(registry.is_active(&id));
    }
}
