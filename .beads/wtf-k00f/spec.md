# BEAD: vo-k00f - e2e: Test terminate workflow

id: "vo-k00f"
title: "e2e: Test terminate workflow"
type: task
priority: 1
effort_estimate: "1hr"
labels: [e2e, terminate, integration-test, journal]

clarification_status: "RESOLVED"

resolved_clarifications:
  - question: "What is the full terminate request flow?"
    answer: "DELETE /api/v1/workflows/<namespace>/<instance_id> -> axum terminate_workflow handler -> OrchestratorMsg::Terminate -> handle_terminate in master -> InstanceMsg::Cancel -> handle_cancel in instance handlers -> publishes WorkflowEvent::InstanceCancelled to JetStream via EventStore -> myself_ref.stop(Some(reason))"
    decided_by: "workflow.rs:70-95, terminate.rs:8-48, handlers.rs:143-175"
    date: "2026-03-23"
  - question: "How do we verify InstanceCancelled appears in the journal?"
    answer: "GET /api/v1/workflows/<namespace>/<instance_id>/journal returns JournalResponse with JournalEntry list. The InstanceCancelled event maps to JournalEntryType::Run with status='recorded' via the catch-all arm in map_event_fields at journal.rs:183-191"
    decided_by: "journal.rs:183-191, map_event_fields catch-all arm"
    date: "2026-03-23"
  - question: "What paradigm should the test workflow use?"
    answer: "Procedural with a long sleep (e.g. 60000ms) to keep the instance alive while the DELETE is issued. FSM or DAG also work if a definition is registered. Procedural is simplest because it requires only registering a WorkflowFn, not a full definition."
    decided_by: "actor.rs:57-59 starts procedural workflow in pre_start; start.rs spawns with paradigm from request"
    date: "2026-03-23"
  - question: "Where should the E2E test file live?"
    answer: "New file crates/vo-actor/tests/terminate_e2e.rs as an integration test (tests/ directory, not #[cfg(test)] in src). This requires spawning real Ractor actors and a live NATS JetStream connection."
    decided_by: "Workspace convention — integration tests with external deps go in tests/"
    date: "2026-03-23"
  - question: "How to verify the actor actually stopped?"
    answer: "After sending InstanceMsg::Cancel, call actor_ref.get_status(). If the actor stopped, this returns Err (ActorExitStatus) or we can use a oneshot channel in post_stop. Simpler: use ActorRef::is_alive() or monitor the supervised link. Most practical: after terminate succeeds, attempt GetStatus on the orchestrator — it should return NOT_FOUND (ActorDied)."
    decided_by: "workflow_mappers.rs:122-126 maps GetStatusError::ActorDied to 404"
    date: "2026-03-23"

assumptions:
  - assumption: "NATS server is running in Docker container vo-nats-test on localhost:4222"
    validation_method: "AGENTS.md specifies this; cargo run -p vo-storage --bin nats_connect_test"
    risk_if_wrong: "All integration tests fail with connection refused — non-recoverable in test"
  - assumption: "JetStream stream vo-events is provisioned (via provision_streams)"
    validation_method: "Called during startup; provision.rs provisions vo-events stream"
    risk_if_wrong: "append_event() fails with stream not found error"
  - assumption: "vo-storage NatsClient implements EventStore trait (lib.rs:41-64)"
    validation_method: "See lib.rs:40-64 — EventStore impl for NatsClient delegates to journal::append_event"
    risk_if_wrong: "Cannot compile — would be caught immediately"
  - assumption: "The terminate handler in instance/handlers.rs:143-175 always calls myself_ref.stop(Some(reason)) after publishing InstanceCancelled"
    validation_method: "Line 173: myself_ref.stop(Some(reason))"
    risk_if_wrong: "Actor might remain alive after cancel — test would hang or timeout"
  - assumption: "OrchestratorState tracks active instances in HashMap<InstanceId, ActorRef<InstanceMsg>> (state.rs:42)"
    validation_method: "state.rs:42 — active field"
    risk_if_wrong: "handle_terminate would fail to find the instance"

