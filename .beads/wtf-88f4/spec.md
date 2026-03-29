---
bead_id: vo-88f4
title: "instance: Store signal in InstanceState"
effort_estimate: "30min"
status: draft
type: task
priority: 2
---

# Section 0: Clarifications

- **Scope**: Add a `pending_signal_calls` field to `InstanceState` and wire it into the signal handler. This bead does NOT add `WorkflowContext::wait_for_signal()` — that is a separate bead.
- **Pattern**: Follows the exact same pattern as `pending_activity_calls` (`HashMap<ActivityId, RpcReplyPort<...>>`) and `pending_timer_calls` (`HashMap<TimerId, RpcReplyPort<...>>`).
- **Durability**: Signals must produce a `WorkflowEvent::SignalReceived` in JetStream before delivering to the waiting RPC port — same as activities dispatch `ActivityDispatched` then wait for `ActivityCompleted`.
- **Key type**: `String` (signal name) is the natural key. `WorkflowEvent::SignalReceived { signal_name: String, payload: Bytes }` already exists in `vo_common::WorkflowEvent`.

---

# Section 1: EARS (Requirements)

**WHEN** a signal is received via `InstanceMsg::InjectSignal { signal_name, payload, reply }`
**AND** a `wait_for_signal` RPC port is registered under that signal name
**THEN** the system SHALL publish a `WorkflowEvent::SignalReceived { signal_name, payload }` to JetStream, inject the event into paradigm state, and send the payload to the registered RPC port.

**WHILE** no RPC port is registered for a given signal name
**THEN** the system SHALL publish the `WorkflowEvent::SignalReceived` to JetStream (for durability) and reply `Ok(())` to the caller.

**IF** the event store is unavailable when `InjectSignal` arrives
**THEN** the system SHALL reply with `Err(VoError::nats_publish(...))` and NOT modify instance state.

---

# Section 2: KIRK Contracts

## Contract: `InstanceState::pending_signal_calls`

```rust
/// Pending RPC calls from procedural workflows waiting for signals.
/// Keyed by signal name (String). Not persisted in snapshots.
pub pending_signal_calls: HashMap<String, RpcReplyPort<Result<Bytes, VoError>>>,
```

**Invariants:**
- I1: A signal name maps to at most one pending RPC port (single waiter per signal name).
- I2: Entries are removed on signal delivery or actor stop — never leaked.
- I3: `pending_signal_calls` is NOT serialized in sled snapshots (like `pending_activity_calls` and `pending_timer_calls`).

## Contract: `handle_signal` (updated)

```rust
async fn handle_signal(
    state: &mut InstanceState,   // NOTE: &mut, not &InstanceState
    signal_name: String,
    payload: Bytes,               // NOTE: no longer _payload
    reply: RpcReplyPort<Result<(), VoError>>,
) -> Result<(), ActorProcessingErr>
```

**Preconditions:**
- `state.args.event_store` is `Some` for the publish path.
- Signal name is non-empty and matches `[a-z][a-z0-9_]+` (validated upstream by API).

**Postconditions:**
- If event store publish succeeds: `WorkflowEvent::SignalReceived` is in JetStream and `inject_event` has been called with the returned seq.
- If a pending RPC port exists for `signal_name`: it receives `Ok(payload)` and is removed from the map.
- `reply` always receives `Ok(())` on success or `Err(VoError)` on publish failure.

---

# Section 2.5: Research

**Existing pattern — `pending_activity_calls` delivery chain:**

1. `procedural.rs:handle_dispatch` → publishes `ActivityDispatched` to JetStream → inserts RPC port into `state.pending_activity_calls[activity_id]` → calls `inject_event`.
2. `handlers.rs:handle_inject_event_msg` → on `ActivityCompleted` → `state.pending_activity_calls.remove(&aid)` → `port.send(Ok(result))`.

**Proposed signal delivery chain (mirrors activity pattern):**

1. `handlers.rs:handle_signal` → publishes `SignalReceived` to JetStream → `state.pending_signal_calls.remove(&signal_name)` → `port.send(Ok(payload))`.
2. Registration of the RPC port (by a future `wait_for_signal` call) inserts into `pending_signal_calls[signal_name]`.

**Key difference**: Activity uses `ActivityId` key (two-phase: dispatch then complete). Signal uses `String` signal name key (single-phase: publish and deliver immediately).

