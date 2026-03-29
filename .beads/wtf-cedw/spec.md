# vo-cedw

## instance: Implement handle_signal wake in instance handlers

### 1. identity

- **bead_id:** vo-cedw
- **title:** instance: Implement handle_signal wake in instance handlers
- **type:** feature
- **priority:** 2
- **effort_estimate:** 1hr
- **status:** planned
- **crates:** vo-actor
- **branch:** main

### 2. problem

`handle_signal` in `crates/vo-actor/src/instance/handlers.rs:116-129` is a stub. When an external signal arrives via `InstanceMsg::InjectSignal`, the handler logs "signal received (stub)" and acks the reply, but does nothing else. Procedural workflows have no way to wait for a signal, and signals are not persisted to the event store. This means signals are silently discarded.

### 3. context

Signals enter the system via `crates/vo-actor/src/master/handlers/signal.rs:8-26`, which calls `actor_ref.cast(InstanceMsg::InjectSignal { ... })`. The `WorkflowEvent::SignalReceived { signal_name: String, payload: Bytes }` variant already exists in `crates/vo-common/src/events/mod.rs:71`.

The existing wake pattern is established by `handle_inject_event_msg` (lines 87-113) which: persists the event, then wakes pending waiters by removing from `state.pending_activity_calls` / `state.pending_timer_calls` and sending the result through the `RpcReplyPort`.

### 4. objectives

1. Persist signals to the event store as `WorkflowEvent::SignalReceived`.
2. Store the signal in `InstanceState` so it can be replayed.
3. Wake any pending `wait_for_signal` caller via `RpcReplyPort`.

### 5. scope

**In scope:**
- Replace `handle_signal` stub with real implementation.
- Add `pending_signal_calls` field to `InstanceState`.
- Add `InstanceMsg::ProceduralWaitForSignal` variant.
- Add `WorkflowContext::wait_for_signal()` method.
- Persist `WorkflowEvent::SignalReceived` to the event store.
- Wake pending signal waiter on signal arrival.
- Handle signal replay via `inject_event`.

**Out of scope:**
- Frontend `WaitForSignal` node editing (already exists in `vo-frontend`).
- Linter rules for signal usage.
- Multiple waiters for the same signal name.

### 6. contracts

#### 6.1. handle_signal must persist event before waking

```rust
// In crates/vo-actor/src/instance/handlers.rs
async fn handle_signal(
    state: &mut InstanceState,
    signal_name: String,
    payload: Bytes,
    reply: RpcReplyPort<Result<(), VoError>>,
) -> Result<(), ActorProcessingErr>;
```

**Preconditions:**
- `state.args.event_store` is `Some` (Live phase).
- Signal has not been persisted yet.

**Postconditions:**
- `WorkflowEvent::SignalReceived { signal_name, payload }` is published to the event store.
- If `state.pending_signal_calls` contains the signal name, the `RpcReplyPort` is sent `Ok(payload)` and removed from the map.
- `reply` is sent `Ok(())`.

**Errors:**
- Event store publish failure: log error, still ack the reply. Recovery will replay from JetStream.

#### 6.2. InstanceState gains pending_signal_calls

```rust
// In crates/vo-actor/src/instance/state.rs
pub pending_signal_calls: HashMap<String, RpcReplyPort<Result<Bytes, VoError>>>,
```

- Key: signal name (exact match).
- Value: one-shot reply port from the procedural workflow.
- Not persisted in snapshots (transient runtime state, like `pending_activity_calls`).

#### 6.3. InstanceMsg::ProceduralWaitForSignal variant

```rust
// In crates/vo-actor/src/messages/instance.rs
ProceduralWaitForSignal {
    signal_name: String,
    reply: RpcReplyPort<Result<Bytes, VoError>>,
},
```

#### 6.4. WorkflowContext::wait_for_signal

```rust
// In crates/vo-actor/src/procedural/context.rs
pub async fn wait_for_signal(&self, signal_name: &str) -> anyhow::Result<Bytes>;
```

Follows the same checkpoint-then-dispatch pattern as `activity()` and `sleep()`:
1. Check for checkpoint at current `op_counter`.
2. If checkpoint exists, increment counter, return stored payload.
3. If not, send `InstanceMsg::ProceduralWaitForSignal`, await reply.

#### 6.5. inject_event wakes signal waiters on replay

In `handle_inject_event_msg` (line 87), add a match arm for `WorkflowEvent::SignalReceived`:
```rust
if let WorkflowEvent::SignalReceived { signal_name, payload } = &event {
    if let Some(port) = state.pending_signal_calls.remove(signal_name) {
        let _ = port.send(Ok::<Bytes, VoError>(payload.clone()));
    }
}
```

### 7. design

#### State changes

**`InstanceState`** (`crates/vo-actor/src/instance/state.rs`):
- Add field: `pub pending_signal_calls: HashMap<String, RpcReplyPort<Result<Bytes, VoError>>>`
- Initialize to `HashMap::new()` in `InstanceState::initial()`.

**`InstanceMsg`** (`crates/vo-actor/src/messages/instance.rs`):
- Add variant `ProceduralWaitForSignal { signal_name: String, reply: RpcReplyPort<Result<Bytes, VoError>> }`.

#### Handler changes

