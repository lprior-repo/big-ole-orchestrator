# Red Queen Report — wtf-88f4: "Store signal in InstanceState"

**Date:** 2026-03-23
**Verdict:** 1 BROKE, 6 SURVIVED

---

## Attack 1: Signal with no waiter — BROKE (design gap)

**Path:** `instance/handlers.rs:161-169`

Signal buffering only happens for `ParadigmState::Procedural`. FSM and DAG paradigms silently drop the in-memory buffer:

```rust
if let Some(port) = state.pending_signal_calls.remove(&signal_name) {
    let _ = port.send(Ok(payload));
} else if let ParadigmState::Procedural(s) = &mut state.paradigm_state {
    s.received_signals.entry(signal_name).or_default().push(payload);
}
// FSM/DAG: signal published to JetStream but NOT buffered in-memory
```

**Impact:** FSM/DAG workflows that receive a signal before any handler processes it will not have an in-memory copy. The signal IS persisted in JetStream (survives crash recovery via replay), but the current in-memory paradigm state has no record of it until `apply_event` processes it.

**Severity:** LOW — event sourcing means the signal IS durable; this is not a data loss bug. But the buffering asymmetry between paradigms is a design smell.

---

## Attack 2: Publish failure — SURVIVED

**Path:** `instance/handlers.rs:155-176`

All state mutations (pending_signal_calls.remove, received_signals.insert, inject_event) are gated inside the `Ok(seq)` branch. On publish failure:

- `total_events_applied` is NOT incremented (verified by test `handle_signal_reply_error_on_publish_failure`)
- `pending_signal_calls` entry is NOT removed
- `received_signals` is NOT modified
- Caller receives `Err`

---

## Attack 3: Concurrent signals for same name — SURVIVED

Ractor actors process messages sequentially within a single actor. Two signals for the same name arriving in sequence both get buffered in the `Vec`:

```rust
s.received_signals.entry(signal_name).or_default().push(payload);
```

Multiple waiters consume FIFO via `queue.remove(0)`, verified by test `wait_for_signal_consumes_fifo_from_vec`.

---

## Attack 4: Empty signal name — SURVIVED (at API layer)

`handle_signal` does NOT validate `signal_name`. An empty string "" would be accepted, published, and buffered as a valid HashMap key. No panic, no crash.

The API layer (`wtf-api/types/newtypes.rs`) validates `signal_name` against `[a-z][a-z0-9_]+` before reaching the actor, so "" is blocked at the HTTP boundary.

**Risk:** Any code path that bypasses the API (e.g., internal actor messaging, test mocks) could inject a "" signal name without error.

---

## Attack 5: Large payload — SURVIVED

`Bytes` is reference-counted. Clones in `handle_signal` are atomic refcount bumps, not copies. No unbounded loops or recursive allocations in the signal path. Size limits are enforced by JetStream at the storage layer.

---

## Attack 6: Test isolation — SURVIVED

Ran `cargo test -p wtf-actor --lib -- signal` twice consecutively:

```
Run 1: 13 passed, 0 failed, 0 ignored
Run 2: 13 passed, 0 failed, 0 ignored
```

No shared mutable state between test runs. All state is constructed fresh per test via `make_test_state()`.

---

## Attack 7: Clippy strict — SURVIVED

```
cargo clippy -p wtf-actor -- -W clippy::unwrap_used -W clippy::expect_used
```

- **0 errors**
- **0 unwrap_used warnings** in signal code
- **0 expect_used warnings** in signal code
- `instance/mod.rs` has `#![deny(clippy::unwrap_used)]` and `#![deny(clippy::expect_used)]` at the module level

---

## BONUS: Signal loss in wait_for_signal — BROKE (critical)

**Path:** `instance/procedural.rs:107-118`

When `handle_wait_for_signal` finds a buffered signal, it removes the payload from the queue BEFORE attempting to persist:

```rust
let payload_to_return = queue.remove(0);           // <-- CONSUMED
if queue.is_empty() {
    s.received_signals.remove(&signal_name);        // <-- BUFFER EMPTIED
}
publish_signal_event(state, ...).await;             // <-- PUBLISH MAY FAIL
let _ = reply.send(Ok(payload_to_return));          // <-- REPLY SENT REGARDLESS
```

And `publish_signal_event` silently swallows publish failures:

```rust
if let Ok(seq) = store.publish(...).await {
    let _ = handlers::inject_event(state, seq, &event).await;
}
// On Err: event NOT persisted, NOT injected — signal LOST
```

**Result:** If JetStream publish fails here, the signal is:
1. Removed from `received_signals` buffer (gone from memory)
2. NOT persisted in JetStream (gone from durable log)
3. Reply sent with `Ok(payload)` to the workflow (workflow thinks it succeeded)

The workflow continues as if the signal was processed, but there's no record of it in the event log. After crash recovery, the signal never happened — the workflow will re-execute `wait_for_signal` and wait forever.

**Severity:** HIGH — this is a silent data loss bug that corrupts workflow state after recovery.

---

## Summary

| # | Attack Vector | Verdict | Severity |
|---|--------------|---------|----------|
| 1 | Signal with no waiter (FSM/DAG) | BROKE | LOW |
| 2 | Publish failure in handle_signal | SURVIVED | — |
| 3 | Concurrent signals | SURVIVED | — |
| 4 | Empty signal name | SURVIVED | — |
| 5 | Large payload | SURVIVED | — |
| 6 | Test isolation | SURVIVED | — |
| 7 | Clippy strict | SURVIVED | — |
| BONUS | Signal loss in wait_for_signal | BROKE | HIGH |
