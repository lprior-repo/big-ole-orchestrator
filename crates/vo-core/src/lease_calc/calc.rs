//! Pure lease state transition calculator.

use super::types::{LeaseError, LeaseState, LeaseTransition};

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
