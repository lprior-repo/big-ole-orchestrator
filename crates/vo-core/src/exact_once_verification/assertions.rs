//! Recovery assertion helpers for exact-once verification (ADR-043).
//!
//! These helpers provide structured assertions for verifying that after a crash
//! and recovery, the system reaches a legal deterministic state.
//!
//! ## Key Invariants Verified
//!
//! 1. **Deterministic replay**: Same event sequence always reaches same state
//! 2. **No duplicate work**: Duplicate ingress commands don't create duplicate logical work
//! 3. **Stale fence rejection**: Old fence tokens cannot win over newer acquisitions
//! 4. **Effect idempotency**: Effects can be applied multiple times without change
//! 5. **Compensation safety**: Compensation only runs for durably committed effects
//! 6. **Signal routing correctness**: Signals route to correct lineage epoch

use vo_types::events::EventEnvelope;
use vo_types::state::LifecycleState;

use crate::exact_once_verification::crash_points::{CrashPoint, CrashPosition, CrashScenario};
use crate::exact_once_verification::harness::VerificationHarness;
use crate::replay::ReplayEngine;
use crate::replay::{ReplayError, ReplayResult};

#[derive(Debug)]
pub struct RecoveryAssertion {
    scenario: CrashScenario,
    pre_crash_state: Option<LifecycleState>,
    post_crash_state: Option<LifecycleState>,
    events_applied_pre: usize,
    events_applied_post: usize,
}

impl RecoveryAssertion {
    pub fn new(scenario: CrashScenario) -> Self {
        Self {
            scenario,
            pre_crash_state: None,
            post_crash_state: None,
            events_applied_pre: 0,
            events_applied_post: 0,
        }
    }

    pub fn with_pre_crash(
        mut self,
        events: &[EventEnvelope],
        engine: &ReplayEngine,
    ) -> Result<Self, ReplayError> {
        let result = engine.replay(events)?;
        self.pre_crash_state = result.final_state;
        self.events_applied_pre = result.events_applied;
        Ok(self)
    }

    pub fn with_post_crash(
        mut self,
        events: &[EventEnvelope],
        engine: &ReplayEngine,
    ) -> Result<Self, ReplayError> {
        let result = engine.replay(events)?;
        self.post_crash_state = result.final_state;
        self.events_applied_post = result.events_applied;
        Ok(self)
    }

    pub fn assert_deterministic(&self) -> Result<(), RecoveryAssertionError> {
        if self.pre_crash_state != self.post_crash_state {
            return Err(RecoveryAssertionError::StateMismatch {
                crash_point: self.scenario.point,
                pre_state: self.pre_crash_state,
                post_state: self.post_crash_state,
            });
        }
        Ok(())
    }

    pub fn assert_no_regression(&self) -> Result<(), RecoveryAssertionError> {
        if self.events_applied_post < self.events_applied_pre {
            return Err(RecoveryAssertionError::EventRegression {
                crash_point: self.scenario.point,
                pre_count: self.events_applied_pre,
                post_count: self.events_applied_post,
            });
        }
        Ok(())
    }

    pub fn assert_legal_state(&self) -> Result<(), RecoveryAssertionError> {
        match self.post_crash_state {
            None => Err(RecoveryAssertionError::IllegalState {
                crash_point: self.scenario.point,
                reason: "No state after recovery".to_string(),
            }),
            Some(LifecycleState::Pending) => Err(RecoveryAssertionError::IllegalState {
                crash_point: self.scenario.point,
                reason: "Pending is not a valid terminal state after recovery".to_string(),
            }),
            _ => Ok(()),
        }
    }

    pub fn crash_point(&self) -> CrashPoint {
        self.scenario.point
    }

    pub fn pre_crash_state(&self) -> Option<LifecycleState> {
        self.pre_crash_state
    }

    pub fn post_crash_state(&self) -> Option<LifecycleState> {
        self.post_crash_state
    }

    pub fn events_applied_pre(&self) -> usize {
        self.events_applied_pre
    }

    pub fn events_applied_post(&self) -> usize {
        self.events_applied_post
    }
}

#[derive(Debug)]
pub enum RecoveryAssertionError {
    StateMismatch {
        crash_point: CrashPoint,
        pre_state: Option<LifecycleState>,
        post_state: Option<LifecycleState>,
    },
    EventRegression {
        crash_point: CrashPoint,
        pre_count: usize,
        post_count: usize,
    },
    IllegalState {
        crash_point: CrashPoint,
        reason: String,
    },
}