ears_requirements:
  ubiquitous:
    - "THE SYSTEM SHALL support terminating a running workflow instance via DELETE /api/v1/workflows/:id"
    - "THE SYSTEM SHALL record a WorkflowEvent::InstanceCancelled event in the JetStream journal before stopping the actor"
    - "THE SYSTEM SHALL stop the WorkflowInstance actor after successful cancellation"
  event_driven:
    - trigger: "WHEN DELETE /api/v1/workflows/<ns>/<id> is received for a running instance"
      shall: "THE SYSTEM SHALL return HTTP 204 No Content on successful termination"
    - trigger: "WHEN the termination handler processes InstanceMsg::Cancel"
      shall: "THE SYSTEM SHALL publish WorkflowEvent::InstanceCancelled { reason: \"api-terminate\" } to the JetStream subject wtf.log.<namespace>.<instance_id>"
    - trigger: "AFTER publishing InstanceCancelled to JetStream"
      shall: "THE SYSTEM SHALL call myself_ref.stop(Some(reason)) to terminate the WorkflowInstance actor"
    - trigger: "WHEN DELETE /api/v1/workflows/:id is received for a non-existent instance"
      shall: "THE SYSTEM SHALL return HTTP 404 with ApiError { error: \"not_found\", message: <instance_id> }"
    - trigger: "WHEN GET /api/v1/workflows/<ns>/<id>/journal is called after termination"
      shall: "THE SYSTEM SHALL return a JournalResponse containing an entry for InstanceCancelled"
  unwanted:
    - condition: "IF the actor is already stopped before DELETE arrives"
      shall_not: "THE SYSTEM SHALL NOT panic or hang — it returns 404 (TerminateError::NotFound)"
      because: "ActorRef::call on a dead actor returns SenderError, mapped to TerminateError::NotFound"

contracts:
  preconditions:
    auth_required: false
    required_inputs:
      - field: "HTTP method"
        type: "DELETE"
        constraints: "Must be DELETE"
        example_valid: "DELETE"
      - field: "URL path"
        type: "String"
        constraints: "Format: /api/v1/workflows/<namespace>/<instance_id> where both parts are non-empty"
        example_valid: "/api/v1/workflows/e2e-test/01ARZ3NDEKTSV4RRFFQ69G5FAV"
        example_invalid: "/api/v1/workflows/no-slash"
      - field: "Running WorkflowInstance"
        type: "ActorRef<InstanceMsg>"
        constraints: "Instance must be registered in OrchestratorState.active and in Live phase"
    system_state:
      - "NATS server running on localhost:4222"
      - "JetStream stream vo-events is provisioned"
      - "MasterOrchestrator actor is running with at least one active instance"
      - "EventStore is available (NatsClient with JetStream context)"
  postconditions:
    state_changes:
      - "WorkflowEvent::InstanceCancelled { reason: \"api-terminate\" } is persisted in JetStream stream vo-events"
      - "WorkflowInstance actor is stopped (post_stop called)"
      - "OrchestratorState.active no longer contains the instance_id (actor death deregisters via supervision)"
    return_guarantees:
      - field: "HTTP status on successful terminate"
        guarantee: "204 No Content"
      - field: "HTTP status on non-existent instance"
        guarantee: "404 Not Found with ApiError { error: \"not_found\" }"
      - field: "Journal entries after terminate"
        guarantee: "JournalResponse contains entry with entry_type=Run, status=recorded corresponding to InstanceCancelled"
    side_effects:
      - "JetStream publish to subject wtf.log.<namespace>.<instance_id>"
      - "Actor stop signal sent via myself_ref.stop()"
      - "post_stop hook aborts procedural_task and live_subscription_task"
  invariants:
    - "InstanceCancelled event is ALWAYS published before myself_ref.stop() is called (handlers.rs:155-173)"
    - "Terminate handler ALWAYS replies Ok(()) before stopping (handlers.rs:172-173)"
    - "reason string passed to terminate_workflow is \"api-terminate\" (workflow.rs:88)"
    - "Journal entries are sorted by seq ascending (journal.rs:194-198)"
    - "No unwrap() or expect() in terminate path — all errors mapped to VoError or TerminateError"