**`handle_signal`** (`crates/vo-actor/src/instance/handlers.rs:116-129`):
- Change `state: &InstanceState` to `state: &mut InstanceState`.
- Publish `WorkflowEvent::SignalReceived { signal_name, payload }` to the event store.
- Remove matching entry from `state.pending_signal_calls`, send payload through the port.
- Ack reply.

**`handle_inject_event_msg`** (`crates/vo-actor/src/instance/handlers.rs:87-113`):
- Add `WorkflowEvent::SignalReceived` wake arm after the `TimerFired` arm.

**`handle_procedural_msg`** (`crates/vo-actor/src/instance/handlers.rs:37-85`):
- Add `InstanceMsg::ProceduralWaitForSignal` arm that inserts into `state.pending_signal_calls` (same pattern as activity dispatch: register waiter, let `handle_inject_event_msg` or `handle_signal` wake it).

**`WorkflowContext::wait_for_signal`** (`crates/vo-actor/src/procedural/context.rs`):
- New async method following the checkpoint-first replay pattern.

#### Event flow

```
External signal → master/handlers/signal.rs → InstanceMsg::InjectSignal
  → handle_signal()
    → publish WorkflowEvent::SignalReceived to event_store
    → if pending_signal_calls[signal_name] exists → send(payload), remove
    → ack reply

Procedural wait_for_signal() → InstanceMsg::ProceduralWaitForSignal
  → handle_procedural_msg: insert into pending_signal_calls
  → workflow task blocks on RpcReplyPort
  → handle_signal or handle_inject_event_msg wakes it
```

### 8. affected_files

| File | Change |
|------|--------|
| `crates/vo-actor/src/instance/state.rs` | Add `pending_signal_calls` field |
| `crates/vo-actor/src/messages/instance.rs` | Add `ProceduralWaitForSignal` variant |
| `crates/vo-actor/src/instance/handlers.rs` | Replace `handle_signal` stub; add wake in `handle_inject_event_msg`; add `ProceduralWaitForSignal` arm in `handle_procedural_msg` |
| `crates/vo-actor/src/procedural/context.rs` | Add `wait_for_signal()` method |

### 9. dependencies

- **crates:** vo-actor (only)
- **external:** None new (uses existing `bytes`, `ractor`, `vo-common`).
- **beads:** None.

### 10. risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Event store publish fails for signal | Low | Signal lost if process crashes before ack | Signal arrives via JetStream replay; reply ack after publish best-effort |
| Signal arrives before waiter registers | Medium | Signal stored in event_store but waiter misses in-memory wake | `inject_event` wakes during replay; procedural workflow will checkpoint-match on next op |
| Duplicate signal delivery | Low | Waiter already consumed | HashMap::remove is idempotent; second remove returns None |

### 11. testing_strategy

#### Unit tests (vo-actor crate)

1. **`handle_signal_persists_event_and_acks`** — Mock event store, verify `SignalReceived` is published, reply is `Ok(())`.
2. **`handle_signal_wakes_pending_waiter`** — Pre-populate `pending_signal_calls`, call `handle_signal`, verify port receives payload and entry is removed.
3. **`handle_signal_no_waiter_stores_nothing`** — Call with empty `pending_signal_calls`, verify no panic, reply is `Ok(())`.
4. **`inject_event_signal_received_wakes_waiter`** — Call `handle_inject_event_msg` with `SignalReceived` event, verify pending waiter is woken.

#### Integration-style tests

5. **`wait_for_signal_checkpoint_replay`** — Verify that on replay, `wait_for_signal` finds a checkpoint and returns without blocking.
6. **`signal_arrives_before_wait_replay_catches`** — Signal event applied first (replay), then `wait_for_signal` call hits checkpoint.

### 12. acceptance_criteria

- [ ] `handle_signal` publishes `WorkflowEvent::SignalReceived` to the event store.
- [ ] `handle_signal` wakes a pending `wait_for_signal` caller if one exists.
- [ ] `handle_inject_event_msg` wakes pending signal waiters during replay.
- [ ] `WorkflowContext::wait_for_signal()` follows the checkpoint-first pattern.
- [ ] `InstanceMsg::ProceduralWaitForSignal` variant exists and is handled.
- [ ] `InstanceState::pending_signal_calls` is initialized to empty `HashMap`.
- [ ] `cargo test --workspace` passes.
- [ ] `cargo clippy --workspace -- -D warnings` passes.

### 13. rollback_plan

Revert all four affected files. The stub `handle_signal` is safe (it just logs). No data migration needed since `pending_signal_calls` is transient runtime state not persisted.

### 14. metrics_and_observability

- Existing `tracing::debug!` in `handle_signal` — update log message from "signal received (stub)" to "signal received, waking waiter" or "signal received, no pending waiter".
- No new metrics required for initial implementation.

### 15. open_questions

None. All types and patterns are established by existing code.

### 16. implementation_order

1. Add `pending_signal_calls` field to `InstanceState` + initialize in `initial()`.
2. Add `ProceduralWaitForSignal` variant to `InstanceMsg`.
3. Replace `handle_signal` stub: persist event, wake waiter, ack reply.
4. Add `SignalReceived` wake arm to `handle_inject_event_msg`.
5. Add `ProceduralWaitForSignal` handling arm in `handle_procedural_msg`.
6. Add `WorkflowContext::wait_for_signal()` method.
7. Write tests.
8. Run `cargo clippy --workspace -- -D warnings` and `cargo test --workspace`.
