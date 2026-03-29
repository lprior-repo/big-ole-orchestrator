# BEAD: vo-h8u4 - e2e: Test signal delivery workflow

id: "vo-h8u4"
title: "e2e: Test signal delivery workflow"
type: feature
priority: 1
effort_estimate: "2hr"
labels: [e2e, signals, procedural, integration-test]

clarification_status: "RESOLVED"

resolved_clarifications:
  - question: "Which crate should the e2e test live in?"
    answer: "vo-actor/tests/signal_delivery_e2e.rs — same pattern as spawn_workflow_test.rs and fsm_crash_replay.rs. The test spawns MasterOrchestrator with a MockEventStore, registers a procedural WorkflowFn that calls wait_for_signal, then sends a signal via OrchestratorMsg::Signal RPC."
    decided_by: "Existing integration tests in vo-actor/tests/ all use MockEventStore + MasterOrchestrator::spawn."
    date: "2026-03-23"
  - question: "Is wait_for_signal already implemented on WorkflowContext?"
    answer: "No. Beads vo-88f4, vo-3cv7, and vo-cedw cover implementing wait_for_signal, pending_signal_calls, and SignalReceived persistence. This e2e test assumes those beads are implemented first (or the test itself verifies the full pipe). The test MUST document the dependency chain."
    decided_by: "vo-3cv7 spec lines 399-406 list wait_for_signal as implementation output. InstanceState has no pending_signal_calls field yet (state.rs:13-38)."
    date: "2026-03-23"
  - question: "Does the test need a real NATS server?"
    answer: "No. The existing MockEventStore pattern (spawn_workflow_test.rs:44-65) returns Ok(1) from publish() and EmptyReplayStream from open_replay_stream(). The e2e test uses this pattern — it validates the actor message flow, not NATS durability. A separate integration test with real NATS is out of scope."
    decided_by: "All existing vo-actor integration tests use MockEventStore (spawn_workflow_test.rs:44-65, fsm_crash_replay.rs)."
    date: "2026-03-23"
  - question: "How does the test verify the workflow completed after receiving the signal?"
    answer: "The test WorkflowFn calls ctx.wait_for_signal(\"go\").await, then returns Ok(()). After sending the signal via OrchestratorMsg::Signal, the test polls GetStatus until the instance is no longer listed, or uses a tokio::time::timeout to confirm the workflow task completes within a deadline."
    decided_by: "ProceduralWorkflowCompleted is sent when the WorkflowFn returns Ok(()) (instance/handlers.rs:69-71). Instance stops after this message."
    date: "2026-03-23"

assumptions:
  - assumption: "Beads vo-88f4 (pending_signal_calls), vo-3cv7 (wait_for_signal on WorkflowContext), and vo-cedw (SignalReceived persistence + wake) are implemented before this test"
    risk_if_wrong: "Test will fail to compile — WorkflowContext::wait_for_signal and InstanceState::pending_signal_calls won't exist"
  - assumption: "InstanceMsg::InjectSignal wakes pending waiters after vo-cedw is implemented"
    risk_if_wrong: "Signal will be sent but workflow will hang forever — test will timeout"
  - assumption: "MockEventStore returning Ok(1) from publish() is sufficient for the actor to proceed with event injection"
    risk_if_wrong: "Signal delivery path may require seq to be meaningful — but spawn_workflow_test.rs proves seq=1 works"
  - assumption: "The test can observe workflow completion by polling GetStatus and detecting instance removal"
    risk_if_wrong: "May need to observe via a different mechanism (e.g. channel from the WorkflowFn)"

ears_requirements:
  ubiquitous:
    - "THE SYSTEM SHALL deliver an external signal to a procedural workflow blocked on wait_for_signal"
    - "THE SYSTEM SHALL resume the procedural workflow execution after the signal is received"
    - "THE SYSTEM SHALL complete the workflow successfully after signal delivery"

  event_driven: []

  unwanted:
    - "THE SYSTEM SHALL NOT lose the signal if it arrives before wait_for_signal is called"
    - "THE SYSTEM SHALL NOT deadlock the workflow if the signal is sent while wait_for_signal is pending"