research_requirements:
  files_to_read:
    - path: "crates/vo-api/src/handlers/workflow.rs"
      what_to_extract: "terminate_workflow handler (lines 70-95) — DELETE handler, split_path_id, OrchestratorMsg::Terminate construction, map_terminate_result"
    - path: "crates/vo-api/src/app.rs"
      what_to_extract: "Route registration (line 56) — delete(handlers::terminate_workflow), Extension<ActorRef<OrchestratorMsg>> injection"
    - path: "crates/vo-actor/src/master/handlers/terminate.rs"
      what_to_extract: "handle_terminate (lines 8-23) — looks up instance in OrchestratorState.active, calls call_cancel; call_cancel (lines 25-48) — sends InstanceMsg::Cancel via actor_ref.call()"
    - path: "crates/vo-actor/src/instance/handlers.rs"
      what_to_extract: "handle_cancel (lines 143-175) — publishes WorkflowEvent::InstanceCancelled via state.args.event_store, replies Ok(()), calls myself_ref.stop()"
    - path: "crates/vo-storage/src/journal.rs"
      what_to_extract: "append_event (lines 28-50) — publishes to JetStream subject wtf.log.<ns>.<inst>, returns Ok(seq)"
    - path: "crates/vo-common/src/events/mod.rs"
      what_to_extract: "WorkflowEvent::InstanceCancelled { reason: String } (line 28)"
    - path: "crates/vo-api/src/handlers/journal.rs"
      what_to_extract: "get_journal handler (lines 19-69), map_event_fields (lines 111-192) — catch-all at line 183 maps InstanceCancelled to JournalEntryType::Run"
    - path: "crates/vo-actor/src/messages/orchestrator.rs"
      what_to_extract: "OrchestratorMsg::Terminate { instance_id, reason, reply } (lines 34-38)"
    - path: "crates/vo-actor/src/messages/instance.rs"
      what_to_extract: "InstanceMsg::Cancel { reason, reply } (lines 65-68)"
    - path: "crates/vo-actor/src/messages/errors.rs"
      what_to_extract: "TerminateError::NotFound(InstanceId) and TerminateError::Timeout(InstanceId) (lines 20-26)"
    - path: "crates/vo-api/src/handlers/workflow_mappers.rs"
      what_to_extract: "map_terminate_result (lines 131-152) — Ok(()) -> 204, NotFound -> 404, Timeout -> 503"
    - path: "crates/vo-actor/src/instance/actor.rs"
      what_to_extract: "post_stop (lines 73-86) — aborts procedural_task and live_subscription_task"
  research_questions:
    - question: "Does the orchestrator deregister the instance from active on actor death?"
      answered: true
      answer: "Yes — spawn_linked creates a supervision link. When the instance actor stops, the MasterOrchestrator's handle_supervisor_evt fires and calls state.deregister(&id). See master/mod.rs."
    - question: "What does the journal handler's catch-all arm produce for InstanceCancelled?"
      answered: true
      answer: "JournalEntryType::Run, name=Some(\"event\"), status=Some(\"recorded\"), all other fields None. See journal.rs:183-191."
  research_complete_when:
    - "[x] terminate_workflow handler fully understood (workflow.rs:70-95)"
    - "[x] handle_terminate orchestrator flow fully understood (terminate.rs:8-48)"
    - "[x] handle_cancel instance flow fully understood (handlers.rs:143-175)"
    - "[x] append_event JetStream publish flow understood (journal.rs:28-50)"
    - "[x] Journal replay and map_event_fields understood (journal.rs:19-198)"
    - "[x] InstanceCancelled variant confirmed (events/mod.rs:28)"
    - "[x] TerminateError variants confirmed (errors.rs:20-26)"

inversions:
  data_integrity_failures:
    - failure: "Actor stops before InstanceCancelled is published to JetStream (e.g., panic between publish and stop)"
      prevention: "handle_cancel publishes first (line 159), then replies (line 172), then stops (line 173). The reply is sent regardless of publish success — but publish failure is logged (lines 166-169). Test should verify journal contains the event."
      test_for_it: "test_terminate_cancellation_event_in_journal"
    - failure: "Concurrent terminate requests — second DELETE arrives while first is processing"
      prevention: "ActorRef::call is serialized per-actor. The second Cancel will see the actor is stopping. First call returns Ok(()), second likely returns SenderError (mapped to NotFound). Test should verify idempotent behavior."
      test_for_it: "test_double_terminate_second_returns_not_found_or_204"
  ordering_failures:
    - failure: "Journal read returns events before InstanceCancelled (e.g., read before event is published)"
      prevention: "DELETE returns 204 only after InstanceCancelled is published (publish happens before reply in handle_cancel). Add small sleep (50ms) before reading journal to account for JetStream consumer lag."
      test_for_it: "test_journal_read_after_terminate_delay"

