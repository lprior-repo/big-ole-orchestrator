//! BLACK-HAT adversarial tests for vo-common event use-after-free safety (ve-qh5u1).
//!
//! Proves that WorkflowEvent, VoError, and type aliases enforce memory safety
//! under adversarial conditions: clone isolation, drop independence, thread
//! safety, serialization independence, and collection resilience.

use vo_common::{VoError, WorkflowEvent};

#[cfg(test)]
mod clone_isolation {
    use super::*;

    #[test]
    fn cloned_event_timer_id_has_independent_heap_allocation() {
        let ev = WorkflowEvent::TimerFired {
            timer_id: "timer-alpha".into(),
            timestamp_ms: 12345,
        };
        let clone = ev.clone();

        let WorkflowEvent::TimerFired {
            timer_id: ref original_id,
            ..
        } = ev;
        let WorkflowEvent::TimerFired {
            timer_id: ref cloned_id,
            ..
        } = clone;

        assert_ne!(
            original_id.as_ptr(),
            cloned_id.as_ptr(),
            "clone shares heap allocation with original (use-after-free risk)"
        );
        assert_eq!(original_id, cloned_id, "content must still match");
    }

    #[test]
    fn cloned_error_has_independent_heap_allocation() {
        let msg = "A".repeat(1000);
        let err = VoError::internal(msg.clone());
        let clone = err.clone();

        let VoError::Internal(ref s1) = err else {
            panic!("expected Internal variant");
        };
        let VoError::Internal(ref s2) = clone else {
            panic!("expected Internal variant");
        };

        assert_ne!(
            s1.as_ptr(),
            s2.as_ptr(),
            "cloned error shares heap allocation"
        );
        assert_eq!(s1, s2);
    }

    #[test]
    fn deep_clone_large_payload() {
        let big = "X".repeat(10_000);
        let ev = WorkflowEvent::TimerFired {
            timer_id: big,
            timestamp_ms: u64::MAX,
        };
        let mut clone = ev.clone();

        let WorkflowEvent::TimerFired {
            ref mut timer_id, ..
        } = clone;
        timer_id.push_str("_mutated");

        let WorkflowEvent::TimerFired { ref timer_id, .. } = ev;
        assert_eq!(timer_id.len(), 10_000, "original was mutated through clone");
        assert_eq!(timer_id, &"X".repeat(10_000));
    }
}

#[cfg(test)]
mod drop_independence {
    use super::*;

    #[test]
    fn drop_original_does_not_invalidate_clone() {
        let clone = {
            let ev = WorkflowEvent::TimerFired {
                timer_id: "timer-drop-test".into(),
                timestamp_ms: 99999,
            };
            let c = ev.clone();
            drop(ev);
            c
        };
        let WorkflowEvent::TimerFired {
            timer_id,
            timestamp_ms,
        } = clone;
        assert_eq!(timer_id, "timer-drop-test");
        assert_eq!(timestamp_ms, 99999);
    }

    #[test]
    fn drop_clone_does_not_invalidate_original() {
        let ev = WorkflowEvent::TimerFired {
            timer_id: "timer-drop-orig".into(),
            timestamp_ms: 77777,
        };
        let clone = ev.clone();
        drop(clone);

        let WorkflowEvent::TimerFired {
            timer_id,
            timestamp_ms,
        } = ev;
        assert_eq!(timer_id, "timer-drop-orig");
        assert_eq!(timestamp_ms, 77777);
    }

    #[test]
    fn drop_error_original_keeps_clone_valid() {
        let clone = {
            let err = VoError::config("sensitive-config-data".to_string());
            let c = err.clone();
            drop(err);
            c
        };
        assert!(matches!(clone, VoError::Config(ref s) if s == "sensitive-config-data"));
    }

    #[test]
    fn drop_multiple_clones_no_double_free() {
        let ev = WorkflowEvent::TimerFired {
            timer_id: "double-free-test".into(),
            timestamp_ms: 1,
        };
        let clones: Vec<_> = (0..50).map(|_| ev.clone()).collect();
        drop(clones);
        drop(ev);
    }
}

#[cfg(test)]
mod thread_safety {
    use super::*;
    use std::thread;