contracts:
  preconditions:
    auth_required: false
    required_inputs: []
    system_state:
      - "MasterOrchestrator spawned with MockEventStore"
      - "Procedural workflow instance started"
      - "WorkflowFn calls ctx.wait_for_signal(\"go\") and blocks"

  postconditions:
    state_changes:
      - "WorkflowEvent::SignalReceived written to event store"
      - "Workflow resumes from wait_for_signal"
      - "Workflow completes (InstanceStopped / ProceduralWorkflowCompleted)"
    return_guarantees:
      - "OrchestratorMsg::Signal RPC returns Ok(())"

  invariants:
    - "op_counter increments exactly once for the wait_for_signal call"
    - "Signal payload matches what was sent via OrchestratorMsg::Signal"

research_requirements:
  files_to_read:
    - file: "crates/vo-actor/tests/spawn_workflow_test.rs"
      reason: "Reference pattern: MockEventStore, EmptyReplayStream, MasterOrchestrator::spawn, RPC helpers"
    - file: "crates/vo-actor/tests/fsm_crash_replay.rs"
      reason: "Reference pattern: integration test with event replay and workflow lifecycle"
    - file: "crates/vo-actor/src/messages/instance.rs"
      reason: "InstanceMsg::InjectSignal variant at line 59-63, InstanceMsg::ProceduralWaitForSignal (if added by vo-3cv7)"
    - file: "crates/vo-actor/src/messages/orchestrator.rs"
      reason: "OrchestratorMsg::Signal at line 26-31 — the RPC used to send signals from the test"
    - file: "crates/vo-actor/src/instance/handlers.rs"
      reason: "handle_signal at line 116-129 (currently a stub), handle_inject_event_msg for SignalReceived wake"
    - file: "crates/vo-actor/src/instance/state.rs"
      reason: "InstanceState struct — pending_signal_calls field (to be added by vo-88f4)"
    - file: "crates/vo-actor/src/procedural/context.rs"
      reason: "WorkflowContext::wait_for_signal (to be added by vo-3cv7)"
    - file: "crates/vo-api/src/handlers/signal.rs"
      reason: "HTTP signal handler — validates the V3SignalRequest shape"
    - file: "crates/vo-common/src/events/mod.rs"
      reason: "WorkflowEvent::SignalReceived at line 71"

  research_questions: []
  research_complete_when:
    - "spawn_workflow_test.rs pattern understood"
    - "Signal message flow from OrchestratorMsg::Signal → InjectSignal → handle_signal → waiter wake understood"