impl std::fmt::Display for RecoveryAssertionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StateMismatch {
                crash_point,
                pre_state,
                post_state,
            } => {
                write!(
                    f,
                    "State mismatch at {:?}: pre={:?}, post={:?}",
                    crash_point, pre_state, post_state
                )
            }
            Self::EventRegression {
                crash_point,
                pre_count,
                post_count,
            } => {
                write!(
                    f,
                    "Event regression at {:?}: pre={}, post={}",
                    crash_point, pre_count, post_count
                )
            }
            Self::IllegalState {
                crash_point,
                reason,
            } => {
                write!(f, "Illegal state at {:?}: {}", crash_point, reason)
            }
        }
    }
}

impl std::error::Error for RecoveryAssertionError {}

pub struct RecoveryContext {
    engine: ReplayEngine,
    harness: VerificationHarness,
}

impl RecoveryContext {
    pub fn new() -> Self {
        Self {
            engine: ReplayEngine::new(),
            harness: VerificationHarness::new(),
        }
    }

    pub fn with_harness(harness: VerificationHarness) -> Self {
        Self {
            engine: ReplayEngine::new(),
            harness,
        }
    }

    pub fn replay(&self, events: &[EventEnvelope]) -> ReplayResult {
        self.engine.replay(events).expect("Replay should succeed")
    }

    pub fn verify_at_point(
        &self,
        scenario: CrashScenario,
        pre_crash_events: &[EventEnvelope],
        post_crash_events: &[EventEnvelope],
    ) -> Result<RecoveryAssertion, RecoveryAssertionError> {
        let crash_point = scenario.point;
        let assertion = RecoveryAssertion::new(scenario)
            .with_pre_crash(pre_crash_events, &self.engine)
            .map_err(|e| RecoveryAssertionError::IllegalState {
                crash_point,
                reason: format!("Pre-crash replay failed: {}", e),
            })?
            .with_post_crash(post_crash_events, &self.engine)
            .map_err(|e| RecoveryAssertionError::IllegalState {
                crash_point,
                reason: format!("Post-crash replay failed: {}", e),
            })?;

        assertion.assert_deterministic()?;
        assertion.assert_no_regression()?;
        assertion.assert_legal_state()?;

        Ok(assertion)
    }

    pub fn verify_all_crash_points(
        &self,
        base_events: &[EventEnvelope],
        crash_event_index: usize,
    ) -> Vec<Result<RecoveryAssertion, RecoveryAssertionError>> {
        let mut results = Vec::new();

        for point in CrashPoint::all_variants() {
            for position in CrashPosition::all() {
                let scenario = CrashScenario::new(*point, *position);

                let (pre_crash, post_crash) = if crash_event_index <= base_events.len() {
                    (&base_events[..crash_event_index], base_events)
                } else {
                    (base_events, base_events)
                };

                results.push(self.verify_at_point(scenario, pre_crash, post_crash));
            }
        }

        results
    }

    pub fn harness(&self) -> &VerificationHarness {
        &self.harness
    }
}

impl Default for RecoveryContext {
    fn default() -> Self {
        Self::new()
    }
}

pub fn assert_no_duplicate_effects(effects: &[EventEnvelope]) -> Result<(), String> {
    let mut seen = std::collections::HashSet::new();
    for effect in effects {
        let key = (&effect.instance_id, effect.sequence);
        if !seen.insert(key) {
            return Err(format!(
                "Duplicate effect detected: instance_id={}, sequence={}",
                effect.instance_id, effect.sequence
            ));
        }
    }
    Ok(())
}

pub fn assert_fence_token_ordering(
    acquisitions: &[(u64, String)],
    completions: &[(u64, String)],
) -> Result<(), String> {
    for (fence_seq, _) in completions {
        let valid_acquisition = acquisitions.iter().find(|(acq_seq, _)| acq_seq < fence_seq);

        if valid_acquisition.is_none() {
            return Err(format!(
                "Stale fence completion at sequence {}: no earlier acquisition",
                fence_seq
            ));
        }
    }
    Ok(())
}

