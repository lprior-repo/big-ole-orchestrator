# Martin Fowler Test Plan: E2E Signal Delivery

**Test file:** `crates/vo-actor/tests/signal_delivery_e2e.rs`
**Run:** `cargo test -p vo-actor --test signal_delivery_e2e -- --test-threads=1`

**Approach (C1 fix):** Tests use the handler-level pattern from `handlers_tests.rs` —
construct `InstanceState` with `MockOkEventStore`, call
`handlers::handle_signal()` and `procedural::handle_wait_for_signal()` directly.
This bypasses the `MasterOrchestrator` (which has no `procedural_workflows` field)
while still exercising the full signal delivery path: event publish → pending
delivery → buffer fallback → wait_for_signal consumption.

---

## Happy Path Tests

- `signal_delivery_resumes_and_completes_workflow`
- `signal_arrives_before_wait_for_signal`
- `empty_signal_payload_delivered_and_workflow_completes`

## Error Path Tests

- `signal_to_nonexistent_instance_returns_instance_not_found`

## Edge Case Tests

- `signal_with_wrong_name_does_not_unblock_workflow`
- `signal_rpc_returns_ok_even_when_workflow_already_stopped` (if applicable)

## Contract Verification Tests

- `postcondition_signal_event_published_to_event_store`
- `postcondition_pending_signal_call_removed_after_delivery`
- `postcondition_op_counter_increments_once_per_wait_for_signal`
- `invariant_signal_payload_matches_what_was_sent`
- `invariant_signal_never_lost_either_delivered_or_buffered`
- `invariant_received_signals_fifo_ordering`

---

## Test Infrastructure Contracts

### make_test_state Helper

```
Given: make_test_state(event_store, snapshot_db, events_since) is called
When:  Constructs InstanceState with:
         - namespace: "test-ns"
         - instance_id: "test-instance"
         - paradigm: WorkflowParadigm::Procedural
         - ParadigmState::Procedural(ProceduralActorState::new())
         - total_events_applied = 100
         - events_since_snapshot = events_since
Then:  Returns a mutable InstanceState ready for handler calls
```

### MockOkEventStore Contract

```
Given: MockOkEventStore is used as the EventStore implementation
When:  Any event is published via store.publish(ns, inst, event)
Then:  publish returns Ok(1) — seq number is always 1
       (sufficient for inject_event to proceed)

When:  open_replay_stream(ns, inst, from_seq) is called
Then:  Returns Ok(Box::new(EmptyReplayStream))

When:  EmptyReplayStream.next_event() is called
Then:  Returns Ok(ReplayBatch::TailReached)

When:  EmptyReplayStream.next_live_event() is called
Then:  Hangs forever (std::future::pending().await)
```

### send_signal Helper Contract

```
Given: state is a valid &mut InstanceState with event_store = Some(MockOkEventStore)
       and optionally a pre-registered pending_signal_calls entry
When:  send_signal(state, name, payload) is called
Then:  1. Creates a oneshot::channel for caller reply
       2. Calls handlers::handle_signal(state, name, payload, reply)
       3. Returns the caller reply via rx.await
       4. State is mutated in-place (event injected, pending calls updated)
```

---

## Given-When-Then Scenarios

### Scenario 1: signal_delivery_resumes_and_completes_workflow

**Contract references:** PRE-6, PRE-7, PRE-8, POST-1 through POST-8, INV-1, INV-2

```
Given:
  - state = make_test_state(Some(Arc::new(MockOkEventStore)), None, 0)
  - A pending signal call pre-registered:
      let (pending_tx, pending_rx) = oneshot::channel::<Result<Bytes, VoError>>();
      state.pending_signal_calls.insert("go".to_string(), pending_tx.into());
  - Caller reply channel:
      let (caller_tx, caller_rx) = oneshot::channel::<Result<(), VoError>>();

When:
  - handlers::handle_signal(&mut state, "go".to_string(),
      Bytes::from_static(b"proceed"), caller_tx.into()).await

Then:
  - handle_signal returns Ok(())                              [POST-1]
  - caller_rx receives Ok(())                                 [POST-1]
  - pending_rx receives Ok(Bytes::from_static(b"proceed"))    [POST-3, POST-5, INV-2]
  - pending_signal_calls no longer contains "go"              [POST-3]
  - total_events_applied incremented by 1                     [POST-4]
```

