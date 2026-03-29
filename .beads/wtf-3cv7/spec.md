# BEAD: vo-3cv7 - procedural: Implement wait_for_signal in WorkflowContext

id: "vo-3cv7"
title: "procedural: Implement wait_for_signal in WorkflowContext"
type: feature
priority: 1
effort_estimate: "1hr"
labels: [procedural, signals, context, durable-execution]

clarification_status: "RESOLVED"

resolved_clarifications:
  - question: "How does wait_for_signal handle the checkpoint/replay pattern?"
    answer: "Same dual-phase pattern as activity() and sleep(): (1) load op_counter, (2) check checkpoint_map for a completed signal under that op_id, (3) if checkpoint exists (replay), return the signal payload immediately and increment counter, (4) if no checkpoint (live), register as a signal waiter and yield Pending."
    decided_by: "Existing pattern in context.rs:51-96 (activity) and context.rs:99-144 (sleep)"
    date: "2026-03-23"
  - question: "Where are received signals buffered before a waiter registers?"
    answer: "A new `received_signals: HashMap<String, Bytes>` field on ProceduralActorState. When a signal arrives and no waiter exists, it is buffered. When wait_for_signal is called and a matching entry exists, it is consumed immediately."
    decided_by: "No signal buffering exists yet in InstanceState or ProceduralActorState — must be added."
    date: "2026-03-23"
  - question: "What new InstanceMsg variants are needed?"
    answer: "Two new variants on InstanceMsg: (1) ProceduralWaitForSignal { operation_id: u32, signal_name: String, reply: RpcReplyPort<Result<Bytes, VoError>> } for the context to request waiting, (2) The existing InjectSignal path must be extended to check waiters and buffer undelivered signals."
    decided_by: "InstanceMsg at messages/instance.rs:52-94 — follows the ProceduralDispatch/ProceduralSleep pattern."
    date: "2026-03-23"
  - question: "How are pending signal waiters tracked in InstanceState?"
    answer: "New field on InstanceState: `pending_signal_calls: HashMap<String, RpcReplyPort<Result<Bytes, VoError>>>` keyed by signal_name. This mirrors pending_activity_calls (state.rs:27) and pending_timer_calls (state.rs:31)."
    decided_by: "InstanceState at instance/state.rs:13-38 — same pattern as pending_activity_calls and pending_timer_calls."
    date: "2026-03-23"

assumptions:
  - assumption: "ProceduralActorState is Clone (state/mod.rs:30 derives Clone)"
    validation_method: "See state/mod.rs:30: #[derive(Debug, Clone, Serialize, Deserialize)]"
    risk_if_wrong: "apply_event clone pattern would break — trivially caught at compile"
  - assumption: "WorkflowEvent::SignalReceived { signal_name: String, payload: Bytes } exists in vo-common"
    validation_method: "See crates/vo-common/src/events/mod.rs:71"
    risk_if_wrong: "Cannot replay signal events — but this is a core event type, already defined"
  - assumption: "InstanceMsg::InjectSignal already exists at messages/instance.rs:59-63"
    validation_method: "See messages/instance.rs:59-63"
    risk_if_wrong: "Signal delivery path already established via API -> orchestrator -> instance"
  - assumption: "The handle_inject_event_msg at handlers.rs:87-113 already routes ActivityCompleted and TimerFired to wake pending RPC ports"
    validation_method: "See handlers.rs:94-111"
    risk_if_wrong: "Must follow same pattern for SignalReceived in event replay path"

ears_requirements:
  ubiquitous:
    - "THE SYSTEM SHALL allow procedural workflow code to block on wait_for_signal(signal_name) until the named signal is received"
    - "THE SYSTEM SHALL persist signal receipt via WorkflowEvent::SignalReceived to the event log for durable replay"
  event_driven:
    - trigger: "WHEN wait_for_signal is called and a matching signal is already buffered in received_signals"
      shall: "THE SYSTEM SHALL return the buffered payload immediately and remove it from received_signals"
    - trigger: "WHEN wait_for_signal is called and no matching signal is buffered"
      shall: "THE SYSTEM SHALL register a pending signal waiter in pending_signal_calls and suspend the workflow task until the signal arrives"
    - trigger: "WHEN a signal is received (InjectSignal) and a pending waiter exists for that signal_name"
      shall: "THE SYSTEM SHALL deliver the signal payload to the waiter via its RpcReplyPort and remove the waiter"
    - trigger: "WHEN a signal is received (InjectSignal) and no pending waiter exists"
      shall: "THE SYSTEM SHALL buffer the signal in received_signals for a future wait_for_signal call"
    - trigger: "WHEN the event log replays a WorkflowEvent::SignalReceived during crash recovery"
      shall: "THE SYSTEM SHALL create a checkpoint entry in ProceduralActorState.checkpoint_map so wait_for_signal returns immediately on replay"
  unwanted:
    - condition: "IF the same signal_name is waited on twice before the first completes"
      shall_not: "THE SYSTEM SHALL NOT lose signals — each wait_for_signal call consumes one buffered signal"
      because: "Duplicate wait_for_signal calls are a logic error but must not corrupt state"

