# Black Hat Review: vo-m60g — "instance: Publish InstanceStarted event"

## Verdict: APPROVED

## Audit Checklist

### 1. Hallucinated APIs — `WorkflowEvent::InstanceStarted`

**PASS.** Verified in `crates/vo-common/src/events/mod.rs:18-22`:

```rust
InstanceStarted {
    instance_id: String,
    workflow_type: String,
    input: Bytes,
}
```

The construction at `init.rs:169-173` matches exactly: `instance_id`, `workflow_type`, `input` — all present, correct types.

### 2. Event Store API — `EventStore::publish` signature

**PASS.** Verified in `crates/vo-common/src/storage.rs:28-33`:

```rust
async fn publish(&self, ns: &NamespaceId, inst: &InstanceId, event: WorkflowEvent) -> Result<u64, VoError>;
```

Call site at `init.rs:175-178`:

```rust
store.publish(&args.namespace, &args.instance_id, event).await.map_err(|e| ActorProcessingErr::from(Box::new(e)))?;
```

Signature matches: `&NamespaceId`, `&InstanceId`, `WorkflowEvent`. Return type `Result<u64, VoError>` — `u64` is discarded (seq not needed here), error is propagated correctly via `?`.

### 3. Silent Failures

**PASS.** No errors are swallowed:

- Missing `event_store` → explicit `Err(ActorProcessingErr::from("No event store..."))` at line 167.
- `publish` failure → `.map_err(|e| ActorProcessingErr::from(Box::new(e)))?` at line 178, propagated up.
- Crash recovery skip → `Ok(())` returned but this is correct behavior (guard documented in docstring at line 160).

### 4. Contract Violations — Fresh-Only Guard

**PASS.** The guard at `init.rs:160-162`:

```rust
if !event_log.is_empty() {
    return Ok(());
}
```

This is correct. `event_log` is populated by `replay_events()` in `actor.rs:35`. If any events exist, the instance is a crash recovery — `InstanceStarted` was already published previously. Skipping is the right behavior.

**One concern noted but not a defect:** The guard uses `event_log.is_empty()` rather than checking for an explicit `InstanceStarted` variant. This is acceptable — if any replayed events exist, the instance is not fresh, and `InstanceStarted` would have been the first event written originally. If somehow `InstanceStarted` was missing from the log (data corruption), the system has bigger problems.

### 5. Dead Code / Unused Imports

**PASS.** All imports in `init.rs` are used:

- `WorkflowEvent` (line 11) — used in function signature at line 158 and variant construction at line 169.
- No other imports are unused in the new function's scope.
- The mock `RecordingEventStore` and `EmptyStream` in tests are all used by the 3 test functions.

### 6. Call Site Correctness — `actor.rs`

**PASS.** At `actor.rs:51`:

```rust
init::publish_instance_started(&state.args, &event_log).await?;
```

Called AFTER `spawn_live_subscription` (line 48) and BEFORE `state.phase = InstancePhase::Live` (line 53), matching the docstring contract ("AFTER spawn_live_subscription and BEFORE phase transitions to Live"). Error propagated via `?` — if publish fails, `pre_start` fails, actor doesn't start.

### 7. Test Coverage

**PASS.** Three tests cover all branches:

1. `fresh_instance_publishes_started_event` — happy path, asserts variant + field values.
2. `crash_recovery_skips_started_event` — non-empty event_log → no publish.
3. `no_event_store_returns_error` — missing store → error with descriptive message.

Mock `EventStore` implementation correctly implements the trait (verified signature matches).

## Summary

No hallucinated APIs. No silent failures. No dead code. No contract violations. Guard logic is sound. Call site ordering is correct. Test coverage is comprehensive. Implementation is clean and idiomatic.