**Relevant files:**
- `crates/vo-actor/src/instance/state.rs:12-38` — `InstanceState` struct
- `crates/vo-actor/src/instance/handlers.rs:116-129` — current `handle_signal` stub
- `crates/vo-actor/src/instance/procedural.rs:24-75` — `handle_dispatch` as reference pattern
- `crates/vo-actor/src/messages/instance.rs:59-63` — `InstanceMsg::InjectSignal`
- `crates/vo-common/src/events/mod.rs:71` — `WorkflowEvent::SignalReceived`

---

# Section 3: Inversions

| # | Dependency | Strategy |
|---|-----------|----------|
| 1 | NATS JetStream event store | Accept `Option<Arc<dyn EventStore>>` in state; `handle_signal` returns error if `None` |
| 2 | `WorkflowContext::wait_for_signal()` (not yet implemented) | This bead only adds the storage field and wires the handler. Future bead adds the context method that inserts into `pending_signal_calls`. |
| 3 | `inject_event` side effects on `ParadigmState` | Call existing `handlers::inject_event(state, seq, &event)` after publish succeeds |

---

# Section 4: ATDD Tests (Unit)

### T1: `InstanceState::initial` includes empty `pending_signal_calls`

```rust
#[test]
fn initial_state_has_empty_pending_signal_calls() {
    let args = InstanceArguments { /* ... full construction ... */ };
    let state = InstanceState::initial(args);
    assert!(state.pending_signal_calls.is_empty());
}
```

### T2: `handle_signal` publishes event and delivers to pending RPC port

```rust
#[tokio::test]
async fn handle_signal_delivers_payload_to_pending_call() {
    // Setup: InstanceState with mock event_store, insert RpcReplyPort into pending_signal_calls["order_approved"]
    // Action: handle_signal(state, "order_approved", payload, caller_reply)
    // Assert:
    //   - event_store.publish called with WorkflowEvent::SignalReceived { signal_name: "order_approved", payload }
    //   - The pending RPC port receives Ok(payload)
    //   - pending_signal_calls no longer contains "order_approved"
    //   - caller_reply receives Ok(())
}
```

### T3: `handle_signal` with no pending call still publishes event

```rust
#[tokio::test]
async fn handle_signal_publishes_event_when_no_pending_call() {
    // Setup: InstanceState with mock event_store, no pending_signal_calls
    // Action: handle_signal(state, "timeout", payload, caller_reply)
    // Assert:
    //   - event_store.publish called
    //   - caller_reply receives Ok(())
    //   - total_events_applied incremented
}
```

### T4: `handle_signal` returns error when event_store is None

```rust
#[tokio::test]
async fn handle_signal_returns_error_without_event_store() {
    // Setup: InstanceState with event_store: None
    // Action: handle_signal(state, "sig", payload, reply)
    // Assert: reply receives Err containing "Event store missing"
}
```

### T5: SignalReceived event is injected into paradigm state

```rust
#[tokio::test]
async fn handle_signal_injects_event_into_paradigm_state() {
    // Setup: Procedural paradigm state
    // Action: handle_signal on a procedural instance
    // Assert: paradigm_state.applied_seq contains the returned seq number
}
```

---

# Section 5: E2E Tests (Integration)

### E1: Signal round-trip through instance actor (requires NATS)

```rust
#[tokio::test]
async fn signal_round_trip_through_instance_actor() {
    // 1. Spawn WorkflowInstance with procedural paradigm and mock EventStore
    // 2. Send InstanceMsg::InjectSignal { signal_name: "approve", payload, reply }
    // 3. Assert reply is Ok(())
    // 4. Assert JetStream contains SignalReceived event
}
```

---

# Section 5.5: Verification Gates

```bash
cargo test -p vo-actor -- pending_signal
cargo clippy -p vo-actor -- -D warnings
cargo check -p vo-actor
```

---

# Section 6: Implementation Tasks

1. **Add `pending_signal_calls` field to `InstanceState`** in `crates/vo-actor/src/instance/state.rs`
   - Type: `HashMap<String, RpcReplyPort<Result<Bytes, VoError>>>`
   - Initialize to `HashMap::new()` in `InstanceState::initial`
   - Update existing test struct literals in `procedural.rs` tests to include the new field

