use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackpressureEvent {
    QueueFull {
        class: super::WriteClass,
        depth: usize,
        capacity: usize,
    },
    QueueWritable {
        class: super::WriteClass,
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
    pub fn is_backpressured(&self, class: super::WriteClass) -> bool {
        match class {
            super::WriteClass::CriticalControlPlane => self.critical_full.load(Ordering::SeqCst),
            super::WriteClass::OperatorProjection => self.projection_full.load(Ordering::SeqCst),
            super::WriteClass::BulkBlob => self.blob_full.load(Ordering::SeqCst),
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
        #[expect(clippy::unwrap_used)]
        self.last_event.lock().unwrap().clone()
    }

    #[allow(clippy::unwrap_used)]
    pub(crate) fn set_full(&self, class: super::WriteClass, depth: usize, capacity: usize) {
        let was_full = match class {
            super::WriteClass::CriticalControlPlane => {
                self.critical_full.swap(true, Ordering::SeqCst)
            }
            super::WriteClass::OperatorProjection => {
                self.projection_full.swap(true, Ordering::SeqCst)
            }
            super::WriteClass::BulkBlob => self.blob_full.swap(true, Ordering::SeqCst),
        };

        if !was_full {
            let event = BackpressureEvent::QueueFull {
                class,
                depth,
                capacity,
            };
            *self.last_event.lock().unwrap() = Some(event);
        }
    }

    #[allow(clippy::unwrap_used)]
    pub(crate) fn set_writable(&self, class: super::WriteClass, remaining_capacity: usize) {
        let was_full = match class {
            super::WriteClass::CriticalControlPlane => {
                self.critical_full.swap(false, Ordering::SeqCst)
            }
            super::WriteClass::OperatorProjection => {
                self.projection_full.swap(false, Ordering::SeqCst)
            }
            super::WriteClass::BulkBlob => self.blob_full.swap(false, Ordering::SeqCst),
        };

        if was_full {
            let event = BackpressureEvent::QueueWritable {
                class,
                remaining_capacity,
            };
            *self.last_event.lock().unwrap() = Some(event);
        }
    }

    #[must_use]
    pub fn should_reject(&self, class: super::WriteClass) -> bool {
        match class {
            super::WriteClass::CriticalControlPlane => false,
            super::WriteClass::OperatorProjection | super::WriteClass::BulkBlob => {
                self.is_backpressured(class)
            }
        }
    }
}

impl Default for BackpressureSignal {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Default)]
pub struct CommitLatencyTracker {
    state: Mutex<CommitLatencyState>,
}

#[derive(Debug, Default)]
struct CommitLatencyState {
    last_commit_at: Option<Instant>,
    sample_count: u64,
    total_latency_ms: u128,
}

impl CommitLatencyTracker {
    pub fn record_commit(&self, latency_ms: u64) {
        #[expect(clippy::unwrap_used)]
        let mut state = self.state.lock().unwrap();
        state.last_commit_at = Some(Instant::now());
        state.sample_count += 1;
        state.total_latency_ms += u128::from(latency_ms);
    }

    #[must_use]
    pub fn time_since_last_commit(&self) -> Option<std::time::Duration> {
        #[expect(clippy::unwrap_used)]
        let state = self.state.lock().unwrap();
        state.last_commit_at.map(|instant| instant.elapsed())
    }

    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn average_latency_ms(&self) -> Option<u64> {
        #[expect(clippy::unwrap_used)]
        let state = self.state.lock().unwrap();
        if state.sample_count == 0 {
            return None;
        }
        Some(
            u64::try_from(state.total_latency_ms / u128::from(state.sample_count))
                .unwrap_or(u64::MAX),
        )
    }

    #[must_use]
    pub fn sample_count(&self) -> u64 {
        #[expect(clippy::unwrap_used)]
        let state = self.state.lock().unwrap();
        state.sample_count
    }
}