contracts:
  preconditions:
    auth_required: false
    required_inputs:
      - field: "signal_name"
        type: "&str"
        constraints: "Non-empty, identifies the signal type (e.g. 'payment_approved')"
        example_valid: "\"approval\""
        example_invalid: "\"\""
    system_state:
      - "WorkflowContext is initialized with valid instance_id, op_counter, and myself ActorRef"
      - "ProceduralActorState has received_signals and checkpoint_map fields"
      - "InstanceState has pending_signal_calls field"
  postconditions:
    state_changes:
      - "On live (no checkpoint): pending_signal_calls contains an entry keyed by signal_name if signal was not buffered"
      - "On live (signal buffered): received_signals no longer contains the consumed entry"
      - "On replay (checkpoint exists): op_counter incremented by 1"
    return_guarantees:
      - field: "Return type"
        guarantee: "anyhow::Result<Bytes> — Ok(payload) on success, Err on actor call failure"
      - field: "Replay path"
        guarantee: "Returns Ok(checkpoint.result) from checkpoint_map[op_id] and increments op_counter"
      - field: "Live path (signal buffered)"
        guarantee: "Returns Ok(buffered_payload) after consuming from received_signals"
      - field: "Live path (signal not yet received)"
        guarantee: "Suspends until InjectSignal delivers, then returns Ok(signal_payload)"
    side_effects:
      - "Registers waiter in pending_signal_calls (live path, no buffered signal)"
      - "Increments op_counter (all paths)"
  invariants:
    - "op_counter increments exactly once per wait_for_signal call regardless of path"
    - "Each wait_for_signal call consumes at most one buffered signal"
    - "pending_signal_calls entries are keyed by signal_name (one waiter per signal_name)"
    - "Checkpoint replay is deterministic — same op_id always returns same result"

research_requirements:
  files_to_read:
    - path: "crates/vo-actor/src/procedural/context.rs"
      what_to_extract: "WorkflowContext struct, activity() and sleep() dual-phase pattern (checkpoint check then live dispatch), op_counter usage"
    - path: "crates/vo-actor/src/procedural/state/mod.rs"
      what_to_extract: "ProceduralActorState struct, Checkpoint struct, checkpoint_map, apply_event function for SignalReceived handling"
    - path: "crates/vo-actor/src/instance/state.rs"
      what_to_extract: "InstanceState struct, pending_activity_calls and pending_timer_calls patterns for waiter registration"
    - path: "crates/vo-actor/src/messages/instance.rs"
      what_to_extract: "InstanceMsg enum, InjectSignal variant, ProceduralDispatch/ProceduralSleep message patterns for new variant design"
    - path: "crates/vo-actor/src/instance/handlers.rs"
      what_to_extract: "handle_procedural_msg dispatch, handle_inject_event_msg for event replay wake-up pattern"
    - path: "crates/vo-common/src/events/mod.rs"
      what_to_extract: "WorkflowEvent::SignalReceived { signal_name: String, payload: Bytes }"
  research_questions:
    - question: "Does apply_event in state/mod.rs handle WorkflowEvent::SignalReceived?"
      answered: true
      answer: "No — it falls into the catch-all `_ =>` branch at line 279 which only inserts into applied_seq. Must add SignalReceived handling that creates a checkpoint entry."
    - question: "What is the return type of wait_for_signal?"
      answered: true
      answer: "anyhow::Result<Bytes> — same as activity(). Bytes carries the signal payload."
    - question: "How does handle_signal (stub) currently work?"
      answered: true
      answer: "handlers.rs:116-129 — it logs and returns Ok(()) via reply port. Must be extended to check pending_signal_calls and buffer in received_signals."
  research_complete_when:
    - "[x] context.rs read — dual-phase pattern understood"
    - "[x] state/mod.rs read — no SignalReceived handler in apply_event"
    - "[x] state.rs read — pending_*_calls pattern established"
    - "[x] messages/instance.rs read — InjectSignal exists, need ProceduralWaitForSignal"
    - "[x] handlers.rs read — wake-up pattern for ActivityCompleted and TimerFired understood"
    - "[x] events/mod.rs read — SignalReceived variant confirmed"