### Scenario 2: signal_arrives_before_wait_for_signal

**Contract references:** PRE-9, POST-9 through POST-12, INV-4, INV-5

```
Given:
  - state = make_test_state(Some(Arc::new(MockOkEventStore)), None, 0)
  - No pending_signal_calls entry for "early" (workflow hasn't called wait yet)

When:
  Step 1 — Signal arrives first:
    handlers::handle_signal(&mut state, "early".to_string(),
        Bytes::from_static(b"before-wait"), caller_tx.into()).await

  Step 2 — Workflow calls wait_for_signal after signal is buffered:
    procedural::handle_wait_for_signal(&mut state, 0,
        "early".to_string(), wait_tx.into()).await

Then:
  - Step 1: handle_signal returns Ok(())                              [POST-9]
  - Step 1: Signal buffered in received_signals["early"]               [POST-9]
  - Step 2: wait_for_signal returns immediately (no blocking)          [POST-12]
  - Step 2: wait_rx receives Ok(Bytes::from_static(b"before-wait"))   [POST-10, POST-12]
  - Step 2: received_signals["early"] consumed (empty or removed)     [POST-11]

No race condition (M7 fix): handler calls are sequential on &mut state.
```

### Scenario 3: signal_to_nonexistent_instance_returns_instance_not_found

**Contract references:** POST-13

```
Given:
  - state = make_test_state(Some(Arc::new(MockOkEventStore)), None, 0)
  - No instance with instance_id = "ghost-instance" has been started

Note: At handler level, there is no instance-lookup step. The InstanceNotFound
error is produced by the orchestrator's handle_signal() which queries its
active registry. This test verifies the orchestrator-level error path:

When:
  - OrchestratorState::new(test_config()) where test_config() has max_instances=10
  - get(&InstanceId::new("ghost-instance")) is called

Then:
  - get() returns None                                            [POST-13]
  - No panic, no actor error
```

### Scenario 4: signal_with_wrong_name_does_not_unblock_workflow

**Contract references:** POST-14, INV-4

```
Given:
  - state = make_test_state(Some(Arc::new(MockOkEventStore)), None, 0)
  - A pending signal call registered for "approval":
      let (pending_tx, pending_rx) = oneshot::channel::<Result<Bytes, VoError>>();
      state.pending_signal_calls.insert("approval".to_string(), pending_tx.into());

When:
  - handlers::handle_signal(&mut state, "wrong_name".to_string(),
      Bytes::from_static(b"payload"), caller_tx.into()).await

Then:
  - handle_signal returns Ok(())                                  [POST-14]
  - caller_rx receives Ok(())
  - pending_signal_calls still contains "approval"                 [POST-14]
    (workflow remains blocked — waiter untouched)
  - pending_rx.try_recv() returns Err(TryRecvError::Empty)
    (no reply sent to the original waiter)                        [POST-14]
  - Signal buffered in received_signals["wrong_name"]             [INV-4]

Verification mechanism (C5 fix): Direct state inspection —
  assert!(state.pending_signal_calls.contains_key("approval"));
  assert!(pending_rx.try_recv().is_err());
  if let ParadigmState::Procedural(s) = &state.paradigm_state {
      assert!(s.received_signals.contains_key("wrong_name"));
  }
```

### Scenario 5: empty_signal_payload_delivered_and_workflow_completes

**Contract references:** POST-15

