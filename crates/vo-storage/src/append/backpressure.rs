use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use super::write_class::WriteClass;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackpressureEvent {
    QueueFull {
        class: WriteClass,
        depth: usize,
        capacity: usize,
    },
    QueueWritable {
        class: WriteClass,
        remaining_capacity: usize,
    },
}

#[derive(Debug)]
pub struct BackpressureSignal {
    critical_full: AtomicBool,
    projection_full: AtomicBool,
    blob_full: AtomicBool,
    last_event: Mutex<Option<BackpressureEvent>>,
}

impl BackpressureSignal {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            critical_full: AtomicBool::new(false),
            projection_full: AtomicBool::new(false),
            blob_full: AtomicBool::new(false),
            last_event: Mutex::new(None),
        }
    }

    #[must_use]
    pub fn is_backpressured(&self, class: WriteClass) -> bool {
        match class {
            WriteClass::CriticalControlPlane => self.critical_full.load(Ordering::SeqCst),
            WriteClass::OperatorProjection => self.projection_full.load(Ordering::SeqCst),
            WriteClass::BulkBlob => self.blob_full.load(Ordering::SeqCst),
        }
    }

    #[must_use]
    pub fn any_backpressured(&self) -> bool {
        self.critical_full.load(Ordering::SeqCst)
            || self.projection_full.load(Ordering::SeqCst)
            || self.blob_full.load(Ordering::SeqCst)
    }

    #[must_use]
    pub fn last_event(&self) -> Option<BackpressureEvent> {
        match self.last_event.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    pub(crate) fn set_full(&self, class: WriteClass, depth: usize, capacity: usize) {
        let was_full = match class {
            WriteClass::CriticalControlPlane => self.critical_full.swap(true, Ordering::SeqCst),
            WriteClass::OperatorProjection => self.projection_full.swap(true, Ordering::SeqCst),
            WriteClass::BulkBlob => self.blob_full.swap(true, Ordering::SeqCst),
        };

        if !was_full {
            let event = BackpressureEvent::QueueFull {
                class,
                depth,
                capacity,
            };
            match self.last_event.lock() {
                Ok(mut guard) => *guard = Some(event),
                Err(poisoned) => *poisoned.into_inner() = Some(event),
            }
        }
    }

    pub(crate) fn set_writable(&self, class: WriteClass, remaining_capacity: usize) {
        let was_full = match class {
            WriteClass::CriticalControlPlane => self.critical_full.swap(false, Ordering::SeqCst),
            WriteClass::OperatorProjection => self.projection_full.swap(false, Ordering::SeqCst),
            WriteClass::BulkBlob => self.blob_full.swap(false, Ordering::SeqCst),
        };

        if was_full {
            let event = BackpressureEvent::QueueWritable {
                class,
                remaining_capacity,
            };
            match self.last_event.lock() {
                Ok(mut guard) => *guard = Some(event),
                Err(poisoned) => *poisoned.into_inner() = Some(event),
            }
        }
    }

    #[must_use]
    pub fn should_reject(&self, class: WriteClass) -> bool {
        match class {
            WriteClass::CriticalControlPlane => false,
            WriteClass::OperatorProjection | WriteClass::BulkBlob => self.is_backpressured(class),
        }
    }
}

impl Default for BackpressureSignal {
    fn default() -> Self {
        Self::new()
    }
}
