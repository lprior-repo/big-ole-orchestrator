//! Bug regression: start_procedural_workflow must initialize WorkflowContext
//! with op_counter = 0, not paradigm_state.operation_counter().
//!
//! BUG: init.rs::start_procedural_workflow passes
//!   `state.paradigm_state.operation_counter()` (= N after replay) to WorkflowContext::new.
//!
//! When N > 0, the workflow function's first ctx.activity() reads op_counter = N, looks
//! for checkpoint[N] (not found), and dispatches a NEW activity at slot N — bypassing all N
//! previously-recorded checkpoints (0..N-1) and creating a duplicate activity_id.
//!
//! The fix: always pass 0 to WorkflowContext::new in start_procedural_workflow.

use async_trait::async_trait;
use bytes::Bytes;
use ractor::{Actor, ActorProcessingErr, ActorRef};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use wtf_actor::{
    instance::{init::start_procedural_workflow, lifecycle::ParadigmState, state::InstanceState},
    messages::{InstanceArguments, InstanceMsg, InstancePhase, WorkflowParadigm},
    procedural::{
        state::apply_event as proc_apply, ProceduralActorState, WorkflowContext, WorkflowFn,
    },
};
use wtf_common::{InstanceId, NamespaceId, RetryPolicy, WorkflowEvent};

/// Minimal actor that discards all messages — used to get an ActorRef<InstanceMsg>.
struct NullActor;

#[async_trait]
impl Actor for NullActor {
    type Msg = InstanceMsg;
    type State = ();
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _args: (),
    ) -> Result<(), ActorProcessingErr> {
        Ok(())
    }
}

/// Workflow fn that records the initial op_counter value to a shared mutex.
#[derive(Debug)]
struct CaptureInitialOpCounter {
    captured: Arc<Mutex<Option<u32>>>,
}

#[async_trait]
impl WorkflowFn for CaptureInitialOpCounter {
    async fn execute(&self, ctx: WorkflowContext) -> anyhow::Result<()> {
        let initial = ctx.op_counter.load(std::sync::atomic::Ordering::SeqCst);
        *self.captured.lock().expect("lock") = Some(initial);
        Ok(())
    }
}

fn test_args(wf_fn: Arc<dyn WorkflowFn>) -> InstanceArguments {
    InstanceArguments {
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
    }
}

/// After replaying N dispatches, paradigm_state.operation_counter() = N.
/// start_procedural_workflow must pass 0 to WorkflowContext::new, not N.
/// Currently FAILS because it passes operation_counter() = 2.
#[tokio::test]
async fn start_procedural_workflow_creates_ctx_with_zero_op_counter_after_replay() {
    // Build paradigm state with operation_counter = 2 (simulating replay of 2 dispatches)
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
    assert_eq!(
        s2.operation_counter, 2,
        "precondition: counter=2 after two dispatches"
    );

    let captured = Arc::new(Mutex::new(None::<u32>));
    let wf_fn: Arc<dyn WorkflowFn> = Arc::new(CaptureInitialOpCounter {
        captured: Arc::clone(&captured),
    });

    // Spawn a null actor to obtain a valid ActorRef<InstanceMsg>
    let (null_ref, null_handle) = NullActor::spawn(None, NullActor, ())
        .await
        .expect("null actor spawned");

    let mut state = InstanceState {
        paradigm_state: ParadigmState::Procedural(s2),
        args: test_args(wf_fn),
        phase: InstancePhase::Live,
        total_events_applied: 2,
        events_since_snapshot: 2,
        pending_activity_calls: HashMap::new(),
        pending_timer_calls: HashMap::new(),
        pending_signal_calls: HashMap::new(),
        procedural_task: None,
        live_subscription_task: None,
    };

    start_procedural_workflow(&mut state, &null_ref)
        .await
        .expect("start_procedural_workflow ok");

    // Wait for the workflow task to execute and capture the initial counter
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    let initial = captured.lock().expect("lock").expect("workflow ran");
    assert_eq!(
        initial, 0,
        "WorkflowContext must be created with op_counter=0, not paradigm_state.operation_counter()={initial}. \
         Starting at N skips checkpoints [0..N-1] and re-dispatches them as new activities."
    );

    null_ref.stop(None);
    let _ = null_handle.await;
}
