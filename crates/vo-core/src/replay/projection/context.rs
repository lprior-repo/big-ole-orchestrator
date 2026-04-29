//! Rebuild context — tracks individual rebuild operations.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Instant;

pub struct RebuildContext {
    pub projection_id: String,
    pub from_sequence: u64,
    pub events_total: AtomicUsize,
    pub events_processed: AtomicUsize,
    pub progress_percent: AtomicUsize,
    cancelled: AtomicBool,
    started_at: Instant,
}

impl RebuildContext {
    pub fn new(projection_id: String, from_sequence: u64) -> Self {
        Self {
            projection_id,
            from_sequence,
            events_total: AtomicUsize::new(0),
            events_processed: AtomicUsize::new(0),
            progress_percent: AtomicUsize::new(0),
            cancelled: AtomicBool::new(false),
            started_at: Instant::now(),
        }
    }

    pub fn set_total_events(&self, total: u64) {
        self.events_total.store(total as usize, Ordering::Relaxed);
    }

    pub fn update_progress(&self, processed: u64) {
        self.events_processed
            .store(processed as usize, Ordering::Relaxed);
        let total = self.events_total.load(Ordering::Relaxed);
        if total > 0 {
            let percent = (processed as f64 / total as f64 * 100.0) as usize;
            self.progress_percent.store(percent, Ordering::Relaxed);
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    pub fn elapsed_ms(&self) -> u64 {
        self.started_at.elapsed().as_millis() as u64
    }
}
