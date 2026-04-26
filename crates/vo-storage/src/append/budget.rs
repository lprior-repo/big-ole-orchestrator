use std::cell::RefCell;

use super::write_class::WriteClass;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("budget exceeded for {class:?}: requested {requested}, available {available}")]
pub struct BudgetError {
    pub class: WriteClass,
    pub requested: u64,
    pub available: u64,
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
