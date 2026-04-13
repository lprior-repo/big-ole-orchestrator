//! Lease partition — storage interface for execution leases and fencing (ADR-029).
//!
//! Architecture: Data (`LeaseStoreError`, `LeaseEntry`) → Calc (encode/decode)
//!             → Actions (`LeaseStore` trait).
//!
//! This module defines the trait and pure encoding/decoding functions. Concrete Fjall
//! implementations are provided separately.

use std::fmt;

use vo_types::{FenceToken, InstanceId, LeaseRecord, StepId};

#[cfg(all(test, feature = "proptest"))]
mod proptests;
#[cfg(test)]
mod tests_codec;
#[cfg(test)]
mod tests_integration_acquire;
#[cfg(test)]
mod tests_integration_expiry;
#[cfg(test)]
mod tests_integration_release;
#[cfg(test)]
mod tests_integration_stale;
#[cfg(test)]
mod tests_lease_entry;
#[cfg(any(test, kani))]
mod verification;

mod fjall_lease_store;
pub use fjall_lease_store::FjallLeaseStore;

// ---------------------------------------------------------------------------
// Data layer — error enum
// ---------------------------------------------------------------------------

/// Errors from the lease store operations.
#[non_exhaustive]
#[derive(Debug, PartialEq, Eq)]
pub enum LeaseStoreError {
    /// A lease already exists for this (`instance_id`, `step_id`) pair.
    LeaseAlreadyHeld {
        instance_id: String,
        step_id: String,
    },

    /// The specified lease was not found.
    NotFound {
        instance_id: String,
        step_id: String,
    },

    /// The fence token does not match (stale completion).
    StaleFence { expected: String, actual: String },

    /// The fence-token space for this lease pair is exhausted.
    FenceTokenExhausted {
        instance_id: String,
        step_id: String,
    },

    /// Storage operation failed.
    Storage { reason: String },

    /// Codec/serialization error.
    Codec { reason: String },

    /// Invalid argument.
    InvalidArgument,
}

impl fmt::Display for LeaseStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LeaseAlreadyHeld {
                instance_id,
                step_id,
            } => write!(f, "lease already held for {instance_id}::{step_id}"),
            Self::NotFound {
                instance_id,
                step_id,
            } => write!(f, "lease not found for {instance_id}::{step_id}"),
            Self::StaleFence { expected, actual } => {
                write!(f, "stale fence: expected {expected}, got {actual}")
            }
            Self::FenceTokenExhausted {
                instance_id,
                step_id,
            } => write!(f, "fence token exhausted for {instance_id}::{step_id}"),
            Self::Storage { reason } => write!(f, "lease storage error: {reason}"),
            Self::Codec { reason } => write!(f, "lease codec error: {reason}"),
            Self::InvalidArgument => write!(f, "invalid lease argument"),
        }
    }
}

impl std::error::Error for LeaseStoreError {}

// ---------------------------------------------------------------------------
// Data layer — LeaseEntry (persisted form with expiry)
// ---------------------------------------------------------------------------

/// Persisted lease record with expiry metadata.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LeaseEntry {
    instance_id: String,
    step_id: String,
    fence_token: u64,
    expires_at: u64,
}

impl LeaseEntry {
    /// Construct a new `LeaseEntry`.
    ///
    /// # Errors
    ///
    /// Returns `LeaseStoreError::InvalidArgument` if `instance_id` or `step_id` is empty,
    /// or if `fence_token` is zero.
    pub fn new(
        instance_id: String,
        step_id: String,
        fence_token: u64,
        expires_at: u64,
    ) -> Result<Self, LeaseStoreError> {
        if instance_id.is_empty() || step_id.is_empty() {
            return Err(LeaseStoreError::InvalidArgument);
        }
        if fence_token == 0 {
            return Err(LeaseStoreError::InvalidArgument);
        }
        Ok(Self {
            instance_id,
            step_id,
            fence_token,
            expires_at,
        })
    }

    #[must_use]
    pub fn instance_id(&self) -> &str {
        &self.instance_id
    }

    #[must_use]
    pub fn step_id(&self) -> &str {
        &self.step_id
    }

    #[must_use]
    pub const fn fence_token(&self) -> u64 {
        self.fence_token
    }

    #[must_use]
    pub const fn expires_at(&self) -> u64 {
        self.expires_at
    }

    /// Check if this lease has expired.
    #[must_use]
    pub const fn is_expired(&self, now_ms: u64) -> bool {
        now_ms >= self.expires_at
    }

    /// Convert to a `LeaseRecord` using a valid `FenceToken`.
    ///
    /// # Errors
    ///
    /// Returns `LeaseStoreError::Codec` if the `fence_token` is zero or
    /// `instance_id`/`step_id` cannot be parsed.
    pub fn to_lease_record(&self) -> Result<LeaseRecord, LeaseStoreError> {
        let token = FenceToken::new(self.fence_token).map_err(|_| LeaseStoreError::Codec {
            reason: "invalid fence token value".to_string(),
        })?;
        let iid = InstanceId::parse(&self.instance_id).map_err(|e| LeaseStoreError::Codec {
            reason: format!("invalid instance_id: {e}"),
        })?;
        let sid = StepId::parse(&self.step_id).map_err(|e| LeaseStoreError::Codec {
            reason: format!("invalid step_id: {e}"),
        })?;
        Ok(LeaseRecord::new(iid, sid, token))
    }
}

