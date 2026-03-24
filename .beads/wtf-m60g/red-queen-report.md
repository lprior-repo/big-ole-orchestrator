# Red Queen Report — wtf-m60g: "instance: Publish InstanceStarted event"

**Verdict: 6/6 SURVIVED**

---

## AV1: Empty event_log (crash recovery guard)

**Attack:** Call `publish_instance_started` with `event_log = []`. Does it publish? Should.

**Command:** `cargo test -p wtf-actor --lib -- fresh_instance_publishes_started_event`

**Result: SURVIVED** — Test passes. Function publishes `WorkflowEvent::InstanceStarted` with correct fields (`instance_id`, `workflow_type`, `input`). Published event count == 1.

---

## AV2: Missing event_store (Err not panic)

**Attack:** Call `publish_instance_started` with `args.event_store = None` and empty `event_log`. Should return `Err` without panicking.

**Command:** `cargo test -p wtf-actor --lib -- no_event_store_returns_error`

**Result: SURVIVED** — Returns `Err` with message containing "No event store". No panic path exists. Uses `ok_or_else()` at `init.rs:164-167`.

---

## AV3: Call site ordering

**Attack:** Verify `publish_instance_started` is called AFTER `spawn_live_subscription` but BEFORE `state.phase = InstancePhase::Live` in `pre_start`.

**Code read:** `actor.rs:34-63`

```
L34: load_initial_state
L35: replay_events
L37-45: transition_to_live
L47-49: spawn_live_subscription
L51:   init::publish_instance_started  ← CORRECT position
L53:   state.phase = InstancePhase::Live
```

**Result: SURVIVED** — Ordering is correct. InstanceStarted is published after the live subscription is spawned (so the event will be caught by it), and before the phase transitions to Live.

---

## AV4: Double publish

**Attack:** Is there any path where InstanceStarted could be published twice for the same instance?

**Analysis:**
- `publish_instance_started` has exactly ONE production call site: `actor.rs:51`
- `pre_start` is called exactly once per actor by the ractor framework
- The `event_log.is_empty()` guard (`init.rs:160`) ensures idempotency even if called during replay
- `InjectEvent` handler (`handlers.rs`) does NOT call `publish_instance_started`
- No other code in `crates/wtf-actor/src/` constructs or publishes `WorkflowEvent::InstanceStarted`

**Result: SURVIVED** — No double-publish path exists. Single call site + fresh-instance guard makes it safe.

---

## AV5: Test isolation

**Attack:** Run the 3 publish_instance_started tests twice, verify identical results.

**Commands:**
```
cargo test -p wtf-actor --lib -- fresh_instance_publishes crash_recovery_skips no_event_store
cargo test -p wtf-actor --lib -- fresh_instance_publishes crash_recovery_skips no_event_store
```

**Result: SURVIVED** — Both runs: 4 passed, 0 failed. Tests are deterministic (no shared mutable state, fresh `PublishedCapture` per test).

---

## AV6: Clippy strict (unwrap_used, expect_used)

**Attack:** `cargo clippy -p wtf-actor -- -W clippy::unwrap_used -W clippy::expect_used`

**Result: SURVIVED** — 0 `unwrap_used` or `expect_used` warnings in production code. The production function (`init.rs:156-186`) uses `ok_or_else` and `map_err` exclusively — no `.unwrap()` or `.expect()`.

Note: Test code at `init.rs:228,284,312` uses `.expect("lock")` on Mutex, but clippy does not lint test code when run without `--tests` flag, and these are acceptable in test assertions.

---

## Summary

| # | Attack Vector | Result |
|---|---|---|
| 1 | Empty event_log (fresh publish) | **SURVIVED** |
| 2 | Missing event_store (Err path) | **SURVIVED** |
| 3 | Call site ordering | **SURVIVED** |
| 4 | Double publish prevention | **SURVIVED** |
| 5 | Test isolation | **SURVIVED** |
| 6 | Clippy strict mode | **SURVIVED** |

**The throne holds.** Implementation is solid across all adversarial vectors.
