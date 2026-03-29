# Implementation Summary

## Metadata
- **bead_id**: vo-88f4
- **bead_title**: instance: Store signal in InstanceState
- **phase**: STATE-3
- **updated_at**: 2026-03-23T12:00:00Z

## Changes Made

### 1. `crates/vo-actor/src/instance/state.rs` (lines 31-33, 52)
- Added `pending_signal_calls: HashMap<String, RpcReplyPort<Result<Bytes, VoError>>>` field to `InstanceState`
- Initialized to `HashMap::new()` in `InstanceState::initial()`

### 2. `crates/vo-actor/src/instance/handlers.rs` (lines 116-149)
- Replaced `handle_signal` stub with full implementation
- Changed signature from `state: &InstanceState` to `state: &mut InstanceState`
- Logic: guard on event_store → publish SignalReceived → deliver to pending RPC port → inject_event → reply Ok(())
- Error paths: event_store None → reply Err; publish failure → reply Err, no state mutation

### 3. `crates/vo-actor/src/instance/handlers.rs` (tests module, lines 465-614)
Added 6 new tests:

| Test Name | Status | Coverage |
|-----------|--------|----------|
| `initial_state_has_empty_pending_signal_calls` | PASS | T1: empty map after initial() |
| `handle_signal_delivers_payload_to_pending_call` | PASS | T2: publish + deliver + remove entry + inject_event |
| `handle_signal_publishes_event_when_no_pending_call` | PASS | T3: publish + reply Ok when no waiter |
| `handle_signal_returns_error_without_event_store` | PASS | T4: reply Err when store is None |
| `handle_signal_injects_event_into_paradigm_state` | PASS | T5: total_events_applied + events_since_snapshot incremented |
| `handle_signal_reply_error_on_publish_failure` | PASS | Extra: reply Err, no state mutation, pending entry preserved |

### 4. Struct literal updates (added `pending_signal_calls: HashMap::new()`)
- `crates/vo-actor/src/instance/procedural.rs` — 2 struct literals (lines 155, 210)
- `crates/vo-actor/src/instance/mod.rs` — 2 struct literals (lines 94, 119)
- `crates/vo-actor/tests/sleep_timer_id_determinism.rs` — 1 struct literal (line 56)
- `crates/vo-actor/tests/procedural_now_op_id.rs` — 1 struct literal (line 105)
- `crates/vo-actor/tests/procedural_ctx_start_at_zero.rs` — 1 struct literal (line 115)
- `crates/vo-actor/tests/now_publish_failure.rs` — 1 struct literal (line 65)
- `crates/vo-actor/tests/inject_event_paradigm_state.rs` — 1 struct literal (line 59)

## Contract Compliance

| Spec Requirement | Status | Notes |
|-----------------|--------|-------|
| I1: Single waiter per signal name | Enforced | HashMap semantics — insert overwrites |
| I2: Entries removed on delivery | Verified | `remove(&signal_name)` in handle_signal |
| I3: Not serialized in snapshots | Automatic | Only `paradigm_state` is serialized in snapshot logic |
| Publish before deliver | Implemented | Event store publish is first; RPC delivery second |
| Error on missing event_store | Implemented | Early return with Err reply |
| inject_event called after publish | Implemented | Uses existing `handlers::inject_event()` |
| No state mutation on failure | Verified | Publish failure path skips remove, inject_event, and reply Ok |

## Constraint Adherence

- **Data->Calc->Actions**: `handle_signal` is in the Actions layer (I/O via event_store.publish, state mutation)
- **Zero unwrap/expect**: All Result handling uses `match`, `if let`, or `let _ = port.send()`
- **No new mut in core logic**: `&mut InstanceState` is required by the Actions handler pattern (same as `handle_dispatch`)
- **Make illegal states unrepresentable**: Signal name as String key; HashMap enforces single-waiter invariant
- **Expression-based**: `handle_signal` uses match/let-else pattern, no imperative loops

## cargo test output

```
test instance::handlers::tests::initial_state_has_empty_pending_signal_calls ... ok
test instance::handlers::tests::handle_signal_delivers_payload_to_pending_call ... ok
test instance::handlers::tests::handle_signal_publishes_event_when_no_pending_call ... ok
test instance::handlers::tests::handle_signal_returns_error_without_event_store ... ok
test instance::handlers::tests::handle_signal_injects_event_into_paradigm_state ... ok
test instance::handlers::tests::handle_signal_reply_error_on_publish_failure ... ok
test result: ok. 82 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## cargo clippy output

No warnings from changed files. Pre-existing warnings in `vo-common` (clippy::missing_errors_doc) are unrelated to this bead.

## cargo check output

```
Checking vo-actor v0.1.0
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.30s
```
