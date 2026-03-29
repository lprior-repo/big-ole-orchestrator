# QA Report: vo-88f4 — instance: Store signal in InstanceState

**Date:** 2026-03-23
**Verdict: PASS**

---

## 1. pending_signal_calls field in InstanceState

**PASS** — `state.rs:35` defines:
```rust
pub pending_signal_calls: HashMap<String, RpcReplyPort<Result<Bytes, VoError>>>,
```
Initialized to `HashMap::new()` in `state.rs:57`. Documented as "Not persisted in snapshots."

---

## 2. handle_signal is no longer a stub

**PASS** — `handlers.rs:136-179` contains a full implementation:
- Extracts event_store, returns Err if missing
- Constructs `WorkflowEvent::SignalReceived`
- Publishes via store
- On success: delivers to pending RPC port OR buffers in ProceduralActorState
- Replies `Ok(())` on success, `Err(e)` on failure

---

## 3. unwrap/expect in production code (not tests)

**PASS** — All `unwrap()` and `.expect()` calls are in test files:
- `init_tests.rs` (5 occurrences)
- `handlers_tests.rs` (16 occurrences)
- `procedural.rs` (inside `#[cfg(test)]` block, 9 occurrences)
- `mod.rs` (inside `#[cfg(test)]` block, 3 occurrences)

Zero occurrences in production code.

---

## 4. Signal published BEFORE delivery to waiter (ordering)

**PASS** — `handlers.rs:155-171`:
```rust
match store.publish(...).await {
    Ok(seq) => {
        // THEN deliver to pending RPC port (line 161)
        if let Some(port) = state.pending_signal_calls.remove(&signal_name) { ... }
        // THEN inject_event (line 170)
        let _ = inject_event(state, seq, &event).await;
        let _ = reply.send(Ok(()));  // line 171
    }
    Err(e) => {
        let _ = reply.send(Err(e));  // no state mutation
    }
}
```
Publish is the first side effect. On publish failure, no state mutation occurs — only the reply sends an error. This preserves crash consistency.

**Note:** `inject_event` happens AFTER delivery to the waiter. This means if `inject_event` fails after the RPC port has already received the reply, the waiter has the payload but the paradigm state hasn't been updated. This is a minor ordering concern but not a crash-consistency bug since the event is already in the journal and will be replayed.

---

## 5. Signal-specific test results

```
cargo test -p vo-actor --lib -- signal
running 13 tests
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 110 filtered out
```

13 signal-related tests all pass.

---

## 6. Full crate test results

```
cargo test -p vo-actor --lib
running 123 tests
test result: ok. 123 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

123/123 pass. Zero failures.

---

## 7. Line counts

| File | Lines | Limit | Status |
|------|-------|-------|--------|
| state.rs | 79 | 300 | PASS |
| handlers.rs | 263 | 300 | PASS |

Both well under the 300-line limit.

---

## Summary

| Check | Result |
|-------|--------|
| pending_signal_calls field exists | PASS |
| handle_signal fully implemented | PASS |
| No unwrap/expect in prod code | PASS |
| Publish-before-deliver ordering | PASS |
| Signal tests (13/13) | PASS |
| Full crate tests (123/123) | PASS |
| Line count limits | PASS |

**Overall: PASS**