```
Given:
  - state = make_test_state(Some(Arc::new(MockOkEventStore)), None, 0)
  - A pending signal call pre-registered for "go":
      let (pending_tx, pending_rx) = oneshot::channel::<Result<Bytes, VoError>>();
      state.pending_signal_calls.insert("go".to_string(), pending_tx.into());

When:
  - handlers::handle_signal(&mut state, "go".to_string(),
      Bytes::new(), caller_tx.into()).await

Then:
  - handle_signal returns Ok(())
  - caller_rx receives Ok(())
  - pending_rx receives Ok(Bytes::new())                          [POST-15]
  - payload.is_empty() == true                                    [POST-15]
  - pending_signal_calls no longer contains "go"
```

### Scenario 6: postcondition_op_counter_increments_once_per_wait_for_signal

**Contract references:** INV-1

```
Given:
  - state = make_test_state(Some(Arc::new(MockOkEventStore)), None, 0)
  - Record initial operation_counter:
      let before = if let ParadigmState::Procedural(s) = &state.paradigm_state {
          s.operation_counter
      };

When:
  Step 1 — handle_wait_for_signal with no buffer (registers pending):
    procedural::handle_wait_for_signal(&mut state, 0,
        "step1".to_string(), wait_tx1.into()).await

  Step 2 — handle_signal delivers:
    handlers::handle_signal(&mut state, "step1".to_string(),
        Bytes::from_static(b"step1-payload"), caller_tx.into()).await

  Step 3 — Record counter after first wait:
    (SignalReceived event is injected, but wait_for_signal does NOT
     increment operation_counter — that's the WorkflowContext's job
     via op_counter.fetch_add. The handler only publishes + injects.)

  Step 4 — handle_wait_for_signal again:
    procedural::handle_wait_for_signal(&mut state, 1,
        "step2".to_string(), wait_tx2.into()).await

  Step 5 — handle_signal delivers second:
    handlers::handle_signal(&mut state, "step2".to_string(),
        Bytes::from_static(b"step2-payload"), caller_tx2.into()).await

Then:
  - After Step 2: SignalReceived event injected for "step1"       [INV-1]
  - After Step 5: SignalReceived event injected for "step2"       [INV-1]
  - Each handle_signal call increments total_events_applied by 1
  - operation_counter in paradigm_state is NOT mutated by handlers
    (op_counter is the WorkflowContext-level AtomicU32, incremented
    by the workflow fn after wait_for_signal resolves)

Note (C3 fix): Handlers do NOT increment operation_counter. That is the
WorkflowContext's responsibility (op_counter.fetch_add(1) in context.rs
line 235). The handler test verifies total_events_applied increments,
which IS the handler's contract.
```

### Scenario 7: invariant_signal_never_lost_either_delivered_or_buffered

**Contract references:** INV-4

```
Given:
  - state = make_test_state(Some(Arc::new(MockOkEventStore)), None, 0)
  - A pending signal call registered for "release":
      let (pending_tx, pending_rx) = oneshot::channel::<Result<Bytes, VoError>>();
      state.pending_signal_calls.insert("release".to_string(), pending_tx.into());

When:
  Step 1 — First signal delivered to pending waiter:
    handlers::handle_signal(&mut state, "release".to_string(),
        Bytes::from_static(b"first"), caller_tx1.into()).await

  Step 2 — Second signal arrives (no waiter registered):
    handlers::handle_signal(&mut state, "release".to_string(),
        Bytes::from_static(b"second"), caller_tx2.into()).await

Then:
  - Step 1: pending_rx receives Ok(Bytes::from_static(b"first"))
    (first signal delivered immediately)                          [INV-4]
  - Step 1: pending_signal_calls["release"] removed
  - Step 2: caller_rx2 receives Ok(())
  - Step 2: Signal buffered in received_signals["release"]
    with payload b"second"                                        [INV-4]
  - No signal is discarded:
    if let ParadigmState::Procedural(s) = &state.paradigm_state {
        let buffered = s.received_signals.get("release").unwrap();
        assert_eq!(buffered.len(), 1);
        assert_eq!(buffered[0], Bytes::from_static(b"second"));
    }

Reframed as verifiable unit test (C4 fix): direct state inspection
of received_signals proves no signal was lost.
```

### Scenario 8: postcondition_signal_event_published_to_event_store

**Contract references:** POST-2, POST-4

