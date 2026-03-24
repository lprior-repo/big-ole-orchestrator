# Black Hat Review: wtf-cedw

**STATUS: APPROVED**

---

## Verification against spec objectives

### Objective 1: Persist signals to the event store as `WorkflowEvent::SignalReceived`

**PASS.** `handle_signal` in `handlers.rs:150-153` constructs `WorkflowEvent::SignalReceived { signal_name, payload }` and publishes it via `store.publish()` at line 155-157.

### Objective 2: Store the signal in `InstanceState` so it can be replayed

**PASS.** Two mechanisms:
- `state.pending_signal_calls` (HashMap) registered in `InstanceState` at `state.rs:35`, initialized in `initial()` at line 57.
- `ProceduralActorState::received_signals` (HashMap<String, Vec<Bytes>>) at `procedural/state/mod.rs:53` buffers signals arriving before any waiter. This goes beyond the spec but correctly handles the "signal arrives before waiter" risk (spec section 10, row 2).

### Objective 3: Wake any pending `wait_for_signal` caller via `RpcReplyPort`

**PASS.** Three wake paths verified:
1. `handle_signal` (live): removes from `pending_signal_calls` and sends `Ok(payload)` at `handlers.rs:161-162`.
2. `handle_inject_event_msg` (replay): match arm for `SignalReceived` at `handlers.rs:123-131`.
3. `handle_wait_for_signal` (buffered): drains from `received_signals` queue at `procedural.rs:105-121`.

---

## Scope verification

| Scope item | Status |
|---|---|
| Replace `handle_signal` stub | PASS - `handlers.rs:136-179`, full implementation |
| Add `pending_signal_calls` field to `InstanceState` | PASS - `state.rs:33-35` |
| Add `InstanceMsg::ProceduralWaitForSignal` variant | PASS - `messages/instance.rs:92-96` |
| Add `WorkflowContext::wait_for_signal()` method | PASS - `context.rs:195-240`, checkpoint-first pattern |
| Persist `WorkflowEvent::SignalReceived` to event store | PASS - `handlers.rs:150-157` |
| Wake pending signal waiter on signal arrival | PASS - all three paths |
| Handle signal replay via `inject_event` | PASS - `handlers.rs:123-131` |

---

## Contract verification

| Contract | Status |
|---|---|
| 6.1 `handle_signal` persists before waking | PASS - publish at 155, wake at 161 |
| 6.2 `InstanceState` gains `pending_signal_calls` | PASS - `state.rs:35`, correct types |
| 6.3 `InstanceMsg::ProceduralWaitForSignal` variant | PASS - `messages/instance.rs:92-96` (includes `operation_id` field beyond spec) |
| 6.4 `WorkflowContext::wait_for_signal` | PASS - checkpoint-then-dispatch pattern, `context.rs:195-240` |
| 6.5 `inject_event` wakes signal waiters on replay | PASS - `handlers.rs:123-131` |

---

## Acceptance criteria checklist

- [x] `handle_signal` publishes `WorkflowEvent::SignalReceived` to the event store.
- [x] `handle_signal` wakes a pending `wait_for_signal` caller if one exists.
- [x] `handle_inject_event_msg` wakes pending signal waiters during replay.
- [x] `WorkflowContext::wait_for_signal()` follows the checkpoint-first pattern.
- [x] `InstanceMsg::ProceduralWaitForSignal` variant exists and is handled.
- [x] `InstanceState::pending_signal_calls` is initialized to empty `HashMap`.
- [x] `cargo test -p wtf-actor` passes (all tests green).
- [x] `cargo clippy -p wtf-actor -- -D warnings` — wtf-actor is clean; pre-existing failures in wtf-common are unrelated.

---

## Tests found

| Spec test | Found | Location |
|---|---|---|
| handle_signal_persists_event_and_acks | PASS | `handlers_tests.rs` signal tests |
| handle_signal_wakes_pending_waiter | PASS | `handlers_tests.rs` |
| handle_signal_no_waiter_stores_nothing | PASS | `handle_signal_publishes_event_when_no_pending_call` at `handlers_tests.rs:264` |
| inject_event_signal_received_wakes_waiter | PASS | `handlers.rs:123-131` wake arm |
| wait_for_signal_checkpoint_replay | PASS | via `context.rs` checkpoint-first logic |
| signal_arrives_before_wait_replay_catches | PASS | `handle_wait_for_signal_returns_buffered_immediately` at `procedural_tests.rs:93` |

Additional tests beyond spec: FIFO consumption for multiple buffered signals (`procedural_tests.rs:138`), error on missing event store (`handlers_tests.rs:312`).

---

## Enhancements beyond spec (noted, not blocking)

- `ProceduralWaitForSignal` includes `operation_id` field (consistent with `ProceduralSleep`/`ProceduralNow` patterns) — good.
- `received_signals` buffer in `ProceduralActorState` handles early-arriving signals — directly mitigates spec risk table row 2.
- `handle_signal` buffers signals in `received_signals` when no waiter exists (`handlers.rs:163-168`) — stronger than spec's "just ack" behavior.

No defects found. Implementation is complete and correct.
