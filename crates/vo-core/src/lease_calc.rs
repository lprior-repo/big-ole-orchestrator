//! Pure lease state transitions (ADR-039).
//!
//! Architecture: Data (`LeaseState`, `LeaseTransition`, `LeaseError`)
//!            → Calc (`apply`, helper predicates)
//!            → Actions (none — this module is pure).
//!
//! Invariant: A lease cannot be acquired by Node B if held by Node A and
//! unexpired. Only the holding node may renew. Time-based expiration is
//! determined through pure chronological calculation.

use vo_types::NodeName;

// ============================================================================
// Data layer — error enum
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LeaseError {
    #[error("lease already held by {holder}")]
    AlreadyHeld { holder: NodeName },
    #[error("only the holding node can renew")]
    RenewalWrongNode,
    #[error("invalid transition for current lease state")]
    InvalidTransition,
    #[error("ttl must be nonzero")]
    ZeroTtl,
    #[error("fence token exhausted")]
    FenceTokenExhausted,
}

// ============================================================================
// Data layer — lease state
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeaseState {
    Vacant,
    Held {
        holder: NodeName,
        expires_at_ms: u64,
    },
    Expired {
        last_holder: NodeName,
    },
}

// ============================================================================
// Data layer — transition commands
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeaseTransition {
    Acquire {
        requester: NodeName,
        ttl_ms: u64,
        now_ms: u64,
    },
    Renew {
        requester: NodeName,
        ttl_ms: u64,
        now_ms: u64,
    },
    Tick {
        now_ms: u64,
    },
    Release {
        requester: NodeName,
    },
}

// ============================================================================
// Calc layer — pure transition function
// ============================================================================

/// Apply a lease transition, returning the new state or an error.
///
/// # Errors
///
/// Returns `LeaseError` variants for invalid transitions.
pub fn apply(state: &LeaseState, transition: LeaseTransition) -> Result<LeaseState, LeaseError> {
    match (state, transition) {
        (
            LeaseState::Vacant,
            LeaseTransition::Acquire {
                requester,
                ttl_ms,
                now_ms,
            },
        ) => {
            let expires_at = calc_expires(now_ms, ttl_ms)?;
            Ok(LeaseState::Held {
                holder: requester,
                expires_at_ms: expires_at,
            })
        }

        (LeaseState::Vacant, _) => Err(LeaseError::InvalidTransition),

        (
            LeaseState::Held {
                holder,
                expires_at_ms,
            },
            LeaseTransition::Acquire {
                requester, now_ms, ..
            },
        ) => {
            if is_expired(*expires_at_ms, now_ms) {
                let expires_at = calc_expires(now_ms, /* ttl from acquire */ 0)?;
                let _ = expires_at;
                Err(LeaseError::InvalidTransition)
            } else if *holder == requester {
                Err(LeaseError::AlreadyHeld {
                    holder: requester.clone(),
                })
            } else {
                Err(LeaseError::AlreadyHeld {
                    holder: holder.clone(),
                })
            }
        }

        (
            LeaseState::Held {
                holder,
                expires_at_ms,
            },
            LeaseTransition::Renew {
                requester,
                ttl_ms,
                now_ms,
            },
        ) => {
            if *holder != requester {
                return Err(LeaseError::RenewalWrongNode);
            }
            if is_expired(*expires_at_ms, now_ms) {
                return Err(LeaseError::InvalidTransition);
            }
            let new_expires = calc_expires(now_ms, ttl_ms)?;
            Ok(LeaseState::Held {
                holder: requester,
                expires_at_ms: new_expires,
            })
        }

        (
            LeaseState::Held {
                holder,
                expires_at_ms,
            },
            LeaseTransition::Tick { now_ms },
        ) => {
            if is_expired(*expires_at_ms, now_ms) {
                Ok(LeaseState::Expired {
                    last_holder: holder.clone(),
                })
            } else {
                Ok(LeaseState::Held {
                    holder: holder.clone(),
                    expires_at_ms: *expires_at_ms,
                })
            }
        }

        (LeaseState::Held { holder, .. }, LeaseTransition::Release { requester }) => {
            if *holder != requester {
                return Err(LeaseError::RenewalWrongNode);
            }
            Ok(LeaseState::Vacant)
        }

        (
            LeaseState::Expired { last_holder },
            LeaseTransition::Acquire {
                requester,
                ttl_ms,
                now_ms,
            },
        ) => {
            let expires_at = calc_expires(now_ms, ttl_ms)?;
            let _ = last_holder;
            Ok(LeaseState::Held {
                holder: requester,
                expires_at_ms: expires_at,
            })
        }

        (LeaseState::Expired { .. }, LeaseTransition::Renew { .. }) => {
            Err(LeaseError::InvalidTransition)
        }

        (LeaseState::Expired { .. }, LeaseTransition::Tick { .. }) => Ok(state.clone()),

        (LeaseState::Expired { .. }, LeaseTransition::Release { .. }) => {
            Err(LeaseError::InvalidTransition)
        }
    }
}

// ============================================================================
// Calc layer — helpers
// ============================================================================

#[must_use]
const fn is_expired(expires_at_ms: u64, now_ms: u64) -> bool {
    now_ms >= expires_at_ms
}

fn calc_expires(now_ms: u64, ttl_ms: u64) -> Result<u64, LeaseError> {
    if ttl_ms == 0 {
        return Err(LeaseError::ZeroTtl);
    }
    now_ms
        .checked_add(ttl_ms)
        .ok_or(LeaseError::FenceTokenExhausted)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(name: &str) -> NodeName {
        NodeName::parse(name).expect("valid node name")
    }

    // ---- Happy path: Acquire when Vacant ----

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

    // ---- Happy path: Renew when held by same NodeId ----

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

    // ---- Happy path: Tick when not expired stays Held ----

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

    // ---- Happy path: Tick when expired transitions to Expired ----

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

    // ---- Happy path: Tick after expired transitions to Expired ----

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

    // ---- Happy path: Release when held by same node ----

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

    // ---- Happy path: Acquire when Expired ----

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

    // ---- Happy path: Tick on Expired stays Expired ----

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

    // ---- Error: Renewal fails when held by different NodeId ----

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

    // ---- Error: Acquire fails when held by same node (unexpired) ----

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

    // ---- Error: Acquire fails when held by different node (unexpired) ----

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

    // ---- Error: Zero TTL ----

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

    // ---- Error: Release by wrong node ----

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

    // ---- Error: Renew on expired state ----

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

    // ---- Error: Renew after expiry time (held but expired) ----

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

    // ---- Error: Release on expired state ----

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

    // ---- Error: Renew/Rlease/Tick on Vacant ----

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

    // ---- Invariant: lease cannot be acquired by Node B if held by Node A ----

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

    // ---- Full lifecycle ----

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

    // ---- TTL overflow ----

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