acceptance_tests:
  happy_paths:
    - name: "e2e_signal_delivery_resumes_and_completes_workflow"
      given: "MasterOrchestrator spawned with MockEventStore; procedural workflow started that calls ctx.wait_for_signal(\"go\").await then returns Ok(())"
      when: "OrchestratorMsg::Signal { signal_name: \"go\", payload: b'proceed' } is sent to the orchestrator"
      then:
        - "Signal RPC returns Ok(())"
        - "Workflow task completes within 5 seconds"
        - "WorkflowFn received Bytes payload b'proceed' from wait_for_signal"
      real_input: "OrchestratorMsg::Signal { instance_id: InstanceId::new(\"signal-e2e-01\"), signal_name: \"go\", payload: Bytes::from_static(b\"proceed\") }"
      expected_output: "Workflow completes, signal payload consumed"

    - name: "e2e_signal_arrives_before_wait_for_signal"
      given: "MasterOrchestrator spawned; procedural workflow started; signal sent BEFORE workflow calls wait_for_signal"
      when: "Workflow calls ctx.wait_for_signal(\"early\").await"
      then:
        - "wait_for_signal returns immediately with the buffered payload"
        - "Workflow completes"
      real_input: "Signal sent via OrchestratorMsg::Signal before WorkflowFn reaches wait_for_signal call"
      expected_output: "Buffered signal consumed, workflow completes"

    - name: "e2e_signal_to_nonexistent_instance"
      given: "MasterOrchestrator spawned; no instance with id \"ghost\""
      when: "OrchestratorMsg::Signal { instance_id: InstanceId::new(\"ghost\"), signal_name: \"x\", payload: b'' } is sent"
      then:
        - "Signal RPC returns Err(VoError::InstanceNotFound)"
      real_input: "OrchestratorMsg::Signal for non-existent instance"
      expected_output: "Err(VoError::InstanceNotFound { .. })"

  sad_paths:
    - name: "e2e_signal_with_wrong_name_does_not_unblock"
      given: "Workflow waiting on wait_for_signal(\"approval\")"
      when: "Signal with signal_name: \"wrong_name\" is sent"
      then:
        - "Workflow remains blocked"
        - "Signal is buffered (not lost)"
      real_input: "OrchestratorMsg::Signal { signal_name: \"wrong_name\", payload: b'' }"
      expected_output: "Workflow does not complete within 2 seconds (timeout)"

edge_cases:
  - name: "e2e_empty_signal_payload"
    scenario: "Signal with payload Bytes::new() is sent to a waiting workflow"
    expected: "wait_for_signal returns Ok(Bytes::new()), workflow completes"

implementation_tasks:
  - task: "Copy MockEventStore + EmptyReplayStream from spawn_workflow_test.rs"
    done_when: "MockEventStore and EmptyReplayStream structs available in test file"
    files:
      - path: "crates/vo-actor/tests/signal_delivery_e2e.rs"
        action: create

  - task: "Implement SignalWorkflowFn — procedural workflow that waits for signal then completes"
    done_when: "SignalWorkflowFn implements WorkflowFn trait, calls ctx.wait_for_signal(\"go\"), returns Ok(())"
    files:
      - path: "crates/vo-actor/tests/signal_delivery_e2e.rs"
        action: create

  - task: "Implement test_config with procedural_workflow: Some(Arc::new(SignalWorkflowFn))"
    done_when: "OrchestratorConfig includes the signal-waiting workflow function"
    files:
      - path: "crates/vo-actor/tests/signal_delivery_e2e.rs"
        action: create

  - task: "Implement send_signal_rpc helper using OrchestratorMsg::Signal"
    done_when: "Helper function sends signal and returns Result<(), VoError>"
    files:
      - path: "crates/vo-actor/tests/signal_delivery_e2e.rs"
        action: create

  - task: "Write e2e_signal_delivery_resumes_and_completes_workflow test"
    done_when: "Test spawns orchestrator, starts workflow, sends signal, verifies completion"
    files:
      - path: "crates/vo-actor/tests/signal_delivery_e2e.rs"
        action: create

  - task: "Write e2e_signal_to_nonexistent_instance test"
    done_when: "Test verifies InstanceNotFound error for ghost instance"
    files:
      - path: "crates/vo-actor/tests/signal_delivery_e2e.rs"
        action: create

  - task: "Write e2e_signal_arrives_before_wait_for_signal test"
    done_when: "Test sends signal before workflow reaches wait_for_signal, verifies buffered delivery"
    files:
      - path: "crates/vo-actor/tests/signal_delivery_e2e.rs"
        action: create

  - task: "Write e2e_signal_with_wrong_name_does_not_unblock test"
    done_when: "Test verifies wrong signal name does not unblock workflow (timeout assertion)"
    files:
      - path: "crates/vo-actor/tests/signal_delivery_e2e.rs"
        action: create

