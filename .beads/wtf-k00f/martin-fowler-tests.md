# Martin Fowler Test Plan: E2E Terminate Workflow

## Happy Path Tests

### `test_terminate_running_instance_returns_ok`
Verifies that sending `OrchestratorMsg::Terminate` to a running instance returns `Ok(())`.

### `test_terminate_publishes_instance_cancelled_to_jetstream`
Verifies that `WorkflowEvent::InstanceCancelled { reason: "api-terminate" }` appears in the JetStream journal after termination.

### `test_terminate_stops_instance_actor`
Verifies that a subsequent `OrchestratorMsg::GetStatus` returns `Err(GetStatusError::ActorDied)` after successful termination.

### `test_terminate_reason_is_api_terminate`
Verifies that the `reason` field in `WorkflowEvent::InstanceCancelled` equals `"api-terminate"`.

### `test_journal_contains_instance_started_and_instance_cancelled`
Verifies that the full journal for a terminated workflow contains both `InstanceStarted` and `InstanceCancelled` events in ascending `seq` order.

---

## Error Path Tests

### `test_terminate_nonexistent_instance_returns_not_found`
Verifies that `OrchestratorMsg::Terminate` for an instance not in `active` returns `Err(TerminateError::NotFound(id))`.

### `test_terminate_after_actor_already_stopped_returns_not_found`
Verifies that terminating an instance whose actor has already stopped returns `Err(TerminateError::NotFound)` (via `SenderError` mapping at `terminate.rs:45`).

### `test_terminate_returns_timeout_when_instance_does_not_respond`
Verifies that `OrchestratorMsg::Terminate` returns `Err(TerminateError::Timeout(id))` when the instance actor does not reply to `InstanceMsg::Cancel` within `INSTANCE_CALL_TIMEOUT` (5s).