    #[test]
    fn event_survives_thread_transfer() {
        let ev = WorkflowEvent::TimerFired {
            timer_id: "thread-test".into(),
            timestamp_ms: 42,
        };
        let handle = thread::spawn(move || {
            let WorkflowEvent::TimerFired {
                timer_id,
                timestamp_ms,
            } = ev;
            (timer_id, timestamp_ms)
        });
        let (id, ts) = handle.join().expect("thread panicked");
        assert_eq!(id, "thread-test");
        assert_eq!(ts, 42);
    }

    #[test]
    fn cloned_events_concurrent_access() {
        let ev = WorkflowEvent::TimerFired {
            timer_id: "concurrent".into(),
            timestamp_ms: 100,
        };
        let mut handles = Vec::new();
        for i in 0..8 {
            let clone = ev.clone();
            handles.push(thread::spawn(move || {
                let WorkflowEvent::TimerFired {
                    timer_id,
                    timestamp_ms,
                } = clone;
                assert_eq!(timer_id, "concurrent", "thread {i} got corrupted data");
                assert_eq!(timestamp_ms, 100, "thread {i} got corrupted timestamp");
            }));
        }
        for h in handles {
            h.join().expect("thread panicked");
        }
    }

    #[test]
    fn error_survives_thread_transfer() {
        let err = VoError::timeout("30s".to_string());
        let handle = std::thread::spawn(move || {
            assert!(matches!(err, VoError::Timeout(ref s) if s == "30s"));
            err
        });
        let returned = handle.join().expect("thread panicked");
        assert!(matches!(returned, VoError::Timeout(_)));
    }
}

#[cfg(test)]
mod serialization_independence {
    use super::*;

    #[test]
    fn deserialized_event_owns_its_data() {
        let ev: WorkflowEvent =
            serde_json::from_str(r#"{"TimerFired":{"timer_id":"owned-data","timestamp_ms":555}}"#)
                .unwrap();
        // The &str source is a compile-time constant — event must own its own copy
        let WorkflowEvent::TimerFired {
            timer_id,
            timestamp_ms,
        } = ev;
        assert_eq!(timer_id, "owned-data");
        assert_eq!(timestamp_ms, 555);
    }

    #[test]
    fn serialization_roundtrip_produces_independent_allocation() {
        let ev = WorkflowEvent::TimerFired {
            timer_id: "roundtrip-iso".into(),
            timestamp_ms: 88888,
        };
        let json = serde_json::to_string(&ev).unwrap();
        let decoded: WorkflowEvent = serde_json::from_str(&json).unwrap();

        let WorkflowEvent::TimerFired {
            timer_id: ref id1, ..
        } = ev;
        let WorkflowEvent::TimerFired {
            timer_id: ref id2, ..
        } = decoded;

        assert_ne!(
            id1.as_ptr(),
            id2.as_ptr(),
            "deserialized event aliases original's String"
        );
        assert_eq!(id1, id2);
    }

    #[test]
    fn serialize_drop_original_deserialize_still_valid() {
        let ev = WorkflowEvent::TimerFired {
            timer_id: "ghost-data".into(),
            timestamp_ms: 31415,
        };
        let json = serde_json::to_string(&ev).unwrap();
        drop(ev);
        let decoded: WorkflowEvent = serde_json::from_str(&json).unwrap();
        let WorkflowEvent::TimerFired {
            timer_id,
            timestamp_ms,
        } = decoded;
        assert_eq!(timer_id, "ghost-data");
        assert_eq!(timestamp_ms, 31415);
    }

    #[test]
    fn multiple_deserializations_from_same_json_are_independent() {
        let json = r#"{"TimerFired":{"timer_id":"shared-src","timestamp_ms":0}}"#;
        let d1: WorkflowEvent = serde_json::from_str(json).unwrap();
        let d2: WorkflowEvent = serde_json::from_str(json).unwrap();

        let WorkflowEvent::TimerFired {
            timer_id: ref id1, ..
        } = d1;
        let WorkflowEvent::TimerFired {
            timer_id: ref id2, ..
        } = d2;

        assert_ne!(
            id1.as_ptr(),
            id2.as_ptr(),
            "two deserializations share heap allocation"
        );
        assert_eq!(id1, id2);
    }
}

#[cfg(test)]
mod collection_resilience {
    use super::*;

