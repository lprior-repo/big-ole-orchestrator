# vo-cedw Implementation Summary

- **bead_id:** vo-cedw
- **bead_title:** instance: Implement handle_signal wake in instance handlers
- **phase:** STATE-3
- **updated_at:** 2026-03-23T00:00:00Z
- **status:** ALREADY IMPLEMENTED by vo-88f4 + vo-3cv7

## Analysis

This bead's spec (`.beads/vo-cedw/spec.md`) defines 7 objectives across 4 affected files. After thorough code inspection, **all objectives are fully implemented** by the combination of prior beads vo-88f4 and vo-3cv7. No code changes were required.

### Objective-by-Objective Evidence

| # | Objective | Status | Evidence |
|---|-----------|--------|----------|
| 1 | Persist signals to event store as `WorkflowEvent::SignalReceived` | ✅ Done | `handlers.rs:148-155` — `handle_signal` publishes `WorkflowEvent::SignalReceived { signal_name, payload }` to the event store via `store.publish()` |
| 2 | Store signal in `InstanceState` for replay | ✅ Done | `state.rs:33-35` — `pending_signal_calls: HashMap<String, RpcReplyPort<Result<Bytes, VoError>>>` field exists and initialized to `HashMap::new()` at `state.rs:57` |
| 3 | Wake pending `wait_for_signal` caller via `RpcReplyPort` | ✅ Done | `handlers.rs:159` — `state.pending_signal_calls.remove(&signal_name)` sends payload through port; also `handlers.rs:121-129` wakes during `handle_inject_event_msg` replay |
| 4 | `handle_signal` stub replaced with real implementation | ✅ Done | `handlers.rs:134-177` — full implementation with event store publish, waiter wake, signal buffering, and error handling |
| 5 | `InstanceMsg::ProceduralWaitForSignal` variant exists | ✅ Done | `messages/instance.rs:92-96` — variant with `operation_id`, `signal_name`, `reply` fields |
| 6 | `WorkflowContext::wait_for_signal()` method | ✅ Done | `procedural/context.rs:195-240` — checkpoint-first replay pattern, dispatches `ProceduralWaitForSignal` on live path |
| 7 | `handle_inject_event_msg` wakes signal waiters on replay | ✅ Done | `handlers.rs:121-129` — `SignalReceived` match arm removes from `pending_signal_calls` and sends `Ok(payload)` |

### File-by-File Verification

| File (spec §8) | Change Required | Found In |
|----------------|----------------|----------|
| `instance/state.rs` | Add `pending_signal_calls` field | Lines 33-35: field declared; Line 57: initialized in `initial()` |
| `messages/instance.rs` | Add `ProceduralWaitForSignal` variant | Lines 92-96: full variant with all fields per spec §6.3 |
| `instance/handlers.rs` | Replace stub; add wake arms; add handler arm | Lines 70-76: `ProceduralWaitForSignal` arm; Lines 121-129: `SignalReceived` wake; Lines 134-177: full `handle_signal` implementation |
| `procedural/context.rs` | Add `wait_for_signal()` method | Lines 195-240: checkpoint-first replay pattern per spec §6.4 |

### Additional Implementation (Beyond Spec)

The prior beads implemented **signal buffering** (`handle_signal` lines 161-167) — when a signal arrives before a waiter registers, it's stored in `ProceduralActorState.received_signals`. The `handle_wait_for_signal` handler (`procedural.rs:97-128`) checks for buffered signals first, consuming FIFO from the vec. This addresses the risk from spec §10 about "Signal arrives before waiter registers."

`ProceduralActorState.received_signals` is in `procedural/state/mod.rs:48-49` with `#[serde(default)]` for snapshot compatibility.

### Tests Found (All Passing)

| Test | Location | Pass/Fail |
|------|----------|-----------|
| `initial_state_has_empty_pending_signal_calls` | `handlers.rs` | ✅ PASS |
| `handle_signal_delivers_payload_to_pending_call` | `handlers.rs` | ✅ PASS |
| `handle_signal_publishes_event_when_no_pending_call` | `handlers.rs` | ✅ PASS |
| `handle_signal_returns_error_without_event_store` | `handlers.rs` | ✅ PASS |
| `handle_signal_injects_event_into_paradigm_state` | `handlers.rs` | ✅ PASS |
| `handle_signal_reply_error_on_publish_failure` | `handlers.rs` | ✅ PASS |
| `wait_for_signal_returns_buffered_immediately` | `procedural.rs` | ✅ PASS |
| `wait_for_signal_registers_pending_when_no_buffer` | `procedural.rs` | ✅ PASS |
| `wait_for_signal_consumes_fifo_from_vec` | `procedural.rs` | ✅ PASS |

### Test & Clippy Results

```
cargo test -p vo-actor → ALL PASS (0 failures)
cargo clippy -p vo-actor → 0 warnings in vo-actor source
  (4 pre-existing clippy errors in vo-common/types/id.rs — missing # Errors docs — unrelated to this bead)
```

## Conclusion

**No code changes were made.** The entire spec for vo-cedw was already implemented by the combination of vo-88f4 (handle_signal publishing, pending_signal_calls, RpcReplyPort wake) and vo-3cv7 (wait_for_signal context method, ProceduralWaitForSignal message, signal buffering, replay handling). All acceptance criteria from spec §12 are met. All tests pass.
