//! Projection rebuild logic — applies events to rebuild state from scratch.
//!
//! `ProjectionRebuilder` handles full rebuild from event log, tracking
//! progress and supporting cancellation.

use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use super::{ProjectionError, ProjectionResult, Projector};

pub struct RebuildContext {
    pub projection_id: String,
    pub from_sequence: u64,
    pub events_total: AtomicU64,
    pub events_processed: AtomicU64,
    pub progress_percent: AtomicU64,
    cancelled: AtomicBool,
    started_at: Instant,
}

impl RebuildContext {
    pub fn new(projection_id: String, from_sequence: u64) -> Self {
        Self {
            projection_id,
            from_sequence,
            events_total: AtomicU64::new(0),
            events_processed: AtomicU64::new(0),
            progress_percent: AtomicU64::new(0),
            cancelled: AtomicBool::new(false),
            started_at: Instant::now(),
        }
    }

    pub fn set_total_events(&self, total: u64) {
        self.events_total.store(total, Ordering::Relaxed);
    }

    pub fn update_progress(&self, processed: u64) {
        self.events_processed.store(processed, Ordering::Relaxed);
        let total = self.events_total.load(Ordering::Relaxed);
        if total > 0 {
            let pct = (processed as f64 / total as f64 * 100.0).min(100.0) as u64;
            self.progress_percent.store(pct, Ordering::Relaxed);
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn elapsed_ms(&self) -> u64 {
        self.started_at.elapsed().as_millis() as u64
    }
}

// =====================================================================
#[allow(dead_code)]
pub struct ProjectionRebuilder<'a, S, E, P>
where
    S: Clone + Default + serde::Serialize,
    E: Clone,
    P: Projector<S, E>,
{
    #[allow(dead_code)]
    engine: &'a super::ProjectionEngine,
    projector: &'a P,
    context: Arc<RebuildContext>,
    _phantom: PhantomData<(S, E)>,
}

impl<'a, S, E, P> ProjectionRebuilder<'a, S, E, P>
where
    S: Clone + Default + serde::Serialize,
    E: Clone,
    P: Projector<S, E>,
{
    pub fn new(
        #[allow(dead_code)] engine: &'a super::ProjectionEngine,
        projector: &'a P,
        projection_id: String,
        from_sequence: u64,
    ) -> Self {
        Self {
            engine,
            projector,
            context: Arc::new(RebuildContext::new(projection_id, from_sequence)),
            _phantom: PhantomData,
        }
    }

    pub fn context(&self) -> &Arc<RebuildContext> {
        &self.context
    }

    pub fn rebuild_full<I>(&self, events: I) -> Result<ProjectionResult<S>, ProjectionError>
    where
        I: IntoIterator<Item = E>,
        I::IntoIter: ExactSizeIterator,
    {
        let start = Instant::now();
        let events_iter = events.into_iter();
        let total_events = events_iter.len() as u64;
        self.context.set_total_events(total_events);

        let mut state = S::default();
        let mut processed: u64 = 0;
        let start_seq = self.context.from_sequence;

        for event in events_iter {
            if self.context.is_cancelled() {
                return Err(ProjectionError::BuildFailed(
                    "rebuild cancelled".to_string(),
                ));
            }

            state = self
                .projector
                .project(state, &event)
                .map_err(|e| ProjectionError::BuildFailed(e.into().to_string()))?;

            processed += 1;
            self.context.update_progress(processed);
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        let end_seq = start_seq.saturating_add(processed);

        Ok(ProjectionResult::new(
            state,
            processed,
            start_seq,
            end_seq,
            duration_ms,
            self.projector.schema_version(),
        ))
    }
}
