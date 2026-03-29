# QA Report: vo-49tp — instance: Implement snapshot trigger

## Checklist

### 1. handle_snapshot_trigger is no longer a stub
**PASS**
Lines 263-311 of `handlers.rs` contain a full implementation. The function:
- Extracts `event_store` and `snapshot_db` from state
- Serializes `paradigm_state` via msgpack
- Delegates to `write_instance_snapshot`
- Handles success/failure paths

### 2. Msgpack serialization call exists
**PASS**
Line 277: `rmp_serde::to_vec_named(&state.paradigm_state)` with error mapped to `ActorProcessingErr`.

### 3. write_instance_snapshot is called
**PASS**
Lines 281-288: `crate::snapshot::write_instance_snapshot(...)` called with all required arguments.

### 4. events_since_snapshot reset ONLY on success
**PASS**
Line 299 (`state.events_since_snapshot = 0`) is inside the `Ok(result)` match arm only.
The `Err(e)` arm (lines 301-306) logs a warning and does NOT touch the counter.

### 5. No unwrap/expect in production handler
**PASS**
Grep for `unwrap()` / `.expect(` in lines 1-312 of handlers.rs returned zero matches.
All `expect()` calls in the file are inside `#[cfg(test)]` (lines 313+).

### 6. Snapshot trigger tests
**PASS**
```
running 5 tests
test instance::handlers::tests::snapshot_trigger_no_snapshot_db_returns_error ... ok
test instance::handlers::tests::snapshot_trigger_no_event_store_returns_error ... ok
test instance::handlers::tests::snapshot_trigger_success_resets_counter ... ok
test instance::handlers::tests::snapshot_trigger_failure_keeps_counter ... ok
test instance::handlers::tests::snapshot_trigger_preserves_paradigm_state ... ok
test result: ok. 5 passed; 0 failed
```

### 7. Full vo-actor lib tests
**PASS**
```
running 123 tests
test result: ok. 123 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### 8. Line count under 300
**FAIL**
`handlers.rs` is 731 lines — well over the 300 line limit.

---

## Verdict: **FAIL**

The implementation is correct and well-tested, but `handlers.rs` (731 lines) violates the <300 line file constraint. This is a pre-existing issue (the file was already large before this bead), but it fails the architectural gate.