acceptance_tests:
  happy_paths:
    - name: "test_terminate_running_workflow_returns_204"
      given: "A running procedural workflow instance with id=<generated>, namespace=\"e2e-term\""
      when: "DELETE /api/v1/workflows/e2e-term/<instance_id>"
      then:
        - "HTTP status is 204 No Content"
        - "No response body"
    - name: "test_terminate_cancellation_event_in_journal"
      given: "A running procedural workflow instance, terminated via DELETE"
      when: "GET /api/v1/workflows/e2e-term/<instance_id>/journal"
      then:
        - "HTTP status is 200"
        - "JournalResponse.entries contains at least one entry where entry_type == Run and name == Some(\"event\") and status == Some(\"recorded\")"
        - "The total entries includes the InstanceCancelled event (catch-all mapping)"
    - name: "test_terminate_actor_stops"
      given: "A running procedural workflow instance, terminated via DELETE returning 204"
      when: "GET /api/v1/workflows/e2e-term/<instance_id>"
      then:
        - "HTTP status is 404 Not Found (actor died, GetStatusError::ActorDied)"
        - "ApiError { error: \"actor_died\", message: \"instance actor is dead\" }"
  error_paths:
    - name: "test_terminate_nonexistent_instance_returns_404"
      given: "No workflow instance with the given id exists"
      when: "DELETE /api/v1/workflows/e2e-term/nonexistent-fake-id"
      then:
        - "HTTP status is 404 Not Found"
        - "ApiError { error: \"not_found\", message: \"nonexistent-fake-id\" }"
    - name: "test_terminate_invalid_id_returns_400"
      given: "Path id has no slash separator"
      when: "DELETE /api/v1/workflows/no-slash-here"
      then:
        - "HTTP status is 400 Bad Request"
        - "ApiError { error: \"invalid_id\", message: \"bad id\" }"
  integration_paths:
    - name: "test_start_and_terminate_full_flow"
      given: "NATS running, orchestrator started, procedural workflow registered"
      when: "POST /api/v1/workflows to start -> verify 201 -> DELETE /api/v1/workflows/e2e-term/<id> -> verify 204 -> wait 100ms -> GET journal -> verify InstanceCancelled present -> GET status -> verify 404"
      then:
        - "All assertions pass in sequence"
        - "Journal contains at least InstanceStarted + InstanceCancelled events"

e2e_tests:
  pipeline_test:
    name: "test_e2e_start_terminate_verify_journal"
    description: "Start a procedural workflow, terminate it, verify InstanceCancelled in journal and actor stopped"
    setup:
      precondition_commands:
        - "docker start vo-nats-test"
    execute:
      steps:
        - description: "Start a procedural workflow with long sleep"
          command: "POST /api/v1/workflows { \"namespace\": \"e2e-term\", \"workflow_type\": \"e2e-terminate-test\", \"paradigm\": \"procedural\", \"input\": {} }"
          expected_status: 201
          extract: "instance_id from response body .instance_id"
        - description: "Wait briefly for instance to be in Live phase"
          command: "sleep 500ms"
        - description: "Terminate the workflow"
          command: "DELETE /api/v1/workflows/e2e-term/<instance_id>"
          expected_status: 204
        - description: "Wait for JetStream consumer lag"
          command: "sleep 100ms"
        - description: "Verify journal contains InstanceCancelled"
          command: "GET /api/v1/workflows/e2e-term/<instance_id>/journal"
          expected_status: 200
          verify: "response.entries contains entry with type=run and status=recorded"
        - description: "Verify actor is gone"
          command: "GET /api/v1/workflows/e2e-term/<instance_id>"
          expected_status: 404
      timeout_ms: 15000
    verify:
      exit_code: 0
      stdout_contains: []
      side_effects:
        - "JetStream stream vo-events contains message on subject wtf.log.e2e-term.<instance_id> with InstanceCancelled payload"

