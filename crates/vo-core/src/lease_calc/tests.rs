#[cfg(test)]
mod tests {
    use crate::lease_calc::calc::apply;
    use crate::lease_calc::types::{LeaseError, LeaseState, LeaseTransition};
    use vo_types::NodeName;

    fn node(name: &str) -> NodeName {
        NodeName::parse(name).expect("valid node name")
    }

    #[test]
    fn acquire_when_vacant_transitions_to_held() {
        let state = LeaseState::Vacant;
        let result = apply(
            &state,
            LeaseTransition::Acquire {
                requester: node("node-a"),
                ttl_ms: 5000,
                now_ms: 1000,
            },
        );
        assert_eq!(
            result,
            Ok(LeaseState::Held {
                holder: node("node-a"),
                expires_at_ms: 6000,
            })
        );
    }

    #[test]
    fn renew_when_held_by_same_node_extends_expiry() {
        let state = LeaseState::Held {
            holder: node("node-a"),
            expires_at_ms: 6000,
        };
        let result = apply(
            &state,
            LeaseTransition::Renew {
                requester: node("node-a"),
                ttl_ms: 3000,
                now_ms: 4000,
            },
        );
        assert_eq!(
            result,
            Ok(LeaseState::Held {
                holder: node("node-a"),
                expires_at_ms: 7000,
            })
        );
    }

    #[test]
    fn tick_before_expiry_stays_held() {
        let state = LeaseState::Held {
            holder: node("node-a"),
            expires_at_ms: 6000,
        };
        let result = apply(&state, LeaseTransition::Tick { now_ms: 5000 });
        assert_eq!(
            result,
            Ok(LeaseState::Held {
                holder: node("node-a"),
                expires_at_ms: 6000,
            })
        );
    }

    #[test]
    fn tick_at_expiry_transitions_to_expired() {
        let state = LeaseState::Held {
            holder: node("node-a"),
            expires_at_ms: 6000,
        };
        let result = apply(&state, LeaseTransition::Tick { now_ms: 6000 });
        assert_eq!(
            result,
            Ok(LeaseState::Expired {
                last_holder: node("node-a"),
            })
        );
    }

    #[test]
    fn tick_after_expiry_transitions_to_expired() {
        let state = LeaseState::Held {
            holder: node("node-a"),
            expires_at_ms: 6000,
        };
        let result = apply(&state, LeaseTransition::Tick { now_ms: 7000 });
        assert_eq!(
            result,
            Ok(LeaseState::Expired {
                last_holder: node("node-a"),
            })
        );
    }

    #[test]
    fn release_when_held_by_same_node_transitions_to_vacant() {
        let state = LeaseState::Held {
            holder: node("node-a"),
            expires_at_ms: 6000,
        };
        let result = apply(
            &state,
            LeaseTransition::Release {
                requester: node("node-a"),
            },
        );
        assert_eq!(result, Ok(LeaseState::Vacant));
    }

    #[test]
    fn acquire_when_expired_transitions_to_held() {
        let state = LeaseState::Expired {
            last_holder: node("node-a"),
        };
        let result = apply(
            &state,
            LeaseTransition::Acquire {
                requester: node("node-b"),
                ttl_ms: 5000,
                now_ms: 10000,
            },
        );
        assert_eq!(
            result,
            Ok(LeaseState::Held {
                holder: node("node-b"),
                expires_at_ms: 15000,
            })
        );
    }

    #[test]
    fn tick_on_expired_stays_expired() {
        let state = LeaseState::Expired {
            last_holder: node("node-a"),
        };
        let result = apply(&state, LeaseTransition::Tick { now_ms: 20000 });
        assert_eq!(
            result,
            Ok(LeaseState::Expired {
                last_holder: node("node-a"),
            })
        );
    }

    #[test]
    fn renewal_fails_when_held_by_different_nodeid() {
        let state = LeaseState::Held {
            holder: node("node-a"),
            expires_at_ms: 6000,
        };
        let result = apply(
            &state,
            LeaseTransition::Renew {
                requester: node("node-b"),
                ttl_ms: 3000,
                now_ms: 4000,
            },
        );
        assert_eq!(result, Err(LeaseError::RenewalWrongNode));
    }

    #[test]
    fn acquire_fails_when_held_by_same_node_unexpired() {
        let state = LeaseState::Held {
            holder: node("node-a"),
            expires_at_ms: 6000,
        };
        let result = apply(
            &state,
            LeaseTransition::Acquire {
                requester: node("node-a"),
                ttl_ms: 5000,
                now_ms: 1000,
            },
        );
        assert_eq!(
            result,
            Err(LeaseError::AlreadyHeld {
                holder: node("node-a"),
            })
        );
    }

    #[test]
    fn acquire_fails_when_held_by_different_node_unexpired() {
        let state = LeaseState::Held {
            holder: node("node-a"),
            expires_at_ms: 6000,
        };
        let result = apply(
            &state,
            LeaseTransition::Acquire {
                requester: node("node-b"),
                ttl_ms: 5000,
                now_ms: 1000,
            },
        );
        assert_eq!(
            result,
            Err(LeaseError::AlreadyHeld {
                holder: node("node-a"),
            })
        );
    }

    #[test]
    fn acquire_with_zero_ttl_fails() {
        let state = LeaseState::Vacant;
        let result = apply(
            &state,
            LeaseTransition::Acquire {
                requester: node("node-a"),
                ttl_ms: 0,
                now_ms: 1000,
            },
        );
        assert_eq!(result, Err(LeaseError::ZeroTtl));
    }

