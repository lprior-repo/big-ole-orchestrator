//! Data layer types for lease state machine.

use vo_types::NodeName;

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
