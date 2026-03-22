//! Bug regression: start_procedural_workflow must initialize WorkflowContext
//! with op_counter = 0, not paradigm_state.operation_counter().
//!
//! BUG: init.rs::start_procedural_workflow passes
//!   `state.paradigm_state.operation_counter()` (= N after replay) to WorkflowContext::new.
//!
//! When the workflow function starts executing, it calls ctx.activity() which reads
//! ctx.op_counter.load() to determine which checkpoint to look up. If op_counter = N
//! instead of 0, the first ctx.activity() checks checkpoint[N] (not found), causing
//! it to dispatch a NEW activity at op N — skipping all N previously-recorded operations
//! and creating a duplicate activity at slot N.
//!
//! The fix: always pass 0 to WorkflowContext::new in start_procedural_workflow.

use bytes::Bytes;
use std::sync::{Arc, Mutex};
use wtf_actor::procedural::{WorkflowContext, WorkflowFn};
use wtf_common::InstanceId;

/// A workflow function that captures the initial op_counter value from its context.
#[derive(Debug)]
struct CaptureInitialOpCounter {
    captured: Arc<Mutex<Option<u32>>>,
}

#[async_trait::async_trait]
impl WorkflowFn for CaptureInitialOpCounter {
    async fn execute(&self, ctx: WorkflowContext) -> anyhow::Result<()> {
        let initial = ctx.op_counter.load(std::sync::atomic::Ordering::SeqCst);
        *self.captured.lock().expect("lock") = Some(initial);
        Ok(())
    }
}

/// start_procedural_workflow must always create ctx with op_counter = 0.
/// After replay of N operations (paradigm_state.operation_counter() = N),
/// the workflow function must start from op 0 to replay checkpoints in order.
#[tokio::test]
async fn workflow_context_op_counter_starts_at_zero_regardless_of_replay_depth() {
    use wtf_actor::instance::{lifecycle::ParadigmState, state::InstanceState};
    use wtf_actor::messages::{InstanceArguments, InstancePhase, WorkflowParadigm};
    use wtf_actor::procedural::{state::apply_event as proc_apply, ProceduralActorState};
    use wtf_common::{NamespaceId, RetryPolicy, WorkflowEvent};
    use std::collections::HashMap;

    // Build a paradigm state with operation_counter = 2 (two dispatches replayed)
    let s0 = ProceduralActorState::new();
    let ev0 = WorkflowEvent::ActivityDispatched {
        activity_id: "inst-01:0".into(),
        activity_type: "step_a".into(),
        payload: Bytes::new(),
        retry_policy: RetryPolicy::default(),
        attempt: 1,
    };
    let ev1 = WorkflowEvent::ActivityDispatched {
        activity_id: "inst-01:1".into(),
        activity_type: "step_b".into(),
        payload: Bytes::new(),
        retry_policy: RetryPolicy::default(),
        attempt: 1,
    };
    let (s1, _) = proc_apply(&s0, &ev0, 1).expect("ev0");
    let (s2, _) = proc_apply(&s1, &ev1, 2).expect("ev1");
    assert_eq!(s2.operation_counter, 2, "precondition: operation_counter = 2 after replay");

    let captured = Arc::new(Mutex::new(None::<u32>));
    let wf_fn: Arc<dyn WorkflowFn> = Arc::new(CaptureInitialOpCounter {
        captured: Arc::clone(&captured),
    });

    let args = InstanceArguments {
        namespace: NamespaceId::new("test"),
        instance_id: InstanceId::new("inst-01"),
        workflow_type: "wf".into(),
        paradigm: WorkflowParadigm::Procedural,
        input: Bytes::from_static(b"{}"),
        engine_node_id: "node-1".into(),
        event_store: None,
        state_store: None,
        task_queue: None,
        snapshot_db: None,
        procedural_workflow: Some(wf_fn),
        workflow_definition: None,
    };

    let mut state = InstanceState {
        paradigm_state: ParadigmState::Procedural(s2),
        args,
        phase: InstancePhase::Live,
        total_events_applied: 2,
        events_since_snapshot: 2,
        pending_activity_calls: HashMap::new(),
        pending_timer_calls: HashMap::new(),
        procedural_task: None,
        live_subscription_task: None,
    };

    // We cannot call start_procedural_workflow without an ActorRef.
    // Instead, assert the CONTRACT: the initial counter passed to WorkflowContext::new
    // must be 0, not state.paradigm_state.operation_counter().
    //
    // This assertion documents the bug: the call is:
    //   WorkflowContext::new(id, state.paradigm_state.operation_counter(), myself)
    // but it MUST be:
    //   WorkflowContext::new(id, 0, myself)
    let buggy_initial = state.paradigm_state.operation_counter(); // = 2 (the bug)
    assert_eq!(
        buggy_initial, 0,
        "start_procedural_workflow must pass 0 to WorkflowContext::new, \
         not paradigm_state.operation_counter() (= {buggy_initial}). \
         Passing N causes the workflow to skip N checkpoints and re-dispatch them."
    );
}