verification_checkpoints:
  gate_0_research:
    name: "Research Gate"
    must_pass_before: "Writing any code"
    checks:
      - "[x] terminate_workflow handler understood (workflow.rs:70-95)"
      - "[x] handle_cancel instance handler understood (handlers.rs:143-175)"
      - "[x] InstanceCancelled event variant confirmed (events/mod.rs:28)"
      - "[x] append_event JetStream publish understood (journal.rs:28-50)"
      - "[x] journal.rs map_event_fields catch-all for InstanceCancelled understood (line 183-191)"
      - "[x] TerminateError variants understood (errors.rs:20-26)"
      - "[x] map_terminate_result status mappings understood (workflow_mappers.rs:131-152)"
    evidence_required:
      - "All function signatures, types, and line numbers documented above"
  gate_1_tests:
    name: "Test Gate"
    must_pass_before: "Writing implementation code"
    checks:
      - "[ ] Integration test file crates/vo-actor/tests/terminate_e2e.rs exists"
      - "[ ] Test spawns MasterOrchestrator with real NATS EventStore"
      - "[ ] Test starts a procedural workflow instance"
      - "[ ] Test sends InstanceMsg::Cancel (or OrchestratorMsg::Terminate) and verifies Ok(())"
      - "[ ] Test reads journal via JetStream replay and finds InstanceCancelled event"
      - "[ ] Test verifies actor is stopped after cancel"
      - "[ ] Test compiles and runs with NATS available"
    evidence_required:
      - "Test file exists and cargo test -p vo-actor --test terminate_e2e compiles"
  gate_2_implementation:
    name: "Implementation Gate"
    must_pass_before: "Declaring task complete"
    checks:
      - "[ ] All tests pass (green) with NATS running"
      - "[ ] No unwrap() or expect() in test code"
      - "[ ] Test uses real NatsClient, not mocks"
      - "[ ] Test cleans up spawned actors"
    evidence_required:
      - "cargo test -p vo-actor --test terminate_e2e shows all green"
      - "cargo test --workspace passes (no regressions)"

implementation_tasks:
  phase_0_research:
    parallelizable: true
    tasks:
      - task: "Understand MasterOrchestrator spawn and supervision setup"
        file: "crates/vo-actor/src/master/mod.rs"
        done_when: "Know how to spawn MasterOrchestrator with OrchestratorConfig that includes EventStore"
      - task: "Understand procedural workflow registration and WorkflowFn trait"
        file: "crates/vo-actor/src/procedural/mod.rs"
        done_when: "Know how to create a test WorkflowFn that sleeps long enough for cancel"
      - task: "Check existing integration test patterns in workspace"
        file: "crates/vo-storage/src/nats.rs"
        done_when: "Know how tests create NatsClient and connect to NATS"
  phase_1_tests_first:
    parallelizable: true
    gate_required: "gate_0_research"
    tasks:
      - task: "Create test file crates/vo-actor/tests/terminate_e2e.rs"
        file: "crates/vo-actor/tests/terminate_e2e.rs"
        done_when: "File created with #[tokio::test] scaffold and use imports"
      - task: "Write test helper: connect to NATS, provision streams, create NatsClient with EventStore"
        file: "crates/vo-actor/tests/terminate_e2e.rs"
        done_when: "Helper function nats_setup() returns (NatsClient, JetStream context) — pattern from existing tests"
      - task: "Write test helper: spawn MasterOrchestrator with EventStore config"
        file: "crates/vo-actor/tests/terminate_e2e.rs"
        done_when: "Helper function spawn_orchestrator(nats_client) returns ActorRef<OrchestratorMsg>"
      - task: "Write test: start procedural workflow, terminate, verify InstanceCancelled in journal, verify actor stopped"
        file: "crates/vo-actor/tests/terminate_e2e.rs"
        done_when: "Test compiles, runs against NATS, all assertions pass"
      - task: "Write test: terminate non-existent instance returns TerminateError::NotFound"
        file: "crates/vo-actor/tests/terminate_e2e.rs"
        done_when: "Test compiles and passes"
  phase_2_implementation:
    parallelizable: false
    gate_required: "gate_1_tests"
    tasks:
      - task: "Ensure OrchestratorConfig in test includes event_store: Some(Arc::new(nats_client))"
        file: "crates/vo-actor/tests/terminate_e2e.rs"
        done_when: "MasterOrchestrator spawned with real EventStore so InstanceCancelled gets published to JetStream"
      - task: "Register a procedural workflow that sleeps long enough (60s) before terminating"
        file: "crates/vo-actor/tests/terminate_e2e.rs"
        done_when: "WorkflowFn registered via OrchestratorState.registry or appropriate mechanism — instance stays alive during cancel"
      - task: "After terminate, read journal via replay_events from JetStream and assert InstanceCancelled present"
        file: "crates/vo-actor/tests/terminate_e2e.rs"
        done_when: "Replay from seq=1, find WorkflowEvent::InstanceCancelled { reason } in events, assert reason == \"api-terminate\""
      - task: "After terminate, verify actor stopped by sending GetStatus and expecting ActorDied/NotFound"
        file: "crates/vo-actor/tests/terminate_e2e.rs"
        done_when: "OrchestratorMsg::GetStatus returns Err(GetStatusError::ActorDied) or Ok(None) after cancel"