// ---------------------------------------------------------------------------
// Calc layer — key encoding/decoding
// ---------------------------------------------------------------------------

/// Partition name for the lease store.
pub const LEASE_PARTITION: &str = "leases";

/// Encode a lease key as `<instance_id>::<step_id>` UTF-8 bytes.
///
/// Uses the Display representation of both types with `::` delimiter.
#[must_use]
pub fn encode_lease_key(instance_id: &InstanceId, step_id: &StepId) -> Vec<u8> {
    format!("{instance_id}::{step_id}").into_bytes()
}

/// Decode UTF-8 bytes into an `instance_id` and `step_id`.
///
/// Expects format `<instance_id>::<step_id>`.
///
/// # Errors
///
/// Returns `LeaseStoreError::Codec` if the bytes are not valid UTF-8,
/// do not contain the `::` delimiter, or the parts cannot be parsed.
pub fn decode_lease_key(bytes: &[u8]) -> Result<(InstanceId, StepId), LeaseStoreError> {
    let s = std::str::from_utf8(bytes).map_err(|e| LeaseStoreError::Codec {
        reason: e.to_string(),
    })?;
    let (iid_str, sid_str) = s.split_once("::").ok_or_else(|| LeaseStoreError::Codec {
        reason: "missing :: delimiter in lease key".to_string(),
    })?;
    let instance_id = InstanceId::parse(iid_str).map_err(|e| LeaseStoreError::Codec {
        reason: format!("invalid instance_id: {e}"),
    })?;
    let step_id = StepId::parse(sid_str).map_err(|e| LeaseStoreError::Codec {
        reason: format!("invalid step_id: {e}"),
    })?;
    Ok((instance_id, step_id))
}

// ---------------------------------------------------------------------------
// Calc layer — entry encoding/decoding
// ---------------------------------------------------------------------------

/// Encode a `LeaseEntry` to JSON bytes.
///
/// # Errors
///
/// Returns `LeaseStoreError::Codec` if JSON serialization fails.
pub fn encode_lease_entry(entry: &LeaseEntry) -> Result<Vec<u8>, LeaseStoreError> {
    serde_json::to_vec(entry).map_err(|error| LeaseStoreError::Codec {
        reason: error.to_string(),
    })
}

/// Decode JSON bytes into a `LeaseEntry`.
///
/// # Errors
///
/// Returns `LeaseStoreError::Codec` if deserialization fails.
pub fn decode_lease_entry(bytes: &[u8]) -> Result<LeaseEntry, LeaseStoreError> {
    serde_json::from_slice(bytes).map_err(|e| LeaseStoreError::Codec {
        reason: e.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Actions layer — LeaseStore trait
// ---------------------------------------------------------------------------

/// Storage interface for execution leases and fencing (ADR-029).
///
/// Provides atomic lease acquisition with monotonic fence tokens.
pub trait LeaseStore {
    /// Acquire a lease for (`instance_id`, `step_id`). Returns the `LeaseRecord` with fence token.
    ///
    /// If a lease already exists and is not expired, returns `LeaseStoreError::LeaseAlreadyHeld`.
    /// If a lease exists but is expired, advances the fence and returns the new lease.
    ///
    /// # Errors
    ///
    /// Returns `LeaseStoreError::InvalidArgument` if `ttl_ms` is zero.
    /// Returns `LeaseStoreError::LeaseAlreadyHeld` if the lease is held.
    /// Returns `LeaseStoreError::FenceTokenExhausted` if the per-pair token space is exhausted.
    /// Returns `LeaseStoreError::Storage` if the underlying storage fails.
    fn acquire(
        &self,
        instance_id: &InstanceId,
        step_id: &StepId,
        ttl_ms: u64,
    ) -> Result<LeaseRecord, LeaseStoreError>;

    /// Release a lease. Verifies the fence token matches.
    ///
    /// # Errors
    ///
    /// Returns `LeaseStoreError::NotFound` if the lease does not exist.
    /// Returns `LeaseStoreError::StaleFence` if the token does not match.
    /// Returns `LeaseStoreError::Storage` if the underlying storage fails.
    fn release(&self, lease: &LeaseRecord) -> Result<(), LeaseStoreError>;

    /// Check if a fence token is stale (does not match the current lease).
    ///
    /// Returns `true` if the token is stale (completion should be rejected).
    /// Returns `false` if the token matches or no lease exists.
    ///
    /// # Errors
    ///
    /// Returns `LeaseStoreError::Storage` if the underlying storage fails.
    fn check_stale_fence(
        &self,
        instance_id: &InstanceId,
        step_id: &StepId,
        token: &FenceToken,
    ) -> Result<bool, LeaseStoreError>;
}
