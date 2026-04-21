use serde::{Deserialize, Serialize};
use std::cell::RefCell;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WriteClass {
    CriticalControlPlane,
    OperatorProjection,
    BulkBlob,
}

impl WriteClass {
    #[must_use]
    pub const fn tier(self) -> u8 {
        match self {
            Self::CriticalControlPlane => 1,
            Self::OperatorProjection => 2,
            Self::BulkBlob => 3,
        }
    }

    #[must_use]
    pub const fn never_drops(self) -> bool {
        matches!(self, Self::CriticalControlPlane)
    }
}

impl std::str::FromStr for WriteClass {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "critical_control_plane" => Ok(Self::CriticalControlPlane),
            "operator_projection" => Ok(Self::OperatorProjection),
            "bulk_blob" => Ok(Self::BulkBlob),
            _ => Err(format!("unknown write class: {s}")),
        }
    }
}

pub fn class_label(class: WriteClass) -> &'static str {
    match class {
        WriteClass::CriticalControlPlane => "critical_control_plane",
        WriteClass::OperatorProjection => "operator_projection",
        WriteClass::BulkBlob => "bulk_blob",
    }
}

#[derive(Clone, Debug)]
pub struct WriteBudget {
    critical_limit: u64,
    projection_limit: u64,
    blob_limit: u64,
    critical_used: RefCell<u64>,
    projection_used: RefCell<u64>,
    blob_used: RefCell<u64>,
}

impl WriteBudget {
    #[must_use]
    pub const fn new(critical_limit: u64, projection_limit: u64, blob_limit: u64) -> Self {
        Self {
            critical_limit,
            projection_limit,
            blob_limit,
            critical_used: RefCell::new(0),
            projection_used: RefCell::new(0),
            blob_used: RefCell::new(0),
        }
    }

    #[must_use]
    pub fn remaining(&self, class: WriteClass) -> u64 {
        match class {
            WriteClass::CriticalControlPlane => self
                .critical_limit
                .saturating_sub(*self.critical_used.borrow()),
            WriteClass::OperatorProjection => self
                .projection_limit
                .saturating_sub(*self.projection_used.borrow()),
            WriteClass::BulkBlob => self.blob_limit.saturating_sub(*self.blob_used.borrow()),
        }
    }

    #[must_use]
    pub fn can_write(&self, class: WriteClass, size_bytes: u64) -> bool {
        self.remaining(class) >= size_bytes
    }

    pub fn reserve(&self, class: WriteClass, size_bytes: u64) -> Result<(), BudgetError> {
        let remaining = self.remaining(class);
        if remaining < size_bytes {
            return Err(BudgetError {
                class,
                requested: size_bytes,
                available: remaining,
            });
        }
        match class {
            WriteClass::CriticalControlPlane => {
                *self.critical_used.borrow_mut() += size_bytes;
            }
            WriteClass::OperatorProjection => {
                *self.projection_used.borrow_mut() += size_bytes;
            }
            WriteClass::BulkBlob => {
                *self.blob_used.borrow_mut() += size_bytes;
            }
        }
        Ok(())
    }

    pub fn release(&self, class: WriteClass, size_bytes: u64) {
        match class {
            WriteClass::CriticalControlPlane => {
                let current = self.critical_used.borrow().saturating_sub(size_bytes);
                *self.critical_used.borrow_mut() = current;
            }
            WriteClass::OperatorProjection => {
                let current = self.projection_used.borrow().saturating_sub(size_bytes);
                *self.projection_used.borrow_mut() = current;
            }
            WriteClass::BulkBlob => {
                let current = self.blob_used.borrow().saturating_sub(size_bytes);
                *self.blob_used.borrow_mut() = current;
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("budget exceeded for {class:?}: requested {requested}, available {available}")]
pub struct BudgetError {
    pub class: WriteClass,
    pub requested: u64,
    pub available: u64,
}