**Implementation approach:** Use a custom `WorkflowFn` that deliberately drops the `reply` port inside its procedural task, causing the `Cancel` handler to hang (the cancel message arrives but the handler's `reply` send is never reached because the task is blocking). Alternatively, use a test-only `InstanceMsg` handler that swallows `Cancel`. The key is that `call_cancel` in `terminate.rs:30-35` hits `Some(INSTANCE_CALL_TIMEOUT)` and `call_result` matches `CallResult::Timeout` at line 44. Since `INSTANCE_CALL_TIMEOUT` is 5s in production, the test must either (a) mock the timeout to a shorter duration, or (b) use a dedicated test instance that consumes the `Cancel` message without replying and assert the full 5s timeout. Prefer (a) if the timeout is injectable; otherwise (b) with a reduced test-only timeout constant.

---

## Edge Case Tests

### `test_journal_read_after_terminate_returns_entries_sorted_by_seq`
Verifies that journal entries returned by `open_replay_stream` are in strictly ascending `seq` order, with `InstanceCancelled` having the highest `seq`.

### `test_terminate_produces_no_unwrap_panic`
Verifies that the entire terminate chain completes without any `unwrap()` or `expect()` panic (enforced by clippy lint in source; test confirms at runtime).

### `test_instance_cancelled_event_has_correct_journal_entry_mapping`
Verifies that `WorkflowEvent::InstanceCancelled` maps to `JournalEntry { entry_type: Run, name: Some("event"), status: Some("recorded") }` via the catch-all arm in `map_event_fields`.

### `test_terminate_with_no_event_store_still_stops_actor`
Verifies that when `OrchestratorConfig.event_store` is `None` (no JetStream), `handle_cancel` skips the publish, still replies `Ok(())`, and still calls `myself_ref.stop()`.

**Implementation approach:** Spawn `MasterOrchestrator` with `event_store: None` in `OrchestratorConfig`. Start and terminate a workflow instance. Assert: (1) reply is `Ok(())`, (2) subsequent `GetStatus` returns `ActorDied`. Do NOT assert journal contents — there is no journal. This tests the `None` branch at `handlers.rs:208` (`if let Some(store) = ...`).

### `test_terminate_when_jetstream_publish_fails_still_stops_actor`
Verifies that when `EventStore::publish` returns `Err`, `handle_cancel` logs the error, still replies `Ok(())`, and still calls `myself_ref.stop()` — resulting in a data-loss scenario where `InstanceCancelled` is NOT persisted but the actor IS stopped.

**Implementation approach:** Use a `MockEventStore` (only for this test) whose `publish()` method returns `Err(WtfError::nats_publish("simulated failure"))`. Assert: (1) reply is `Ok(())`, (2) actor is dead after terminate, (3) journal replay does NOT contain `InstanceCancelled`. This verifies the defensive error-logging path at `handlers.rs:213-218` and the data-loss risk documented in the comment.

> **Note:** This is the ONLY test in the plan that uses a mock. All other tests use real `NatsClient`. The mock is justified because inducing a real JetStream publish failure would require killing the NATS server mid-stream, which is unreliable and affects other tests.

---

## Contract Verification Tests

### `test_precondition_cancel_reply_sent_before_actor_stop`
Verifies that `handle_cancel` sends `Ok(())` on the reply port and THEN calls `myself_ref.stop()`. This is a **structural invariant** verified by code review (line 222 reply, line 223 stop), not by runtime timing.

**Verification approach (documentation-only, not a runtime test):** The invariant `I-2 (Reply-before-stop)` is enforced by sequential statement ordering in `handle_cancel` (`handlers.rs:222-223`). A runtime race-condition test cannot prove this — the reply send and stop call are in the same async function with no `.await` between them, making them effectively atomic from the caller's perspective. Instead, this invariant is enforced by:
1. **Code review gate:** PR review checklist item: "handle_cancel reply precedes stop (line 222 vs 223)"
2. **Regression guard:** A `#[test]` that loads the source file and asserts the string `"reply.send"` appears before `"myself_ref.stop"` in `handle_cancel` via a static analysis assertion (regex or string search on the source text).

### `test_precondition_instancecancelled_published_before_stop`
Verifies that `handle_cancel` publishes `InstanceCancelled` to JetStream BEFORE calling `myself_ref.stop()`. This is a **structural invariant** verified by code review (line 209-220 publish, line 223 stop), not by runtime timing.

**Verification approach (documentation-only, not a runtime test):** The invariant `I-1 (Event-before-stop)` is enforced by sequential statement ordering (`handlers.rs:209-223`). A runtime test cannot reliably distinguish "publish ACK received" from "consumer received the message" — the JetStream consumer may lag. Instead, this invariant is enforced by:
1. **Code review gate:** PR review checklist item: "handle_cancel publish precedes stop (line 209 vs 223)"
2. **Regression guard:** A `#[test]` that loads the source file and asserts `"store.publish"` appears before `"myself_ref.stop"` in `handle_cancel` via a static analysis assertion.

> **Rationale for removing the original runtime tests:** The original `test_precondition_event_published_before_actor_stops` subscribed to a JetStream consumer and checked `ActorRef::is_alive()` — but between the publish ACK returning to the handler and a downstream consumer receiving the message, the actor may have already stopped. The original `test_precondition_reply_sent_before_actor_stops` attempted to observe reply-before-stop ordering via a subsequent `GetStatus` call — but by the time the caller processes the reply, stop may have already executed. Both are fundamentally racy. The structural assertions above are the correct approach.

### `test_postcondition_instance_removed_from_active_after_supervision`
Verifies invariant I-4: after termination, `OrchestratorMsg::GetStatus` returns `Err(GetStatusError::ActorDied)` (indicating supervision deregistered the instance from `active`).

### `test_invariant_no_duplicate_events_in_journal`
Verifies that exactly one `WorkflowEvent::InstanceCancelled` event exists in the journal for the terminated instance.

---

## Given-When-Then Scenarios

### Scenario 1: Successful termination of a running procedural workflow

```
Given: NATS server running on localhost:4222
  And: JetStream stream wtf-events is provisioned
  And: MasterOrchestrator actor is spawned with real NatsClient as EventStore
  And: A procedural WorkflowFn "e2e-terminate-test" is registered (sleeps 60s)
  And: A workflow instance is started via OrchestratorMsg::StartWorkflow
  And: The instance is in the Live phase (waited ~500ms)

When: OrchestratorMsg::Terminate { instance_id, reason: "api-terminate", reply } is sent

Then: The reply receives Ok(())
  And: open_replay_stream from seq=1, polled with retry loop (up to 2s, 50ms backoff),
       contains WorkflowEvent::InstanceCancelled { reason: "api-terminate" }
  And: OrchestratorMsg::GetStatus for the same instance returns Err(GetStatusError::ActorDied)
  And: The journal entries are sorted by ascending seq
  And: The journal contains at least InstanceStarted + InstanceCancelled events
```

### Scenario 2: Terminate non-existent instance

```
Given: MasterOrchestrator is running with no active instances
  And: instance_id = InstanceId::new("nonexistent-fake-id")

When: OrchestratorMsg::Terminate { instance_id, reason: "test", reply } is sent

Then: The reply receives Err(TerminateError::NotFound(instance_id))
  And: matches!(reply_err, TerminateError::NotFound(id) if id.as_str() == "nonexistent-fake-id")
  And: The error message contains "nonexistent-fake-id"
```

### Scenario 3: Terminate already-stopped instance

```
Given: A workflow instance was started and then terminated (Scenario 1 completed)
  And: The instance actor is confirmed dead via GetStatus returning ActorDied

When: OrchestratorMsg::Terminate { same instance_id, reason: "again", reply } is sent

Then: The reply receives Err(TerminateError::NotFound(instance_id))
  And: matches!(reply_err, TerminateError::NotFound(id) if id == instance_id)
  And: The error is specifically the NotFound variant — NOT Timeout, NOT any other variant
  And: No panic occurs
  And: No hang occurs (reply received within ACTOR_CALL_TIMEOUT)
```

### Scenario 4: Verify reason string propagation

```
Given: A running procedural workflow instance (Scenario 1 setup)

When: OrchestratorMsg::Terminate { instance_id, reason: "my-custom-reason", reply } is sent

Then: The reply receives Ok(())
  And: The journal contains WorkflowEvent::InstanceCancelled { reason: "my-custom-reason" }
```

### Scenario 5: Journal ordering after terminate

```
Given: A workflow instance that has been started and terminated (Scenario 1)

When: open_replay_stream(namespace, instance_id, 1) is called and all events are collected

Then: Events are returned in ascending seq order
  And: The last event is WorkflowEvent::InstanceCancelled
  And: The InstanceCancelled event has the highest seq number
  And: There are no gaps in seq (consecutive)
```

### Scenario 6: InstanceCancelled maps to correct JournalEntry shape

```
Given: A workflow instance that has been terminated (Scenario 1)
  And: The journal has been replayed

When: The last journal entry (corresponding to InstanceCancelled) is inspected

Then: entry.entry_type == JournalEntryType::Run
  And: entry.name == Some("event")
  And: entry.status == Some("recorded")
  And: entry.input == None
  And: entry.output == None
  And: entry.duration_ms == None
```

### Scenario 7: Terminate when instance does not respond to Cancel

```
Given: A running workflow instance that deliberately drops the Cancel reply port
  And: INSTANCE_CALL_TIMEOUT is set to a test-appropriate value (injected or overridden)

When: OrchestratorMsg::Terminate { instance_id, reason: "test-timeout", reply } is sent

Then: The reply receives Err(TerminateError::Timeout(instance_id))
  And: matches!(reply_err, TerminateError::Timeout(id) if id == instance_id)
  And: The error is specifically the Timeout variant — NOT NotFound
  And: The reply is received within ACTOR_CALL_TIMEOUT + 500ms
```

### Scenario 8: Terminate with no EventStore configured

```
Given: MasterOrchestrator spawned with event_store: None in OrchestratorConfig
  And: A procedural WorkflowFn registered that sleeps 60s
  And: A workflow instance is started and in the Live phase

When: OrchestratorMsg::Terminate { instance_id, reason: "no-store", reply } is sent

Then: The reply receives Ok(())
  And: OrchestratorMsg::GetStatus returns Err(GetStatusError::ActorDied)
  And: No journal exists for this instance (EventStore was None)
```

### Scenario 9: Terminate when JetStream publish fails (data loss)

```
Given: MasterOrchestrator spawned with MockEventStore whose publish() returns
       Err(WtfError::nats_publish("simulated failure"))
  And: A workflow instance is started and in the Live phase

When: OrchestratorMsg::Terminate { instance_id, reason: "publish-fail", reply } is sent

Then: The reply receives Ok(())
  And: OrchestratorMsg::GetStatus returns Err(GetStatusError::ActorDied)
  And: The MockEventStore recorded that publish was attempted with
       WorkflowEvent::InstanceCancelled { reason: "publish-fail" }
  And: The journal replay (if attempted) does NOT contain InstanceCancelled
```

---

## Test Execution Notes

1. **Test isolation:** Each test gets its own namespace (`"e2e-term-<test-name>"`) and unique `InstanceId` via `ulid::Ulid::new()`.
2. **NATS dependency:** Tests require `#[cfg_attr(not(feature = "integration"), ignore)]` -- they are only run when the `integration` feature flag is enabled.
3. **Timing — retry loop, NOT fixed sleep:** After terminate, journal reads use a retry loop with exponential backoff instead of `tokio::time::sleep(100ms)`:
   ```rust
   const JOURNAL_POLL_TIMEOUT: Duration = Duration::from_secs(2);
   const JOURNAL_POLL_INTERVAL: Duration = Duration::from_millis(50);

   let events = tokio::time::timeout(JOURNAL_POLL_TIMEOUT, async {
       loop {
           match replay_all_events(&store, &namespace, &instance_id).await {
               Ok(events) if events.iter().any(|e| matches!(e, WorkflowEvent::InstanceCancelled { .. })) => {
                   break events;
               }
               _ => tokio::time::sleep(JOURNAL_POLL_INTERVAL).await,
           }
       }
   })
   .await
   .expect("timed out waiting for InstanceCancelled in journal");
   ```
   This eliminates the fixed 100ms sleep and replaces it with a polling approach that succeeds as soon as the event is available, with a hard timeout to prevent infinite loops.
4. **Cleanup:** `MasterOrchestrator` must be stopped after each test to prevent actor leaks.
5. **Provision:** `provision_streams(&jetstream).await` MUST be called before any JetStream operations.
6. **Zero unwrap law:** No `.unwrap()` or `.expect()` in test code. All fallible operations use `match`, `?`, or `map_err`. The one exception is `tokio::time::timeout().await.expect("timed out waiting for ...")` which is an intentional test-failure assertion (the test SHOULD fail if the timeout fires).
7. **Structural invariant tests (Scenarios for I-1, I-2):** Use source-level string analysis (regex or `include_str!` + string search) to assert statement ordering in `handle_cancel`. These do NOT require NATS and run as unit tests.
8. **Mock-only test:** Scenario 9 is the ONLY test that uses a `MockEventStore`. All other scenarios use real `NatsClient`. The mock must implement `EventStore` trait and be in a `#[cfg(test)]` module.

---

## LOW-PRIORITY ADVISORIES (not blocking)

These are noted but NOT required for test plan approval:

- **A1:** Consider a fuzz test for malformed `InstanceId` values passed to Terminate.
- **A2:** Consider a test for `MessagingErr::ChannelClosed` when the MasterOrchestrator itself dies mid-terminate (requires killing the orchestrator, complex to set up).
- **A3:** Consider parameterizing the retry-loop constants so CI environments with slower JetStream can increase timeouts.
- **A4:** Consider a test for the "actor dying between `state.get()` and `call_cancel()`" race — currently produces `TerminateError::NotFound` via `SenderError` mapping (already covered by Scenario 3).