    #[test]
    fn renew_with_zero_ttl_fails() {
        let state = LeaseState::Held {
            holder: node("node-a"),
            expires_at_ms: 6000,
        };
        let result = apply(
            &state,
            LeaseTransition::Renew {
                requester: node("node-a"),
                ttl_ms: 0,
                now_ms: 4000,
            },
        );
        assert_eq!(result, Err(LeaseError::ZeroTtl));
    }

    #[test]
    fn release_by_wrong_node_fails() {
        let state = LeaseState::Held {
            holder: node("node-a"),
            expires_at_ms: 6000,
        };
        let result = apply(
            &state,
            LeaseTransition::Release {
                requester: node("node-b"),
            },
        );
        assert_eq!(result, Err(LeaseError::RenewalWrongNode));
    }

    #[test]
    fn renew_on_expired_fails() {
        let state = LeaseState::Expired {
            last_holder: node("node-a"),
        };
        let result = apply(
            &state,
            LeaseTransition::Renew {
                requester: node("node-a"),
                ttl_ms: 5000,
                now_ms: 10000,
            },
        );
        assert_eq!(result, Err(LeaseError::InvalidTransition));
    }

    #[test]
    fn renew_after_expiry_time_fails() {
        let state = LeaseState::Held {
            holder: node("node-a"),
            expires_at_ms: 6000,
        };
        let result = apply(
            &state,
            LeaseTransition::Renew {
                requester: node("node-a"),
                ttl_ms: 5000,
                now_ms: 7000,
            },
        );
        assert_eq!(result, Err(LeaseError::InvalidTransition));
    }

    #[test]
    fn release_on_expired_fails() {
        let state = LeaseState::Expired {
            last_holder: node("node-a"),
        };
        let result = apply(
            &state,
            LeaseTransition::Release {
                requester: node("node-a"),
            },
        );
        assert_eq!(result, Err(LeaseError::InvalidTransition));
    }

    #[test]
    fn renew_on_vacant_fails() {
        let state = LeaseState::Vacant;
        let result = apply(
            &state,
            LeaseTransition::Renew {
                requester: node("node-a"),
                ttl_ms: 5000,
                now_ms: 1000,
            },
        );
        assert_eq!(result, Err(LeaseError::InvalidTransition));
    }

    #[test]
    fn release_on_vacant_fails() {
        let state = LeaseState::Vacant;
        let result = apply(
            &state,
            LeaseTransition::Release {
                requester: node("node-a"),
            },
        );
        assert_eq!(result, Err(LeaseError::InvalidTransition));
    }

    #[test]
    fn tick_on_vacant_fails() {
        let state = LeaseState::Vacant;
        let result = apply(&state, LeaseTransition::Tick { now_ms: 1000 });
        assert_eq!(result, Err(LeaseError::InvalidTransition));
    }

    #[test]
    fn invariant_different_node_cannot_acquire_unexpired() {
        let state = LeaseState::Held {
            holder: node("node-a"),
            expires_at_ms: 6000,
        };
        let err = apply(
            &state,
            LeaseTransition::Acquire {
                requester: node("node-b"),
                ttl_ms: 5000,
                now_ms: 5000,
            },
        )
        .unwrap_err();
        assert!(matches!(err, LeaseError::AlreadyHeld { .. }));
        if let LeaseError::AlreadyHeld { holder } = err {
            assert_eq!(holder, node("node-a"));
        }
    }

    #[test]
    fn full_lifecycle_acquire_renew_tick_release_reacquire() {
        let s0 = LeaseState::Vacant;

        let s1 = apply(
            &s0,
            LeaseTransition::Acquire {
                requester: node("node-a"),
                ttl_ms: 5000,
                now_ms: 1000,
            },
        )
        .expect("acquire");
        assert_eq!(
            s1,
            LeaseState::Held {
                holder: node("node-a"),
                expires_at_ms: 6000,
            }
        );

        let s2 = apply(
            &s1,
            LeaseTransition::Renew {
                requester: node("node-a"),
                ttl_ms: 5000,
                now_ms: 3000,
            },
        )
        .expect("renew");
        assert_eq!(
            s2,
            LeaseState::Held {
                holder: node("node-a"),
                expires_at_ms: 8000,
            }
        );

        let s3 = apply(&s2, LeaseTransition::Tick { now_ms: 7000 }).expect("tick not expired");
        assert_eq!(
            s3,
            LeaseState::Held {
                holder: node("node-a"),
                expires_at_ms: 8000,
            }
        );

        let s4 = apply(&s3, LeaseTransition::Tick { now_ms: 8000 }).expect("tick expired");
        assert_eq!(
            s4,
            LeaseState::Expired {
                last_holder: node("node-a"),
            }
        );

        let s5 = apply(
            &s4,
            LeaseTransition::Acquire {
                requester: node("node-b"),
                ttl_ms: 3000,
                now_ms: 9000,
            },
        )
        .expect("re-acquire after expiry");
        assert_eq!(
            s5,
            LeaseState::Held {
                holder: node("node-b"),
                expires_at_ms: 12000,
            }
        );

        let s6 = apply(
            &s5,
            LeaseTransition::Release {
                requester: node("node-b"),
            },
        )
        .expect("release");
        assert_eq!(s6, LeaseState::Vacant);
    }

    #[test]
    fn acquire_with_ttl_overflow_fails() {
        let state = LeaseState::Vacant;
        let result = apply(
            &state,
            LeaseTransition::Acquire {
                requester: node("node-a"),
                ttl_ms: 1,
                now_ms: u64::MAX,
            },
        );
        assert_eq!(result, Err(LeaseError::FenceTokenExhausted));
    }
}
