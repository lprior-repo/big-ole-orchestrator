//! Write class taxonomy for storage QoS tiers.
//!
//! Defines the three-tier write class taxonomy per ADR-032:
//! - Tier 1: CriticalControlPlane — never dropped
//! - Tier 2: OperatorProjection — may lag
//! - Tier 3: BulkBlob — may be deferred
//!
//! Also provides WriteBudget for per-class budget tracking.

use serde::{Deserialize, Serialize};
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Error {
    #[error("unknown write class: {0}")]
    UnknownWriteClass(String),

    #[error("serialization error: {0}")]
    SerializationError(String),

    #[error("taxonomy not initialized")]
    TaxonomyNotInitialized,

    #[error("budget exceeded for {class:?}: requested {requested}, available {available}")]
    BudgetExceeded {
        class: WriteClass,
        requested: u64,
        available: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WriteClass {
    CriticalControlPlane,
    OperatorProjection,
    BulkBlob,
}

impl WriteClass {
    pub fn tier(self) -> u8 {
        match self {
            WriteClass::CriticalControlPlane => 1,
            WriteClass::OperatorProjection => 2,
            WriteClass::BulkBlob => 3,
        }
    }

    pub fn never_drops(self) -> bool {
        matches!(self, WriteClass::CriticalControlPlane)
    }

    pub fn parse(s: &str) -> Result<WriteClass, Error> {
        match s {
            "critical_control_plane" => Ok(WriteClass::CriticalControlPlane),
            "operator_projection" => Ok(WriteClass::OperatorProjection),
            "bulk_blob" => Ok(WriteClass::BulkBlob),
            _ => Err(Error::UnknownWriteClass(s.to_string())),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            WriteClass::CriticalControlPlane => "critical_control_plane",
            WriteClass::OperatorProjection => "operator_projection",
            WriteClass::BulkBlob => "bulk_blob",
        }
    }
}

impl FromStr for WriteClass {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        WriteClass::parse(s)
    }
}