inversions:
  data_integrity_failures:
    - failure: "Signal delivered to waiter but event log write fails — signal lost on crash recovery"
      prevention: "SignalReceived event MUST be written to event log BEFORE delivering to waiter. On replay, the checkpoint from the event will satisfy wait_for_signal."
      test_for_it: "test_signal_replay_without_waiter"
    - failure: "Two waiters registered for same signal_name — second overwrites first"
      prevention: "HashMap semantics mean second wait_for_signal for same signal_name replaces the first waiter. Document as single-waiter-per-signal. If this is wrong, use a Vec instead."
      test_for_it: "test_duplicate_signal_name_overwrites_waiter"
    - failure: "Signal buffered but instance crashes before checkpoint — buffer lost"
      prevention: "Buffer only used in live phase. On replay, SignalReceived event creates checkpoint directly. Buffer is transient."
      test_for_it: "test_buffered_signal_not_in_snapshot"
  ordering_failures:
    - failure: "InjectSignal arrives between checkpoint check and waiter registration (race window)"
      prevention: "Both checkpoint check and waiter registration happen inside the actor's message handler via synchronous InstanceMsg::ProceduralWaitForSignal call. Actor processes messages serially, so no race."
      test_for_it: "test_signal_arrives_during_wait_for_signal"

acceptance_tests:
  happy_paths:
    - name: "test_wait_for_signal_returns_immediately_when_buffered"
      given: "Signal 'approval' already buffered in ProceduralActorState.received_signals with payload b'ok'"
      when: "wait_for_signal(\"approval\") is called"
      then:
        - "Returns Ok(Bytes::from_static(b\"ok\"))"
        - "op_counter incremented by 1"
        - "received_signals no longer contains 'approval'"
    - name: "test_wait_for_signal_replays_from_checkpoint"
      given: "ProceduralActorState.checkpoint_map contains Checkpoint { result: b'data', .. } at operation_id matching current op_counter"
      when: "wait_for_signal(\"approval\") is called during replay phase"
      then:
        - "Returns Ok(Bytes::from_static(b\"data\"))"
        - "op_counter incremented by 1"
        - "No waiter registered"
    - name: "test_wait_for_signal_suspends_until_inject_signal"
      given: "No buffered signal and no checkpoint for current op_id"
      when: "wait_for_signal(\"approval\") is called, then InjectSignal { signal_name: \"approval\", payload: b'go' } is sent"
      then:
        - "Workflow task suspends (waiter registered in pending_signal_calls)"
        - "After InjectSignal, returns Ok(Bytes::from_static(b\"go\"))"
        - "pending_signal_calls no longer contains 'approval'"
  error_paths:
    - name: "test_actor_call_failure"
      given: "Actor ref is dead or call times out"
      when: "wait_for_signal(\"x\") is called"
      then:
        - "Returns Err containing actor call failure description"
    - name: "test_signal_with_no_waiter_gets_buffered"
      given: "No waiter registered for 'timeout' signal"
      when: "InjectSignal { signal_name: \"timeout\", payload: b'expired' } is sent"
      then:
        - "Signal is buffered in received_signals with key 'timeout'"
        - "No reply failure"
        - "Subsequent wait_for_signal(\"timeout\") returns the buffered payload"
  edge_cases:
    - name: "test_multiple_signals_same_name"
      given: "Two signals with name 'retry' arrive before any wait_for_signal call"
      when: "InjectSignal { signal_name: \"retry\", payload: b'1' } then InjectSignal { signal_name: \"retry\", payload: b'2' }"
      then:
        - "First wait_for_signal(\"retry\") returns b'1'"
        - "Second wait_for_signal(\"retry\") returns b'2'"
        - "received_signals is empty after both consumed"
      note: "This requires received_signals to be a HashMap<String, Vec<Bytes>> or similar multi-value buffer"