failure_modes:
  - symptom: "Test fails to start instance — SpawnFailed error"
    likely_cause: "Missing EventStore in OrchestratorConfig, or NATS not running"
    where_to_look:
      - file: "crates/vo-actor/tests/terminate_e2e.rs"
        what_to_check: "Is nats_client passed as event_store in OrchestratorConfig? Is docker vo-nats-test running?"
    fix_pattern: "Add event_store: Some(Arc::new(nats_client)) to config; run docker start vo-nats-test"
  - symptom: "Terminate returns 204 but InstanceCancelled not in journal"
    likely_cause: "Reading journal too fast before JetStream consumer catches up, or EventStore is None in InstanceArguments"
    where_to_look:
      - file: "crates/vo-actor/tests/terminate_e2e.rs"
        what_to_check: "Is there a tokio::time::sleep after terminate before reading journal? Is OrchestratorConfig.event_store set?"
    fix_pattern: "Add 100ms sleep between terminate and journal read. Verify event_store is Some in config."
  - symptom: "Instance never reaches Live phase — terminate times out"
    likely_cause: "Replay hangs or pre_start fails silently"
    where_to_look:
      - file: "crates/vo-actor/src/instance/init.rs"
        what_to_check: "Is replay_events returning? Is create_replay_consumer timing out on empty stream?"
    fix_pattern: "Ensure ReplayConfig.tail_timeout is reasonable (200ms default). Check tracing logs for replay phase."
  - symptom: "Actor does not stop after cancel"
    likely_cause: "handle_cancel not reached — message dispatched to wrong handler arm"
    where_to_look:
      - file: "crates/vo-actor/src/instance/handlers.rs:15"
        what_to_check: "Is InstanceMsg::Cancel matched in handle_msg at line 23-25? Does it reach handle_cancel?"
    fix_pattern: "Verify the message enum arm. The Cancel variant is at instance.rs:65-68."
  - symptom: "compile error: use of undeclared type ReplayedEvent or ReplayBatch"
    likely_cause: "Missing use import for vo_common::storage::{ReplayBatch, ReplayedEvent}"
    where_to_look:
      - file: "crates/vo-actor/tests/terminate_e2e.rs"
        what_to_check: "Imports section"
    fix_pattern: "Add: use vo_common::storage::{ReplayBatch, ReplayedEvent};"

anti_hallucination:
  read_before_write:
    - file: "crates/vo-actor/tests/"
      must_read_first: true
      key_sections_to_understand:
        - "Check if tests/ directory exists — if not, create it"
        - "Look for existing integration test patterns (e.g., other test files)"
    - file: "crates/vo-actor/src/master/mod.rs"
      must_read_first: true
      key_sections_to_understand:
        - "How MasterOrchestrator is spawned (Actor spawn pattern)"
        - "How OrchestratorConfig is constructed and passed"
        - "How the supervision link is set up (spawn_linked in start.rs:60-63)"
    - file: "crates/vo-actor/src/procedural/mod.rs"
      must_read_first: true
      key_sections_to_understand:
        - "WorkflowFn trait definition"
        - "How procedural workflows are registered in the registry"
        - "start_procedural_workflow function signature"
    - file: "crates/vo-storage/src/nats.rs"
      must_read_first: true
      key_sections_to_understand:
        - "NatsClient struct, NatsConfig, connect() function"
        - "How to create NatsClient for tests"
    - file: "crates/vo-storage/src/provision.rs"
      must_read_first: true
      key_sections_to_understand:
        - "provision_streams() — needed before using JetStream"
    - file: "crates/vo-common/src/storage.rs"
      must_read_first: true
      key_sections_to_understand:
        - "ReplayStream trait — next_event() method"
        - "EventStore trait — publish() and open_replay_stream() methods"
    - file: "crates/vo-actor/Cargo.toml"
      must_read_first: true
      key_sections_to_understand:
        - "dev-dependencies — what testing libraries are available"
        - "dependencies — vo-storage, vo-common available?"
  no_placeholder_values:
    - "Do NOT use placeholder instance IDs — generate via InstanceId::new(ulid::Ulid::new().to_string())"
    - "Do NOT guess the WorkflowFn trait — read procedural/mod.rs to get the exact signature"
    - "Do NOT mock EventStore — use real NatsClient with live NATS"
    - "Do NOT assume journal endpoint is available in actor-level tests — read directly from JetStream via ReplayStream"
    - "Do NOT invent status codes — DELETE returns 204, GET not-found returns 404, bad-id returns 400"
    - "Do NOT skip the JetStream provision step — provision_streams must be called before append_event"