2. **Rewrite `handle_signal`** in `crates/vo-actor/src/instance/handlers.rs:116-129`
   - Change `state: &InstanceState` to `state: &mut InstanceState`
   - Guard: if `state.args.event_store.is_none()`, send `Err(VoError::nats_publish("Event store missing"))` and return
   - Publish `WorkflowEvent::SignalReceived { signal_name: signal_name.clone(), payload: payload.clone() }` via `state.args.event_store`
   - On publish success: remove pending call from `state.pending_signal_calls.remove(&signal_name)`, send `Ok(payload)` to that port if present
   - Call `handlers::inject_event(state, seq, &event)` to update paradigm state
   - Send `Ok(())` to the caller's `reply` port

3. **Update struct literals** in test code that constructs `InstanceState` directly (2 tests in `procedural.rs:155-167` and `210-219`) to include `pending_signal_calls: HashMap::new()`

---

# Section 7: Failure Modes

| Failure | Detection | Mitigation |
|---------|-----------|------------|
| Event store publish fails | `store.publish()` returns `Err` | Reply `Err(VoError)` to caller, do not modify state |
| RPC port already dropped (workflow cancelled) | `port.send()` returns `Err` | Ignore — log at trace, this is expected during cancellation |
| Signal name collision (two waiters same name) | N/A — HashMap insert overwrites | Document invariant I1: single waiter per signal name |
| Actor stops with pending signal calls | `post_stop` in `actor.rs` | RPC ports are dropped on actor stop; no explicit cleanup needed |

---

# Section 7.5: Anti-Hallucination

- `WorkflowEvent::SignalReceived` already exists at `vo-common/src/events/mod.rs:71` — do NOT create it.
- `InstanceMsg::InjectSignal` already exists at `vo-actor/src/messages/instance.rs:59-63` — do NOT modify the message type.
- `VoError::nats_publish()` already exists — use it for error construction.
- `handlers::inject_event` is a `pub(crate)` function at `handlers.rs:195-213` — call it directly, do NOT reimplement.
- The `handle_signal` function signature change (`&InstanceState` → `&mut InstanceState`) requires no change to the call site in `handle_msg` at `handlers.rs:21` because `state` is already `&mut`.

---

# Section 7.6: Context Survival

If the LLM context is lost, the following files contain the complete picture:
- `crates/vo-actor/src/instance/state.rs` — the struct to modify
- `crates/vo-actor/src/instance/handlers.rs:116-129` — the stub to replace
- `crates/vo-actor/src/instance/procedural.rs:24-75` — reference pattern (`handle_dispatch`)
- `crates/vo-common/src/events/mod.rs:71` — `WorkflowEvent::SignalReceived { signal_name: String, payload: Bytes }`
- `crates/vo-actor/src/messages/instance.rs:59-63` — `InstanceMsg::InjectSignal { signal_name, payload, reply }`

---

# Section 8: Completion Criteria

- [ ] `InstanceState` has `pending_signal_calls: HashMap<String, RpcReplyPort<Result<Bytes, VoError>>>`
- [ ] `InstanceState::initial` initializes it to empty
- [ ] `handle_signal` publishes `WorkflowEvent::SignalReceived` and delivers to pending RPC port
- [ ] `handle_signal` returns error when event store is missing
- [ ] All existing struct literal constructions of `InstanceState` compile
- [ ] `cargo clippy -p vo-actor -- -D warnings` passes
- [ ] `cargo test -p vo-actor` passes

---

# Section 9: Context

This bead is part of the procedural workflow signal delivery feature. The current `handle_signal` at `handlers.rs:116-129` is a stub that logs "signal received (stub)" and immediately replies `Ok(())`. This bead replaces the stub with real signal delivery logic that:
1. Persists the signal as a `WorkflowEvent::SignalReceived` in JetStream (durability)
2. Injects the event into paradigm state (replay correctness)
3. Delivers the payload to any waiting `wait_for_signal` RPC port (functionality)

The `wait_for_signal` method on `WorkflowContext` (registration of RPC ports into `pending_signal_calls`) is a separate future bead.

---

# Section 10: AI Hints

- Follow the `handle_dispatch` → `append_and_inject_event` pattern in `procedural.rs:24-75` as a template, but simplified (no two-phase dispatch/complete).
- The `handle_signal` call site at `handlers.rs:21` passes `state` as `&mut` already, so changing the function signature from `&InstanceState` to `&mut InstanceState` requires zero call-site changes.
- When constructing `WorkflowEvent::SignalReceived`, use `signal_name.clone()` and `payload.clone()` because both are moved into the publish call and may be needed for the RPC reply.
- The `inject_event` function at `handlers.rs:195` is `pub(crate)` and in the same module — call it directly.