e2e_tests:
  pipeline_test:
    name: "test_signal_round_trip_in_procedural_workflow"
    description: "Procedural workflow calls wait_for_signal -> external signal sent -> workflow resumes -> completes"
    setup:
      precondition_commands:
        - "docker start vo-nats-test"
    execute:
      steps:
        - "Spawn a procedural workflow that calls ctx.wait_for_signal(\"go\").await"
        - "POST /api/v1/workflows/<id>/signals with { signal_name: 'go', payload: 'started' }"
        - "Wait for workflow to complete"
      timeout_ms: 10000
    verify:
      exit_code: 0
      workflow_completed: true
      signal_delivered: true

verification_checkpoints:
  gate_0_research:
    name: "Research Gate"
    must_pass_before: "Writing any code"
    checks:
      - "[x] context.rs dual-phase pattern understood (checkpoint check then live dispatch)"
      - "[x] ProceduralActorState fields known — no SignalReceived handler in apply_event"
      - "[x] InstanceState pending_*_calls pattern understood"
      - "[x] InstanceMsg variants understood — need ProceduralWaitForSignal"
      - "[x] WorkflowEvent::SignalReceived shape known: { signal_name: String, payload: Bytes }"
      - "[x] handle_inject_event_msg wake-up pattern for TimerFired understood"
    evidence_required:
      - "All function signatures and types documented above"
  gate_1_tests:
    name: "Test Gate"
    must_pass_before: "Writing implementation code"
    checks:
      - "[ ] Test for replay-from-checkpoint path written and compiles"
      - "[ ] Test for buffered-signal path written and compiles"
      - "[ ] Test for live-wait path written and compiles"
      - "[ ] Test for signal buffering on inject written and compiles"
    evidence_required:
      - "Tests exist in context.rs mod tests and state/tests.rs and fail (red)"
  gate_2_implementation:
    name: "Implementation Gate"
    must_pass_before: "Declaring task complete"
    checks:
      - "[ ] All tests pass (green)"
      - "[ ] No unwrap() or expect() in new code"
      - "[ ] received_signals field added to ProceduralActorState"
      - "[ ] pending_signal_calls field added to InstanceState"
      - "[ ] ProceduralWaitForSignal variant added to InstanceMsg"
      - "[ ] SignalReceived handled in apply_event (creates checkpoint)"
      - "[ ] handle_signal extended to check waiters and buffer"
      - "[ ] handle_inject_event_msg extended to wake signal waiters on SignalReceived replay"
      - "[ ] handle_procedural_msg extended to dispatch ProceduralWaitForSignal"
    evidence_required:
      - "cargo test -p vo-actor shows green"
      - "cargo clippy --workspace -- -D warnings passes"