    #[test]
    fn vec_drain_does_not_corrupt_remaining_events() {
        let mut events: Vec<WorkflowEvent> = (0..20)
            .map(|i| WorkflowEvent::TimerFired {
                timer_id: format!("timer-{i}"),
                timestamp_ms: i,
            })
            .collect();

        let drained: Vec<_> = events.drain(5..15).collect();
        drop(drained);

        assert_eq!(events.len(), 10);
        for (i, ev) in events.iter().enumerate() {
            let expected_idx = if i < 5 { i } else { i + 10 };
            assert_eq!(
                *ev,
                WorkflowEvent::TimerFired {
                    timer_id: format!("timer-{expected_idx}"),
                    timestamp_ms: expected_idx as u64,
                }
            );
        }
    }

    #[test]
    fn vec_retain_preserves_event_integrity() {
        let mut events: Vec<WorkflowEvent> = (0..10)
            .map(|i| WorkflowEvent::TimerFired {
                timer_id: format!("keep-{i}"),
                timestamp_ms: i,
            })
            .collect();

        events.retain(|ev| {
            let WorkflowEvent::TimerFired { timestamp_ms, .. } = ev;
            timestamp_ms % 2 == 0
        });

        assert_eq!(events.len(), 5);
        for (i, ev) in events.iter().enumerate() {
            let WorkflowEvent::TimerFired {
                timer_id,
                timestamp_ms,
            } = ev;
            assert_eq!(*timestamp_ms, (i as u64) * 2);
            assert_eq!(*timer_id, format!("keep-{}", i * 2));
        }
    }

    #[test]
    fn swap_remove_no_use_after_free_on_remaining() {
        let mut events: Vec<WorkflowEvent> = (0..5)
            .map(|i| WorkflowEvent::TimerFired {
                timer_id: format!("swap-{i}"),
                timestamp_ms: i,
            })
            .collect();

        let removed = events.swap_remove(0);
        assert!(matches!(removed, WorkflowEvent::TimerFired { .. }));
        assert_eq!(events.len(), 4);
        for ev in &events {
            let WorkflowEvent::TimerFired {
                timer_id,
                timestamp_ms,
            } = ev;
            assert!(!timer_id.is_empty() || *timestamp_ms == 4);
        }
    }

    #[test]
    fn clone_from_vec_elements_are_independent() {
        let events: Vec<WorkflowEvent> = (0..5)
            .map(|i| WorkflowEvent::TimerFired {
                timer_id: format!("vec-clone-{i}"),
                timestamp_ms: i,
            })
            .collect();

        let cloned: Vec<WorkflowEvent> = events.iter().cloned().collect();
        drop(events);

        for (i, ev) in cloned.iter().enumerate() {
            let WorkflowEvent::TimerFired {
                timer_id,
                timestamp_ms,
            } = ev;
            assert_eq!(timer_id, &format!("vec-clone-{i}"));
            assert_eq!(*timestamp_ms, i as u64);
        }
    }
}

#[cfg(test)]
mod type_alias_ownership {
    use super::*;
    use std::collections::HashMap;
    use vo_common::{NamespaceId, TimerId};

    #[test]
    fn timer_id_clone_then_drop_original() {
        let id = TimerId::new("timer-uaf-test").unwrap();
        let clone = id.clone();
        drop(id);
        assert_eq!(clone.as_str(), "timer-uaf-test");
    }

    #[test]
    fn namespace_id_in_hashmap_survives_removal() {
        let mut map: HashMap<NamespaceId, WorkflowEvent> = HashMap::new();
        for i in 0..10 {
            map.insert(
                NamespaceId::new(format!("ns-{i}")).unwrap(),
                WorkflowEvent::TimerFired {
                    timer_id: format!("t-{i}"),
                    timestamp_ms: i,
                },
            );
        }
        let key = NamespaceId::new("ns-5").unwrap();
        let removed = map.remove(&key).expect("key must exist");
        assert!(map.get(&key).is_none());
        assert_eq!(map.len(), 9);
        let WorkflowEvent::TimerFired { timer_id, .. } = removed;
        assert_eq!(timer_id, "t-5");
    }
}

#[cfg(test)]
mod error_chain_safety {
    use super::*;