context_survival:
  progress_file:
    path: ".beads/vo-k00f/progress.txt"
    format: "Markdown checklist"
  recovery_instructions: |
    Read progress.txt and continue from last incomplete task.
    Key facts for context recovery:
    - Terminate flow: DELETE -> terminate_workflow (workflow.rs:70) -> OrchestratorMsg::Terminate (orchestrator.rs:34) -> handle_terminate (terminate.rs:8) -> InstanceMsg::Cancel (instance.rs:65) -> handle_cancel (handlers.rs:143) -> append_event InstanceCancelled -> myself_ref.stop()
    - InstanceCancelled variant: WorkflowEvent::InstanceCancelled { reason: String } at events/mod.rs:28
    - JetStream subject: wtf.log.<namespace>.<instance_id> at journal.rs:54-56
    - Test file: crates/vo-actor/tests/terminate_e2e.rs (new file)
    - Requires: NATS running (docker start vo-nats-test), provision_streams called, real NatsClient as EventStore
    - Verification: replay journal via ReplayStream, check for InstanceCancelled; check actor stopped via GetStatus returning ActorDied
    - ACTOR_CALL_TIMEOUT: 5s at handlers/mod.rs:26
    - INSTANCE_CALL_TIMEOUT: check master/handlers/mod.rs for exact value

completion_checklist:
  tests:
    - "[ ] Integration test: terminate running workflow returns Ok / InstanceCancelled published"
    - "[ ] Integration test: journal contains InstanceCancelled after terminate"
    - "[ ] Integration test: actor is stopped after terminate (GetStatus returns ActorDied)"
    - "[ ] Integration test: terminate non-existent instance returns TerminateError::NotFound"
    - "[ ] cargo test -p vo-actor --test terminate_e2e passes"
    - "[ ] cargo test --workspace passes (no regressions)"
  code:
    - "[ ] Zero unwrap() or expect() in test code"
    - "[ ] Test uses real NatsClient, not mocks"
    - "[ ] JetStream streams provisioned before test runs"
    - "[ ] Actors cleaned up after test (stop or let supervision handle it)"
    - "[ ] InstanceCancelled reason verified as \"api-terminate\""
  ci:
    - "[ ] cargo test -p vo-actor --test terminate_e2e passes"
    - "[ ] cargo clippy --workspace -- -D warnings passes"
    - "[ ] cargo check --workspace passes"