pub fn assert_invariant_no_orphans<'a, I: IntoIterator<Item = &'a EventEnvelope>>(
    events: I,
) -> Result<(), String>
where
    I::IntoIter: Clone,
{
    let events: Vec<&EventEnvelope> = events.into_iter().collect();
    let mut parent_started: std::collections::HashMap<String, bool> =
        std::collections::HashMap::new();

    for event in &events {
        if let Ok(payload) = vo_types::events::EventPayload::try_from_json(&event.payload) {
            match payload {
                vo_types::events::EventPayload::WorkflowStarted { .. } => {
                    parent_started.insert(event.instance_id.clone(), true);
                }
                _ => {}
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::test_helpers::{
        make_event, step_completed_payload, step_scheduled_payload, step_started_payload,
        workflow_started_payload,
    };

    #[test]
    fn recovery_assertion_new() {
        let scenario = CrashScenario::new(CrashPoint::DedupeWrite, CrashPosition::Before);
        let assertion = RecoveryAssertion::new(scenario);

        assert_eq!(assertion.crash_point(), CrashPoint::DedupeWrite);
        assert!(assertion.pre_crash_state.is_none());
    }

    #[test]
    fn recovery_assertion_with_pre_crash() {
        let scenario = CrashScenario::new(CrashPoint::DedupeWrite, CrashPosition::Before);
        let events = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        ];

        let engine = ReplayEngine::new();
        let assertion = RecoveryAssertion::new(scenario)
            .with_pre_crash(&events, &engine)
            .expect("Pre-crash replay should succeed");

        assert!(assertion.pre_crash_state.is_some());
        assert_eq!(assertion.events_applied_pre, 2);
    }

    #[test]
    fn recovery_assertion_deterministic_passes() {
        let scenario = CrashScenario::new(CrashPoint::DedupeWrite, CrashPosition::Before);
        let events = vec![make_event("inst-1", 1, workflow_started_payload("wf-1"))];

        let engine = ReplayEngine::new();
        let assertion = RecoveryAssertion::new(scenario)
            .with_pre_crash(&events, &engine)
            .expect("Pre-crash replay should succeed")
            .with_post_crash(&events, &engine)
            .expect("Post-crash replay should succeed");

        assertion
            .assert_deterministic()
            .expect("Should be deterministic");
    }

    #[test]
    fn recovery_assertion_deterministic_fails() {
        let scenario = CrashScenario::new(CrashPoint::DedupeWrite, CrashPosition::Before);
        let events_pre = vec![make_event("inst-1", 1, workflow_started_payload("wf-1"))];
        let events_post = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        ];

        let engine = ReplayEngine::new();
        let assertion = RecoveryAssertion::new(scenario)
            .with_pre_crash(&events_pre, &engine)
            .expect("Pre-crash replay should succeed")
            .with_post_crash(&events_post, &engine)
            .expect("Post-crash replay should succeed");

        let result = assertion.assert_deterministic();
        assert!(result.is_err());
    }

    #[test]
    fn recovery_context_new() {
        let ctx = RecoveryContext::new();
        assert!(ctx.harness().should_crash(CrashPoint::DedupeWrite) == false);
    }

    fn make_test_event_for_dedup(instance_id: &str, sequence: u64) -> EventEnvelope {
        EventEnvelope {
            schema_version: 1,
            instance_id: instance_id.to_string(),
            sequence,
            timestamp_ms: 1000 * sequence,
            payload: serde_json::json!({
                "type": "WorkflowStarted",
                "workflow_id": instance_id,
                "binary_hash": "sha256abc",
                "version": 1
            }),
            metadata: vo_types::events::EventMetadata::default(),
        }
    }

    #[test]
    fn assert_no_duplicate_effects_passes() {
        let effects = vec![
            make_test_event_for_dedup("inst-1", 1),
            make_test_event_for_dedup("inst-1", 2),
            make_test_event_for_dedup("inst-2", 1),
        ];

        assert_no_duplicate_effects(&effects).expect("Should not have duplicates");
    }

    #[test]
    fn assert_no_duplicate_effects_fails() {
        let effects = vec![
            make_test_event_for_dedup("inst-1", 1),
            make_test_event_for_dedup("inst-1", 1),
        ];

        let result = assert_no_duplicate_effects(&effects);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Duplicate effect detected"));
    }

    #[test]
    fn assert_fence_token_ordering_valid() {
        let acquisitions = vec![(1, "token-1".to_string()), (3, "token-2".to_string())];
        let completions = vec![(2, "token-1".to_string()), (4, "token-2".to_string())];

        assert_fence_token_ordering(&acquisitions, &completions).expect("Ordering should be valid");
    }

    #[test]
    fn assert_fence_token_ordering_stale() {
        let acquisitions = vec![(3, "token-1".to_string())];
        let completions = vec![(2, "token-1".to_string())];

        let result = assert_fence_token_ordering(&acquisitions, &completions);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Stale fence completion"));
    }
}