    #[test]
    fn error_from_io_error_owns_message() {
        let io_err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe broke");
        let vo_err: VoError = io_err.into();
        let msg = vo_err.to_string();
        assert!(msg.contains("pipe broke"), "error message was freed: {msg}");
    }

    #[test]
    fn error_from_serde_error_owns_message() {
        let serde_err: Result<WorkflowEvent, _> = serde_json::from_str("{invalid}");
        let vo_err: VoError = serde_err.unwrap_err().into();
        assert!(
            !vo_err.to_string().is_empty(),
            "serde error message was lost"
        );
    }

    #[test]
    fn all_error_variants_clone_safe() {
        let errors = vec![
            VoError::config("c"),
            VoError::internal("i"),
            VoError::not_found("n"),
            VoError::validation("v"),
            VoError::timeout("t"),
        ];
        let cloned: Vec<_> = errors.iter().cloned().collect();
        drop(errors);
        for (i, err) in cloned.iter().enumerate() {
            assert!(!err.to_string().is_empty(), "error {i} message freed");
        }
    }
}

#[cfg(test)]
mod edge_cases {
    use super::*;

    #[test]
    fn empty_timer_id_clone_drop_cycle() {
        for _ in 0..100 {
            let ev = WorkflowEvent::TimerFired {
                timer_id: String::new(),
                timestamp_ms: 0,
            };
            let c = ev.clone();
            drop(ev);
            drop(c);
        }
    }

    #[test]
    fn large_timestamp_clone_isolation() {
        let ev = WorkflowEvent::TimerFired {
            timer_id: "max-ts".into(),
            timestamp_ms: u64::MAX,
        };
        let c = ev.clone();
        let WorkflowEvent::TimerFired { timestamp_ms, .. } = c;
        assert_eq!(timestamp_ms, u64::MAX);
    }

    #[test]
    fn event_in_option_take_does_not_uaf() {
        let mut opt = Some(WorkflowEvent::TimerFired {
            timer_id: "opt-take".into(),
            timestamp_ms: 111,
        });
        let taken = opt.take();
        assert!(opt.is_none());
        let WorkflowEvent::TimerFired {
            timer_id,
            timestamp_ms,
        } = taken.unwrap();
        assert_eq!(timer_id, "opt-take");
        assert_eq!(timestamp_ms, 111);
    }

    #[test]
    fn event_in_box_clone_safe() {
        let boxed = Box::new(WorkflowEvent::TimerFired {
            timer_id: "boxed".into(),
            timestamp_ms: 222,
        });
        let clone = (*boxed).clone();
        drop(boxed);
        let WorkflowEvent::TimerFired {
            timer_id,
            timestamp_ms,
        } = clone;
        assert_eq!(timer_id, "boxed");
        assert_eq!(timestamp_ms, 222);
    }

    #[test]
    fn arc_event_multiple_strong_references() {
        use std::sync::Arc;
        let ev = Arc::new(WorkflowEvent::TimerFired {
            timer_id: "arc-shared".into(),
            timestamp_ms: 333,
        });
        let weak = Arc::downgrade(&ev);

        let clone = Arc::clone(&ev);
        drop(ev);

        assert!(weak.upgrade().is_some());

        let WorkflowEvent::TimerFired {
            timer_id,
            timestamp_ms,
        } = &*clone;
        assert_eq!(timer_id, "arc-shared");
        assert_eq!(*timestamp_ms, 333);

        drop(clone);
        assert!(
            weak.upgrade().is_none(),
            "dangling weak reference after all Arcs dropped"
        );
    }

    #[test]
    fn rc_event_does_not_double_free() {
        use std::rc::Rc;
        let ev = Rc::new(WorkflowEvent::TimerFired {
            timer_id: "rc-test".into(),
            timestamp_ms: 444,
        });
        let clones: Vec<_> = (0..20).map(|_| Rc::clone(&ev)).collect();
        drop(ev);
        assert_eq!(Rc::strong_count(&clones[0]), 20);
        drop(clones);
    }
}