implementation_tasks:
  phase_0_data_structures:
    parallelizable: true
    gate_required: "gate_0_research"
    tasks:
      - task: "Add received_signals field to ProceduralActorState"
        file: "crates/vo-actor/src/procedural/state/mod.rs:31-50"
        done_when: "received_signals: HashMap<String, Vec<Bytes>> field added with #[serde(default)]"
      - task: "Add pending_signal_calls field to InstanceState"
        file: "crates/vo-actor/src/instance/state.rs:13-38"
        done_when: "pending_signal_calls: HashMap<String, RpcReplyPort<Result<Bytes, VoError>>> field added"
      - task: "Add ProceduralWaitForSignal variant to InstanceMsg"
        file: "crates/vo-actor/src/messages/instance.rs:52-94"
        done_when: "ProceduralWaitForSignal { operation_id: u32, signal_name: String, reply: RpcReplyPort<Result<Bytes, VoError>> } added"
      - task: "Add SignalReceived handling to apply_event"
        file: "crates/vo-actor/src/procedural/state/mod.rs:126-285"
        done_when: "WorkflowEvent::SignalReceived arm creates checkpoint_map entry from payload, removes from received_signals"
  phase_1_actor_handlers:
    parallelizable: false
    gate_required: "gate_0_research"
    tasks:
      - task: "Extend handle_signal to check pending_signal_calls and buffer in received_signals"
        file: "crates/vo-actor/src/instance/handlers.rs:116-129"
        done_when: "If pending_signal_calls contains signal_name: remove and reply with payload. Else: buffer in received_signals via ProceduralActorState."
      - task: "Add ProceduralWaitForSignal handler in procedural handler module"
        file: "crates/vo-actor/src/instance/handlers.rs:37-85"
        done_when: "handle_procedural_msg dispatches ProceduralWaitForSignal to procedural::handle_wait_for_signal"
      - task: "Extend handle_inject_event_msg to wake signal waiters on SignalReceived replay"
        file: "crates/vo-actor/src/instance/handlers.rs:87-113"
        done_when: "After applying SignalReceived event, check pending_signal_calls and wake waiter if present"
  phase_2_context_impl:
    parallelizable: false
    gate_required: "gate_1_tests"
    tasks:
      - task: "Implement wait_for_signal on WorkflowContext"
        file: "crates/vo-actor/src/procedural/context.rs:29-189"
        done_when: "Dual-phase method: (1) load op_id, (2) check checkpoint, (3) if replay: return checkpoint.result, (4) if live: send ProceduralWaitForSignal, await reply, increment counter"
  phase_3_tests:
    parallelizable: true
    gate_required: "gate_2_implementation"
    tasks:
      - task: "Add unit tests for wait_for_signal in context.rs mod tests"
        file: "crates/vo-actor/src/procedural/context.rs:191-258"
        done_when: "Tests cover replay path, buffered path, live-wait path, error path"
      - task: "Add tests for signal event application in state/tests.rs"
        file: "crates/vo-actor/src/procedural/state/tests.rs"
        done_when: "SignalReceived creates checkpoint and removes from received_signals"

failure_modes:
  - symptom: "wait_for_signal hangs forever — signal sent but workflow never resumes"
    likely_cause: "handle_signal not checking pending_signal_calls, or signal_name mismatch"
    where_to_look:
      - file: "crates/vo-actor/src/instance/handlers.rs:116-129"
        what_to_check: "Does handle_signal check state.pending_signal_calls.get(&signal_name)?"
    fix_pattern: "Ensure handle_signal checks pending_signal_calls first, then buffers"
  - symptom: "Replay creates checkpoint but wait_for_signal doesn't find it"
    likely_cause: "SignalReceived arm in apply_event doesn't create checkpoint_map entry, or op_id mismatch"
    where_to_look:
      - file: "crates/vo-actor/src/procedural/state/mod.rs:279"
        what_to_check: "Does the SignalReceived arm (currently in catch-all) create a checkpoint_map entry?"
    fix_pattern: "Add explicit WorkflowEvent::SignalReceived arm that creates Checkpoint { result: payload.clone(), completed_seq: seq }"
  - symptom: "SignalReceived event not being applied during replay"
    likely_cause: "handle_inject_event_msg doesn't route SignalReceived to wake waiters"
    where_to_look:
      - file: "crates/vo-actor/src/instance/handlers.rs:94-113"
        what_to_check: "Is there a match arm for WorkflowEvent::SignalReceived after the TimerFired arm?"
    fix_pattern: "Add: if let WorkflowEvent::SignalReceived { signal_name, payload } = &event { check pending_signal_calls and wake }"
  - symptom: "Compile error: cannot find ProceduralWaitForSignal in scope"
    likely_cause: "InstanceMsg variant not added or not imported"
    where_to_look:
      - file: "crates/vo-actor/src/messages/instance.rs:52-94"
        what_to_check: "Is ProceduralWaitForSignal variant defined?"
    fix_pattern: "Add the variant to InstanceMsg enum"