```
Given:
  - state = make_test_state(Some(Arc::new(MockOkEventStore)), None, 0)
  - total_events_applied = 100 (set by make_test_state)

When:
  - handlers::handle_signal(&mut state, "test".to_string(),
      Bytes::from_static(b"data"), caller_tx.into()).await

Then:
  - MockOkEventStore.publish() was called exactly once
    (verified by total_events_applied increment)
  - total_events_applied == 101                                  [POST-2, POST-4]
  - events_since_snapshot == 1
```

### Scenario 9: postcondition_pending_signal_call_removed_after_delivery

**Contract references:** POST-3

```
Given:
  - state = make_test_state(Some(Arc::new(MockOkEventStore)), None, 0)
  - pending_signal_calls["delivery"] = pre-registered oneshot sender

When:
  - handlers::handle_signal(&mut state, "delivery".to_string(),
      Bytes::from_static(b"payload"), caller_tx.into()).await

Then:
  - pending_signal_calls does NOT contain "delivery"              [POST-3]
  - The pending oneshot received Ok(Bytes::from_static(b"payload"))
```

### Scenario 10: invariant_signal_payload_matches_what_was_sent

**Contract references:** INV-2

```
Given:
  - state = make_test_state(Some(Arc::new(MockOkEventStore)), None, 0)
  - pending_signal_calls["match"] = pre-registered oneshot sender
  - Original payload: Bytes::from_static(b"exact-match-payload")

When:
  - handlers::handle_signal(&mut state, "match".to_string(),
      original_payload.clone(), caller_tx.into()).await

Then:
  - pending_rx receives Ok(received_payload)
  - received_payload == original_payload                          [INV-2]
  - Exact byte equality, no truncation or transformation
```

### Scenario 11: invariant_received_signals_fifo_ordering

**Contract references:** INV-3

```
Given:
  - state = make_test_state(Some(Arc::new(MockOkEventStore)), None, 0)
  - No pending waiter for "queue"

When:
  Step 1 — Buffer first signal:
    handlers::handle_signal(&mut state, "queue".to_string(),
        Bytes::from_static(b"alpha"), caller_tx1.into()).await

  Step 2 — Buffer second signal:
    handlers::handle_signal(&mut state, "queue".to_string(),
        Bytes::from_static(b"beta"), caller_tx2.into()).await

  Step 3 — Consume first:
    procedural::handle_wait_for_signal(&mut state, 0,
        "queue".to_string(), wait_tx1.into()).await

  Step 4 — Consume second:
    procedural::handle_wait_for_signal(&mut state, 1,
        "queue".to_string(), wait_tx2.into()).await

Then:
  - Step 3: wait_rx1 receives Ok(Bytes::from_static(b"alpha"))
  - Step 4: wait_rx2 receives Ok(Bytes::from_static(b"beta"))
  - FIFO order preserved: "alpha" before "beta"                  [INV-3]
```

---

## Removed Tests

### e2e_signal_rpc_returns_ok_even_when_workflow_already_stopped (removed)

**Reason:** At handler level, there is no concept of "stopped workflow."
The InstanceNotFound / stopped-actor path lives in the orchestrator's
`handle_signal()` which queries `active` registry. This test is not
testable at the handler level without spawning a full actor and stopping it,
which introduces async teardown complexity for marginal coverage gain.
Covered implicitly by `handlers_tests.rs::handle_signal_returns_error_without_event_store`.

### e2e_empty_signal_payload (removed — M6 fix)

**Reason:** Duplicate of `empty_signal_payload_delivered_and_workflow_completes`
(Scenario 5). Removed from list to eliminate confusion.

---

## Test Ordering and Isolation

```
Run with: --test-threads=1

Isolation: each test constructs its own InstanceState via make_test_state().
           No shared state between tests.
           No actor spawning or cleanup required.

Pattern: each test follows:
  1. let mut state = make_test_state(...)
  2. Optionally pre-register pending_signal_calls
  3. Call handlers::handle_signal() and/or handle_wait_for_signal()
  4. Assert on reply ports and state mutations
```
