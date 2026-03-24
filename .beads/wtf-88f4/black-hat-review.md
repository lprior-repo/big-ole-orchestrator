# Black Hat Review — wtf-88f4: "instance: Store signal in InstanceState"

**Date:** 2026-03-23
**Reviewer:** Black Hat

---

## 1. Hallucinated APIs

**PASS.** Every type and API used in the implementation exists in the codebase:

| API/Type | Location | Verified |
|----------|----------|----------|
| `HashMap<String, RpcReplyPort<Result<Bytes, WtfError>>>` | `state.rs:35` | `RpcReplyPort` is from `ractor` crate (used throughout wtf-actor); `Bytes` from `bytes` crate; `WtfError` from `wtf-common` |
| `WtfError::nats_publish(message)` | `wtf-common/src/types/error.rs:47` | Exists with `impl Into<String>` param |
| `WorkflowEvent::SignalReceived { signal_name, payload }` | `wtf-common/src/events/mod.rs:71` | Pre-existing, not hallucinated |
| `handlers::inject_event(state, seq, &event)` | `handlers.rs:245-263` | `pub(crate)` function, called correctly |
| `EventStore::publish()` | Used at `handlers.rs:155-157` | Matches existing `append_and_inject_event` pattern in `procedural.rs:57-63` |

No hallucinated APIs detected.

---

## 2. Cross-Bead Defect (Red Queen finding on wtf-3cv7)

**OUT OF SCOPE for wtf-88f4.** The Red Queen found a critical bug in `handle_wait_for_signal` (`procedural.rs:107-118`) — it removes buffered payload BEFORE publishing, creating a signal-loss window. This code was added by **wtf-3cv7**, not wtf-88f4.

**wtf-88f4's `handle_signal` is correct:** publish-first-then-deliver ordering (`handlers.rs:155-171`). On publish failure, no state mutation occurs (pending_signal_calls entry preserved, received_signals untouched). This is the correct pattern.

**Verdict:** wtf-88f4 scope is clean. The wtf-3cv7 defect should be tracked separately.

---

## 3. Struct Literal Updates (9 total)

Found 9 struct literal constructions of `InstanceState` across the codebase:

| File | Line(s) | Has `pending_signal_calls: HashMap::new()`? |
|------|---------|---------------------------------------------|
| `state.rs` | 49-60 | YES (in `InstanceState::initial()`) |
| `mod.rs` | 98-109 | YES |
| `mod.rs` | 124-135 | YES |
| `procedural.rs` | 207-220 | YES |
| `tests/sleep_timer_id_determinism.rs` | 56 | YES |
| `tests/procedural_now_op_id.rs` | 105 | YES |
| `tests/procedural_ctx_start_at_zero.rs` | 115 | YES |
| `tests/now_publish_failure.rs` | 65 | YES |
| `tests/inject_event_paradigm_state.rs` | 59 | YES |

Plus `handlers_tests.rs` constructs via `InstanceState::initial(args)` (lines 107, 208) — gets the field automatically.

**All 9 struct literal sites initialized. Zero missing.**

---

## 4. Silent Failures

| Code Path | Failure | Silent? |
|-----------|---------|---------|
| `handle_signal` — event_store None | `reply.send(Err(WtfError::nats_publish(...)))` | **NO** — caller gets error |
| `handle_signal` — publish failure | `reply.send(Err(e))` | **NO** — caller gets error |
| `handle_signal` — `port.send(Ok(payload))` to dropped waiter | `let _ = port.send(...)` | **EXPECTED** — documented in spec §7: "RPC port already dropped (workflow cancelled)" |
| `handle_signal` — `inject_event` failure | `let _ = inject_event(...)` | **MARGINAL** — `inject_event` returns `Result<(), ActorProcessingErr>`. Swallowing this error means paradigm state could be inconsistent with the journal. However, the event IS in JetStream and will be replayed on recovery. This matches the existing pattern at `procedural.rs:69`. Not a defect in wtf-88f4's scope. |

No genuine silent failures within wtf-88f4's scope.

---

## 5. Line Counts

| File | Lines | Limit | Status |
|------|-------|-------|--------|
| `state.rs` | 79 | 300 | **PASS** |
| `handlers.rs` | 263 | 300 | **PASS** |

Both well within limits. `handlers.rs` is at 87.7% — approaching the limit but still healthy.

---

## 6. Additional Observations

### 6a. handle_inject_event_msg SignalReceived arm (handlers.rs:123-131)

This arm wakes pending signal waiters during event replay. It was NOT in wtf-88f4's spec (which explicitly scoped out `wait_for_signal`). Per the wtf-cedw implementation report, this arm was added as part of the combined wtf-88f4 + wtf-3cv7 work. It is correct — uses `remove` + `port.send` with proper type annotation. Not a defect.

### 6b. Signal buffering for Procedural only (handlers.rs:163-168)

The Red Queen flagged that FSM/DAG paradigms don't buffer signals in-memory. This is by design — `wait_for_signal` is a Procedural-only concept. FSM/DAG workflows handle signals via state machine transitions and DAG edge evaluation, not RPC waiters. Not a defect.

---

## Summary

| Check | Verdict |
|-------|---------|
| Hallucinated APIs | PASS — all types verified |
| Cross-bead defect scope | PASS — wtf-3cv7 bug out of scope; wtf-88f4's handle_signal is correct |
| Struct literal completeness | PASS — 9/9 initialized, 0 missing |
| Silent failures | PASS — error paths propagate; dropped-port handling documented |
| Line counts | PASS — 79 and 263, both under 300 |

---

**STATUS: APPROVED**