anti_hallucination:
  read_before_write:
    - file: "crates/vo-actor/src/procedural/context.rs"
      must_read_first: true
      key_sections_to_understand:
        - "activity() at lines 51-96 — the canonical dual-phase (checkpoint then live) pattern"
        - "op_counter.load(Ordering::SeqCst) at line 52 — read current op_id without incrementing"
        - "op_counter.fetch_add(1, Ordering::SeqCst) at lines 72, 94 — increment AFTER checkpoint check and after live dispatch"
        - "self.myself.call(|reply| InstanceMsg::..., None).await? pattern at lines 56-64"
    - file: "crates/vo-actor/src/procedural/state/mod.rs"
      must_read_first: true
      key_sections_to_understand:
        - "ProceduralActorState struct at lines 31-50 — no received_signals field exists yet"
        - "apply_event catch-all at line 279-284 — SignalReceived currently falls through here with no checkpoint"
        - "Checkpoint struct at lines 22-27: { result: Bytes, completed_seq: u64 }"
    - file: "crates/vo-actor/src/instance/state.rs"
      must_read_first: true
      key_sections_to_understand:
        - "InstanceState at lines 13-38 — no pending_signal_calls field yet"
        - "pending_activity_calls: HashMap<ActivityId, RpcReplyPort<Result<Bytes, VoError>>> at line 27 — the template pattern"
        - "pending_timer_calls: HashMap<TimerId, RpcReplyPort<Result<(), VoError>>> at line 31"
    - file: "crates/vo-actor/src/messages/instance.rs"
      must_read_first: true
      key_sections_to_understand:
        - "InstanceMsg enum at lines 52-94"
        - "InjectSignal at lines 59-63: { signal_name: String, payload: Bytes, reply: RpcReplyPort<Result<(), VoError>> }"
        - "ProceduralDispatch at lines 74-78 — pattern for new ProceduralWaitForSignal variant"
    - file: "crates/vo-actor/src/instance/handlers.rs"
      must_read_first: true
      key_sections_to_understand:
        - "handle_procedural_msg at lines 37-85 — dispatch table for procedural messages"
        - "handle_signal stub at lines 116-129 — currently logs and returns Ok(())"
        - "handle_inject_event_msg wake-up at lines 94-111 — ActivityCompleted removes from pending_activity_calls, TimerFired from pending_timer_calls"
  no_placeholder_values:
    - "Do NOT use placeholder signal payloads — use actual Bytes::from_static(b\"payload\") in tests"
    - "Do NOT invent new message types — follow InstanceMsg patterns exactly"
    - "Do NOT modify WorkflowEvent::SignalReceived — it already exists at events/mod.rs:71"
    - "Do NOT change the handle_signal reply type — keep RpcReplyPort<Result<(), VoError>> for InjectSignal, the waiter reply is separate"
    - "Do NOT add received_signals as HashMap<String, Bytes> — must be HashMap<String, Vec<Bytes>> to handle multiple signals of same name arriving before a waiter"

context_survival:
  progress_file:
    path: ".beads/vo-3cv7/progress.txt"
    format: "Markdown checklist"
  recovery_instructions: |
    Read progress.txt and continue from last incomplete task.
    Key facts for context recovery:
    - wait_for_signal follows the activity()/sleep() dual-phase pattern in context.rs
    - Phase 0: Add received_signals to ProceduralActorState, pending_signal_calls to InstanceState, ProceduralWaitForSignal to InstanceMsg
    - Phase 0: Add SignalReceived handling to apply_event (creates checkpoint from payload)
    - Phase 1: Extend handle_signal to check waiters then buffer; add ProceduralWaitForSignal to handle_procedural_msg; extend handle_inject_event_msg
    - Phase 2: Implement wait_for_signal on WorkflowContext — checkpoint check then ProceduralWaitForSignal call
    - SignalReceived currently falls through to catch-all in apply_event at state/mod.rs:279
    - handle_signal is a stub at handlers.rs:116-129
    - op_counter: load for checkpoint check, fetch_add(1) after — NOT before

completion_checklist:
  tests:
    - "[ ] Unit test: wait_for_signal replays from checkpoint (returns checkpoint.result)"
    - "[ ] Unit test: wait_for_signal returns buffered signal immediately"
    - "[ ] Unit test: wait_for_signal suspends until InjectSignal delivers"
    - "[ ] Unit test: InjectSignal with no waiter gets buffered"
    - "[ ] Unit test: SignalReceived event creates checkpoint in apply_event"
    - "[ ] Unit test: SignalReceived removes from received_signals on replay"
    - "[ ] cargo test -p vo-actor passes"
  code:
    - "[ ] ProceduralActorState has received_signals: HashMap<String, Vec<Bytes>>"
    - "[ ] InstanceState has pending_signal_calls: HashMap<String, RpcReplyPort<Result<Bytes, VoError>>>"
    - "[ ] InstanceMsg has ProceduralWaitForSignal variant"
    - "[ ] apply_event handles WorkflowEvent::SignalReceived (creates checkpoint)"
    - "[ ] handle_signal checks pending_signal_calls and buffers to received_signals"
    - "[ ] handle_procedural_msg dispatches ProceduralWaitForSignal"
    - "[ ] handle_inject_event_msg wakes signal waiters on SignalReceived replay"
    - "[ ] WorkflowContext::wait_for_signal implemented with dual-phase pattern"
    - "[ ] Zero unwrap() or expect() calls in new code"
  ci:
    - "[ ] cargo test -p vo-actor passes"
    - "[ ] cargo clippy --workspace -- -D warnings passes"
    - "[ ] cargo check --workspace passes"