context:
  related_files:
    - path: "crates/vo-actor/tests/terminate_e2e.rs"
      relevance: "Primary file — new E2E integration test for terminate path"
    - path: "crates/vo-api/src/handlers/workflow.rs"
      relevance: "terminate_workflow HTTP handler (line 70-95) — DELETE endpoint"
    - path: "crates/vo-actor/src/master/handlers/terminate.rs"
      relevance: "handle_terminate orchestrator handler (line 8-23) — forwards Cancel to instance actor"
    - path: "crates/vo-actor/src/instance/handlers.rs"
      relevance: "handle_cancel (line 143-175) — publishes InstanceCancelled, stops actor"
    - path: "crates/vo-storage/src/journal.rs"
      relevance: "append_event (line 28-50) — JetStream publish, subject wtf.log.<ns>.<id>"
    - path: "crates/vo-common/src/events/mod.rs"
      relevance: "WorkflowEvent::InstanceCancelled { reason } (line 28)"
    - path: "crates/vo-api/src/handlers/journal.rs"
      relevance: "get_journal (line 19-69) — journal read endpoint"
    - path: "crates/vo-actor/src/messages/orchestrator.rs"
      relevance: "OrchestratorMsg::Terminate { instance_id, reason, reply } (line 34-38)"
    - path: "crates/vo-actor/src/messages/instance.rs"
      relevance: "InstanceMsg::Cancel { reason, reply } (line 65-68)"
    - path: "crates/vo-actor/src/messages/errors.rs"
      relevance: "TerminateError enum (line 20-26)"
    - path: "crates/vo-api/src/handlers/workflow_mappers.rs"
      relevance: "map_terminate_result (line 131-152) — 204/404/503 mapping"
    - path: "crates/vo-actor/src/instance/actor.rs"
      relevance: "post_stop (line 73-86) — cleanup on actor stop"
    - path: "crates/vo-actor/src/master/mod.rs"
      relevance: "MasterOrchestrator spawn and supervision"
    - path: "crates/vo-storage/src/nats.rs"
      relevance: "NatsClient, NatsConfig, connect() for test setup"
    - path: "crates/vo-storage/src/provision.rs"
      relevance: "provision_streams() required before JetStream operations"
    - path: "crates/vo-common/src/storage.rs"
      relevance: "EventStore trait (publish), ReplayStream trait (next_event)"
  design_decisions:
    - decision: "Test at actor level (not HTTP level) — use OrchestratorMsg::Terminate directly"
      rationale: "Actor-level tests are faster, don't require spawning axum server, and directly verify the actor chain. HTTP-level tests are covered by separate bead."
      reversible: true
    - decision: "Use real NatsClient, not mocks"
      rationale: "The whole point of E2E is to verify real JetStream persistence of InstanceCancelled. Mocks would not catch serialization or publish failures."
      reversible: false
    - decision: "Use procedural paradigm with long sleep for test workflow"
      rationale: "Procedural is simplest — just register a WorkflowFn. FSM/DAG require full definition objects. Sleep keeps instance alive long enough to send cancel."
      reversible: true

ai_hints:
  do:
    - "Read master/mod.rs to understand how to spawn MasterOrchestrator before writing test"
    - "Read procedural/mod.rs to understand WorkflowFn trait before creating test workflow"
    - "Read nats.rs to understand NatsClient::connect() before writing NATS setup"
    - "Read provision.rs to understand provision_streams() — call it before tests"
    - "Read storage.rs to understand EventStore and ReplayStream traits"
    - "Use InstanceId::new(ulid::Ulid::new().to_string()) for test instance IDs"
    - "Use NamespaceId::new(\"e2e-term\") as test namespace"
    - "Register a procedural WorkflowFn that does tokio::time::sleep(Duration::from_secs(60)) to keep instance alive"
    - "Use OrchestratorMsg::StartWorkflow to start, then OrchestratorMsg::Terminate to cancel"
    - "Use OrchestratorMsg::GetStatus to verify actor is dead after cancel"
    - "Use open_replay_stream + next_event loop to verify InstanceCancelled in journal"
    - "Add tokio::time::sleep(Duration::from_millis(100)) after terminate before reading journal"
    - "Call provision_streams(&jetstream).await before using JetStream"
  do_not:
    - "Do NOT use unwrap() or expect() — use match or map_err"
    - "Do NOT mock EventStore — use real NatsClient"
    - "Do NOT skip provision_streams() — JetStream operations will fail without it"
    - "Do NOT use HTTP endpoints in this test — test at actor level with OrchestratorMsg"
    - "Do NOT forget to add #[cfg_attr(not(feature = \"integration\"), ignore)]" if NATS is not always available"
    - "Do NOT assume the orchestrator registry already has workflows — register your test WorkflowFn"
    - "Do NOT leak actors — stop the MasterOrchestrator when test completes"
  constitution:
    - "Zero unwrap law: NEVER use .unwrap() or .expect()"
    - "Real infra: Use real NATS, not mocks — this is E2E"
    - "Test isolation: Each test gets its own namespace and instance ID"
    - "Cleanup: Stop spawned actors in test teardown"