potential_pitfalls:
  - symptom: "Test hangs forever — signal sent but workflow never resumes"
    likely_cause: "handle_signal (handlers.rs:116-129) is still a stub and does not wake pending_signal_calls waiters"
    fix_pattern: "Ensure vo-cedw is implemented: handle_signal publishes WorkflowEvent::SignalReceived, handle_inject_event_msg wakes pending waiter"

  - symptom: "Test fails to compile — wait_for_signal method not found on WorkflowContext"
    likely_cause: "vo-3cv7 not yet implemented"
    fix_pattern: "Implement bead vo-3cv7 first, or gate test behind #[cfg(feature = \"signals\")]"

  - symptom: "Test fails to compile — pending_signal_calls field not found on InstanceState"
    likely_cause: "vo-88f4 not yet implemented"
    fix_pattern: "Implement bead vo-88f4 first"

  - symptom: "Signal RPC returns Ok(()) but workflow hangs"
    likely_cause: "InjectSignal arrives before wait_for_signal registers its RPC port — signal is discarded (stub handler does not buffer)"
    fix_pattern: "Ensure handle_signal buffers in pending_signal_calls when no waiter exists (vo-88f4)"

  - symptom: "tokio test timeout — MockEventStore publish returns Ok(1) but event injection doesn't happen"
    likely_cause: "handle_signal stub does not call inject_event — SignalReceived event never applied to paradigm state"
    fix_pattern: "Ensure vo-cedw wires handle_signal to publish + inject"

dependencies:
  blocking:
    - bead: "vo-88f4"
      reason: "Adds pending_signal_calls field to InstanceState and wires handle_signal to buffer signals"
    - bead: "vo-3cv7"
      reason: "Implements WorkflowContext::wait_for_signal() with dual-phase checkpoint pattern"
    - bead: "vo-cedw"
      reason: "Persists SignalReceived event, wakes pending waiters in handle_inject_event_msg"
  blocked_by: []

verification_criteria:
  - criterion: "cargo test -p vo-actor --test signal_delivery_e2e passes"
  - criterion: "cargo clippy --workspace -- -D warnings passes"
  - criterion: "All 4 acceptance tests pass: signal delivery, early signal, nonexistent instance, wrong name"

implementation_notes: |
  Signal message flow for this test:
  1. Test calls OrchestratorMsg::Signal → master/handlers/signal.rs
  2. Master finds instance ActorRef → sends InstanceMsg::InjectSignal
  3. WorkflowInstance receives InjectSignal → handle_signal (handlers.rs:116)
  4. handle_signal publishes WorkflowEvent::SignalReceived via event_store
  5. Event injected into paradigm state via inject_event (handlers.rs:195)
  6. handle_inject_event_msg checks for SignalReceived → wakes pending_signal_calls waiter
  7. WorkflowContext::wait_for_signal returns Ok(payload) → WorkflowFn continues → completes

  Test structure follows spawn_workflow_test.rs pattern:
  - #[tokio::test] with --test-threads=1 (no shared mutable state between tests)
  - MasterOrchestrator::spawn(Some(name), MasterOrchestrator, test_config()).await
  - RPC helpers: start_workflow_rpc, send_signal_rpc (new), get_status_rpc
  - Each test stops the orchestrator at the end via orchestrator.stop()

  The MockEventStore must be enhanced to track published events so the test can
  optionally assert WorkflowEvent::SignalReceived was published. However, the minimal
  implementation (returning Ok(1)) is sufficient to validate the actor message flow.

  For the "signal arrives before wait_for_signal" test, the WorkflowFn must have a
  small delay (e.g. ctx.sleep(Duration::from_millis(100))) before calling wait_for_signal.
  The test sends the signal immediately after start_workflow, then the workflow wakes up
  and consumes the buffered signal.