context:
  related_files:
    - path: "crates/vo-actor/src/procedural/context.rs"
      relevance: "Primary file — implement wait_for_signal method on WorkflowContext (after line 188)"
    - path: "crates/vo-actor/src/procedural/state/mod.rs"
      relevance: "Add received_signals field to ProceduralActorState (line 31-50); add SignalReceived arm to apply_event (line 279)"
    - path: "crates/vo-actor/src/instance/state.rs"
      relevance: "Add pending_signal_calls field to InstanceState (line 27-31 area)"
    - path: "crates/vo-actor/src/messages/instance.rs"
      relevance: "Add ProceduralWaitForSignal variant to InstanceMsg (line 92 area)"
    - path: "crates/vo-actor/src/instance/handlers.rs"
      relevance: "Extend handle_signal (116-129), handle_procedural_msg (37-85), handle_inject_event_msg (87-113)"
    - path: "crates/vo-common/src/events/mod.rs"
      relevance: "WorkflowEvent::SignalReceived already defined at line 71 — read-only reference"
  design_decisions:
    - decision: "received_signals uses HashMap<String, Vec<Bytes>> not HashMap<String, Bytes>"
      rationale: "Multiple signals of the same name can arrive before a waiter registers. Vec preserves FIFO order."
      reversible: true
    - decision: "SignalReceived in apply_event creates checkpoint from event payload directly"
      rationale: "On replay, the event already contains the payload. No need to track separate signal state for replay — checkpoint_map is the source of truth."
      reversible: false
    - decision: "InjectSignal reply remains Result<(), VoError> — waiter reply is separate"
      rationale: "InjectSignal is the external API contract (POST /workflows/:id/signals). The waiter is internal plumbing. Changing InjectSignal reply type would break the API handler."
      reversible: false
    - decision: "wait_for_signal returns anyhow::Result<Bytes> not Result<SignalPayload, ...>"
      rationale: "Consistent with activity() which also returns Result<Bytes>. Signal payload is opaque bytes — the workflow interprets them."
      reversible: false

ai_hints:
  do:
    - "Read context.rs activity() method FIRST — it is the canonical pattern to follow exactly"
    - "Follow the exact op_counter pattern: load for checkpoint check, fetch_add(1) AFTER consuming result"
    - "Use received_signals.entry(signal_name).or_default().push(payload.clone()) for buffering"
    - "Use received_signals.entry(signal_name).or_default().pop() for consuming (VecDeque would be better but HashMap<String, Vec> is simpler)"
    - "Add #[serde(default)] to received_signals to handle snapshots created before this field existed"
    - "Match the exact RpcReplyPort type: RpcReplyPort<Result<Bytes, VoError>>"
    - "In apply_event, the SignalReceived arm must also remove from received_signals to keep buffer clean during replay"
    - "In handle_inject_event_msg, after applying SignalReceived event, check pending_signal_calls and wake the waiter"
  do_not:
    - "Do NOT use .unwrap() or .expect() — use match or map_err"
    - "Do NOT change the WorkflowEvent::SignalReceived variant in vo-common"
    - "Do NOT change the InjectSignal reply type in InstanceMsg"
    - "Do NOT increment op_counter before the checkpoint check — load first, increment after"
    - "Do NOT use tokio::sync::watch or broadcast for signal delivery — RpcReplyPort is the established pattern"
    - "Do NOT make received_signals a single-value HashMap<String, Bytes> — use Vec to handle multiple arrivals"
    - "Do NOT implement waiter timeout in this bead — that is a separate concern"
  constitution:
    - "Zero unwrap law: NEVER use .unwrap() or .expect()"
    - "Functional first: Use match and map_err over if-else for error handling"
    - "Test first: Write tests before implementation code"
    - "Existing patterns: Follow the activity()/sleep() dual-phase pattern exactly"
    - "Deterministic replay: Every path through wait_for_signal must produce identical results on replay"
