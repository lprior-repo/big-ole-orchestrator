//! SignalAddress — Routing address for a signal

use std::fmt;

use serde::de::{Deserializer, Error as DeError};
use serde::{Deserialize, Serialize};

use crate::Epoch;
use crate::InstanceId;

use super::lineage_scope::LineageScope;
use super::wait_key::WaitKey;

/// Routing address for a signal, with lineage-aware targeting per ADR-042.
///
/// SignalAddress identifies the target of a signal delivery. It supports two
/// delivery scopes:
/// - **EpochLocal**: Signal targets a specific, immutable epoch
/// - **LineageWide**: Signal routes to the currently active epoch within the lineage
///
/// # Invariants
///
/// - If `lineage_scope == LineageScope::EpochLocal`, then `epoch_id` is `Some`
/// - If `lineage_scope == LineageScope::LineageWide`, then `epoch_id` is `None`
///
/// # Validation
///
/// The [`Deserialize`] impl validates these invariants. Use [`SignalAddress::validate()`]
/// to check an instance manually.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct SignalAddress {
    /// Instance ID of the target workflow instance.
    instance_id: InstanceId,
    /// Wait key for matching against wait records.
    wait_key: WaitKey,
    /// Scope of the signal delivery.
    lineage_scope: LineageScope,
    /// Stable lineage identifier (persists across continue-as-new).
    lineage_id: InstanceId,
    /// Target epoch (required when lineage_scope == EpochLocal, None otherwise).
    epoch_id: Option<Epoch>,
}

impl SignalAddress {
    /// Create a lineage-wide signal address (routes to current active epoch per ADR-042).
    #[must_use]
    pub fn lineage_wide(
        lineage_id: InstanceId,
        instance_id: InstanceId,
        wait_key: WaitKey,
    ) -> Self {
        Self {
            instance_id,
            wait_key,
            lineage_scope: LineageScope::LineageWide,
            lineage_id,
            epoch_id: None,
        }
    }

    /// Create an epoch-local signal address (targets a specific epoch per ADR-042).
    #[must_use]
    pub fn epoch_local(
        lineage_id: InstanceId,
        epoch_id: Epoch,
        instance_id: InstanceId,
        wait_key: WaitKey,
    ) -> Self {
        Self {
            instance_id,
            wait_key,
            lineage_scope: LineageScope::EpochLocal,
            lineage_id,
            epoch_id: Some(epoch_id),
        }
    }

    /// Returns the lineage scope.
    #[must_use]
    pub fn lineage_scope(&self) -> LineageScope {
        self.lineage_scope
    }

    /// Returns the lineage ID.
    #[must_use]
    pub fn lineage_id(&self) -> &InstanceId {
        &self.lineage_id
    }

    /// Returns the epoch ID if set (EpochLocal scope).
    #[must_use]
    pub fn epoch_id(&self) -> Option<Epoch> {
        self.epoch_id
    }

    /// Returns `true` if this is a lineage-wide address.
    #[must_use]
    pub const fn is_lineage_wide(&self) -> bool {
        self.lineage_scope.is_lineage_wide()
    }

    /// Returns `true` if this is an epoch-local address.
    #[must_use]
    pub const fn is_epoch_local(&self) -> bool {
        self.lineage_scope.is_epoch_local()
    }

    /// Returns the instance ID.
    #[must_use]
    pub fn instance_id(&self) -> &InstanceId {
        &self.instance_id
    }

    /// Returns the wait key.
    #[must_use]
    pub fn wait_key(&self) -> &WaitKey {
        &self.wait_key
    }

    fn validate(&self) -> Result<(), &'static str> {
        match self.lineage_scope {
            LineageScope::EpochLocal if self.epoch_id.is_none() => Err(
                "SignalAddress invariant violated: EpochLocal scope requires epoch_id to be Some",
            ),
            LineageScope::LineageWide if self.epoch_id.is_some() => Err(
                "SignalAddress invariant violated: LineageWide scope requires epoch_id to be None",
            ),
            _ => Ok(()),
        }
    }
}

impl<'de> Deserialize<'de> for SignalAddress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct SignalAddressRaw {
            instance_id: InstanceId,
            wait_key: WaitKey,
            lineage_scope: LineageScope,
            lineage_id: InstanceId,
            epoch_id: Option<Epoch>,
        }

        let raw = SignalAddressRaw::deserialize(deserializer)?;
        let addr = SignalAddress {
            instance_id: raw.instance_id,
            wait_key: raw.wait_key,
            lineage_scope: raw.lineage_scope,
            lineage_id: raw.lineage_id,
            epoch_id: raw.epoch_id,
        };

        addr.validate().map_err(|msg| DeError::custom(msg))?;
        Ok(addr)
    }
}

impl fmt::Display for SignalAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}:{}",
            self.lineage_id.as_str(),
            self.instance_id.as_str(),
            self.wait_key.as_str(),
            match self.epoch_id {
                Some(epoch) => format!("epoch={}", epoch.0),
                None => "lineage-wide".to_string(),
            }
        )
    }
}
