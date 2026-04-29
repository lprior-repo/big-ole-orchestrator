//! Projection rebuild logic — applies events to rebuild state from scratch.
//!
//! `ProjectionRebuilder` handles full rebuild from event log, tracking
//! progress and supporting cancellation.

use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Instant;

use super::{ProjectionError, ProjectionResult, Projector, RebuildContext};

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

        let mut rebuild_state = S::default();
        let mut processed: u64 = 0;
        let start_seq = self.context.from_sequence;

        for event in events_iter {
            if self.context.is_cancelled() {
                return Err(ProjectionError::BuildFailed(
                    "rebuild cancelled".to_string(),
                ));
            }

            rebuild_state = self
                .projector
                .project(rebuild_state, &event)
                .map_err(|e| ProjectionError::BuildFailed(e.into().to_string()))?;

            processed += 1;
            self.context.update_progress(processed);
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        let end_seq = start_seq.saturating_add(processed);

        Ok(ProjectionResult::new(
            rebuild_state,
            processed,
            start_seq,
            end_seq,
            duration_ms,
            self.projector.schema_version(),
        ))
    }
}

#[cfg(test)]
mod rebuild_tests {
    use super::{ProjectionError, Projector, ProjectionRebuilder};
    use crate::replay::projection::ProjectionEngine;

    #[derive(Debug)]
    struct BuildError(String);

    impl From<BuildError> for ProjectionError {
        fn from(e: BuildError) -> Self {
            ProjectionError::BuildFailed(e.0)
        }
    }

    struct FailingProjector;

    impl Projector<String, String> for FailingProjector {
        type Error = BuildError;

        fn project(&self, state: String, event: &String) -> Result<String, Self::Error> {
            if event == "event-47" {
                Err(BuildError("synthetic failure at event 47".to_string()))
            } else {
                Ok(format!("{}+{}", state, event))
            }
        }

        fn initial_state() -> String {
            String::new()
        }

        fn schema_version(&self) -> u8 {
            1
        }
    }

    #[test]
    fn rebuild_full_atomic_on_event_failure() {
        let engine = ProjectionEngine::new(1);
        let projector = FailingProjector;
        let rebuilder = ProjectionRebuilder::new(
            &engine,
            &projector,
            "test-projection".to_string(),
            0,
        );

        let events: Vec<String> = (1..=100).map(|i| format!("event-{}", i)).collect();
        let result = rebuilder.rebuild_full(events.into_iter());

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ProjectionError::BuildFailed(msg) if msg.contains("synthetic failure at event 47")));
    }

    #[test]
    fn rebuild_full_succeeds_with_all_events() {
        let engine = ProjectionEngine::new(1);
        let projector = FailingProjector;
        let rebuilder = ProjectionRebuilder::new(
            &engine,
            &projector,
            "test-projection".to_string(),
            0,
        );

        let events: Vec<String> = (1..=46).map(|i| format!("event-{}", i)).collect();
        let result = rebuilder.rebuild_full(events.into_iter());

        assert!(result.is_ok());
        let res = result.unwrap();
        assert_eq!(res.events_applied, 46);
        assert!(res.state.contains("event-46"));
    }
}