code_snippets:
  - name: "MockEventStore (from spawn_workflow_test.rs)"
    language: rust
    content: |
      #[derive(Debug)]
      struct MockEventStore;

      #[async_trait]
      impl EventStore for MockEventStore {
          async fn publish(&self, _ns: &NamespaceId, _inst: &InstanceId, _event: WorkflowEvent) -> Result<u64, VoError> {
              Ok(1)
          }
          async fn open_replay_stream(&self, _ns: &NamespaceId, _inst: &InstanceId, _from_seq: u64) -> Result<Box<dyn ReplayStream>, VoError> {
              Ok(Box::new(EmptyReplayStream))
          }
      }

  - name: "SignalWorkflowFn"
    language: rust
    content: |
      #[derive(Debug)]
      struct SignalWorkflowFn;

      #[async_trait]
      impl WorkflowFn for SignalWorkflowFn {
          async fn execute(&self, ctx: WorkflowContext) -> anyhow::Result<()> {
              let payload = ctx.wait_for_signal("go").await?;
              assert_eq!(payload.as_ref(), b"proceed");
              Ok(())
          }
      }

  - name: "send_signal_rpc helper"
    language: rust
    content: |
      async fn send_signal_rpc(
          orchestrator: &ActorRef<OrchestratorMsg>,
          instance_id: &str,
          signal_name: &str,
          payload: Bytes,
      ) -> Result<(), VoError> {
          let result = orchestrator
              .call(|reply| OrchestratorMsg::Signal {
                  instance_id: InstanceId::new(instance_id),
                  signal_name: signal_name.to_owned(),
                  payload,
                  reply,
              }, Some(RPC_TIMEOUT))
              .await;
          match result {
              Ok(CallResult::Success(Ok(()))) => Ok(()),
              Ok(CallResult::Success(Err(e))) => Err(e),
              Ok(CallResult::Timeout) => Err(VoError::nats_publish("RPC timeout")),
              _ => Err(VoError::nats_publish("RPC call failed")),
          }
      }

files_to_modify:
  - path: "crates/vo-actor/tests/signal_delivery_e2e.rs"
    action: create
    relevance: "New integration test file — all tests live here"
  - path: "crates/vo-actor/Cargo.toml"
    action: check
    relevance: "Verify dev-dependencies include async-trait, bytes, ractor — already present (used by spawn_workflow_test.rs)"

boundaries:
  in_scope:
    - "Integration test for signal delivery via OrchestratorMsg::Signal RPC"
    - "Verification that procedural workflow resumes after wait_for_signal"
    - "Verification that workflow completes after signal"
    - "Edge case: signal to nonexistent instance"
    - "Edge case: signal arrives before wait_for_signal"
    - "Edge case: wrong signal name does not unblock"

  out_of_scope:
    - "HTTP-level signal test (POST /api/v1/workflows/:id/signals) — covered by crates/vo-api/tests/unit/signal_handler_test.rs"
    - "NATS JetStream durability test (real NATS server) — separate integration test"
    - "Signal delivery during crash recovery / replay"
    - "Multiple waiters for the same signal name"

rollout_strategy:
  - "Add crates/vo-actor/tests/signal_delivery_e2e.rs"
  - "Run with: cargo test -p vo-actor --test signal_delivery_e2e -- --test-threads=1"
  - "Test is gated by wait_for_signal implementation — will not compile until vo-3cv7, vo-88f4, vo-cedw are done"

decisions:
  - decision: "Test file lives in vo-actor/tests/ not vo-api/tests/"
    rationale: "Signal delivery is an actor-layer concern. The API handler (signal.rs) is already unit-tested. This test validates the full actor message flow: OrchestratorMsg::Signal → InjectSignal → handle_signal → waiter wake."
  - decision: "No #[ignore] attribute — test is gated by compilation (missing wait_for_signal)"
    rationale: "If the prerequisite beads are not implemented, the test simply won't compile. No need for runtime gating."
  - decision: "Use tokio::time::timeout for completion assertions, not GetStatus polling"
    rationale: "GetStatus returns InstanceStatusSnapshot but doesn't indicate completion. The workflow task completing (JoinHandle resolving) is the reliable signal. However, since we can't access the JoinHandle from outside the actor, we use a small sleep + GetStatus to confirm the instance stopped."
