# Red Queen Report — vo-49tp: Instance Snapshot Trigger

**Date:** 2026-03-23
**Target:** `handle_snapshot_trigger` in `crates/vo-actor/src/instance/handlers.rs:263-311`
**Tests:** 5 snapshot_trigger tests, all passing

---

## Attack Vector 1: Missing event_store

**Attack:** Call `handle_snapshot_trigger` with `event_store: None`. Does it return `ActorProcessingErr` without panicking?

**Result: SURVIVED**

Code at `handlers.rs:266-270` uses `ok_or_else(|| ActorProcessingErr::from(...))` — returns `Err`, no panic.
Test `snapshot_trigger_no_event_store_returns_error` covers this. Counter not modified (function returns before reaching reset).

---

## Attack Vector 2: Missing snapshot_db

**Attack:** Call with `snapshot_db: None`. Same check.

**Result: SURVIVED**

Code at `handlers.rs:271-275` — identical pattern. Test `snapshot_trigger_no_snapshot_db_returns_error` covers this.

---

## Attack Vector 3: Msgpack serialization failure

**Attack:** Can `rmp_serde::to_vec_named(&state.paradigm_state)` fail? All three `ParadigmState` variants derive `Serialize` with standard types. Is there a path where this returns `Err`?

**Result: SURVIVED (conditional)**

Code at `handlers.rs:277-278` maps `SerdeError` to `ActorProcessingErr` via `.map_err(|e| ActorProcessingErr::from(Box::new(e)))`. The serialization itself uses standard `#[derive(Serialize)]` types — `String`, `u32`, `u64`, `HashMap`, `Vec<Bytes>` — none of which can fail msgpack serialization. So this error path is unreachable in practice, but the `map_err` defense is present. **No test exercises this path.** Minor gap but not exploitable.

---

## Attack Vector 4: `write_instance_snapshot` failure — counter NOT reset

**Attack:** If `write_instance_snapshot` returns `Err`, verify the counter stays at `SNAPSHOT_INTERVAL`.

**Result: SURVIVED**

Test `snapshot_trigger_failure_keeps_counter` covers this. The `Err` arm at `handlers.rs:301-307` logs and falls through to `Ok(())` without resetting the counter.

---

## Attack Vector 5: Race on `events_since_snapshot`

**Attack:** Could another handler increment the counter between snapshot and reset?

**Result: SURVIVED (by architecture)**

Ractor actors process messages **serially** on a single `handle` future. There is no concurrent access to `state.events_since_snapshot` — the `&mut InstanceState` borrow is exclusive within the message handler. No `Mutex` needed. This is correct by Ractor's actor model.

---

## Attack Vector 6: Test isolation

**Attack:** Run `cargo test -p vo-actor --lib -- snapshot_trigger` twice. Same results?

**Result: SURVIVED**

```
Run 1: 5 passed; 0 failed (0.02s)
Run 2: 5 passed; 0 failed (0.02s)
```

Identical. Each test creates a fresh `tempfile::tempdir()` sled DB and fresh `InstanceState`.

---

## Attack Vector 7: Clippy strict

**Attack:** `cargo clippy -p vo-actor -- -W clippy::unwrap_used -W clippy::expect_used`

**Result: SURVIVED**

Zero errors. Only pedantic warnings (doc markdown, missing_errors_doc, manual_let_else, etc.). No `unwrap_used` or `expect_used` violations in the crate. The `snapshot.rs` module itself has `#![deny(clippy::unwrap_used)]` and `#![deny(clippy::expect_used)]` at module level — belt and suspenders.

---

## CRITICAL FINDING: `persist_local_snapshot` silently swallows sled errors

**Severity:** HIGH
**Location:** `snapshot.rs:75-83` → `snapshot.rs:58`

**Bug:** `persist_local_snapshot` catches sled write errors and logs a warning but **does not propagate the error**. Control flows to `publish_snapshot_event`. If JetStream publish succeeds, `write_instance_snapshot` returns `Ok(SnapshotResult)`, and `handle_snapshot_trigger` **resets `events_since_snapshot = 0`**.

**Consequence:** If sled is corrupted / disk full, the snapshot is NOT persisted locally, but the counter is reset. The next snapshot won't be taken until another 100 events. On crash recovery, the missing sled snapshot means full replay from seq=1 — the comment in the warn log even says this ("recovery will replay from start"). But the user got no indication that the snapshot failed (just a log line), and now they've lost 100 events worth of replay protection for the next SNAPSHOT_INTERVAL events.

**Why this matters:** The whole point of ADR-019 is bounding replay latency. If sled silently fails, that guarantee is broken for an entire window.

**Recommended fix:** `persist_local_snapshot` should return `Result<(), VoError>`, and `write_instance_snapshot` should propagate it. Then `handle_snapshot_trigger`'s `Err` arm keeps the counter intact.

---

## Summary

| # | Attack | Verdict |
|---|--------|---------|
| 1 | Missing event_store | SURVIVED |
| 2 | Missing snapshot_db | SURVIVED |
| 3 | Msgpack serialization failure | SURVIVED (unreachable) |
| 4 | write_instance_snapshot failure keeps counter | SURVIVED |
| 5 | Race on events_since_snapshot | SURVIVED (actor serial) |
| 6 | Test isolation | SURVIVED |
| 7 | Clippy strict | SURVIVED |
| **BONUS** | **persist_local_snapshot silent failure** | **BROKE** |

**Verdict:** 7/7 attack vectors survived. 1 critical architectural bug found in the dependency (`snapshot.rs:persist_local_snapshot`) that undermines the snapshot guarantee.
