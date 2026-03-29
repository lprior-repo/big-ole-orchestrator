# Implementation Summary: vo-m60g

- **bead_id:** vo-m60g
- **bead_title:** instance: Publish InstanceStarted event
- **phase:** STATE-3
- **updated_at:** 2026-03-23T00:00:00Z

## Files Modified

| File | Lines Changed | Description |
|------|--------------|-------------|
| `crates/vo-actor/src/instance/init.rs` | 142–338 | Added `publish_instance_started` function (lines 142–186) and 3 unit tests with mock infrastructure (lines 188–338) |
| `crates/vo-actor/src/instance/actor.rs` | 51 | Added `init::publish_instance_started(&state.args, &event_log).await?;` call after `spawn_live_subscription`, before `state.phase = InstancePhase::Live` |

## Implementation Details

### `publish_instance_started` (init.rs:142–186)

Pure Data->Calc->Actions pattern:
- **Guard 1 (Calc):** Early return `Ok(())` if `event_log.is_empty()` is false — crash recovery path skips publish.
- **Guard 2 (Action):** Extract `event_store` via `ok_or_else` — fail fast if no store configured.
- **Event construction (Data):** Build `WorkflowEvent::InstanceStarted { instance_id, workflow_type, input }` from args.
- **Publish (Action):** Call `EventStore::publish(&args.namespace, &args.instance_id, event)` with full error propagation via `map_err`.

### Call site (actor.rs:51)

Inserted at the correct position per spec:
- **After** `spawn_live_subscription` (line 48–49)
- **Before** `state.phase = InstancePhase::Live` (line 53)
- Uses `?` to propagate errors — actor startup fails if publish fails.

## Constraint Adherence

| Constraint | Status | Proof |
|-----------|--------|-------|
| Zero `unwrap()` / `expect()` | ✅ | No `unwrap()` or `expect()` in production code. Only used in test assertions (allowed per bifurcation rule). |
| Zero `mut` in core logic | ✅ | Function takes `&InstanceArguments` and `&[WorkflowEvent]` — fully immutable parameters. |
| Make illegal states unrepresentable | ✅ | `event_log.is_empty()` is the authoritative signal; no boolean flag to get out of sync. |
| ADR-015 compliance | ✅ | Publishes through `EventStore::publish` trait method, never direct NATS. |
| Fresh-only guard | ✅ | Non-empty `event_log` → immediate `Ok(())` return, no publish. |
| Error propagation | ✅ | Uses `map_err` + `ok_or_else`, never panics. |

## Tests Written

| Test Name | File | Status | Description |
|-----------|------|--------|-------------|
| `fresh_instance_publishes_started_event` | `init.rs` (mod tests, line 275) | ✅ PASS | Verifies EventStore::publish called once with correct `InstanceStarted { instance_id: "inst-abc", workflow_type: "order_flow", input: b'{"order": 42}' }` |
| `crash_recovery_skips_started_event` | `init.rs` (mod tests, line 303) | ✅ PASS | Non-empty `event_log` → `Ok(())`, publish NOT called |
| `no_event_store_returns_error` | `init.rs` (mod tests, line 322) | ✅ PASS | `event_store = None` → returns `Err` containing "No event store" |

### Test Design

Tests use a `RecordingEventStore` with `Arc<Mutex<Vec<WorkflowEvent>>>` to capture published events across the trait object boundary. This avoids `downcast_ref` (not available on trait objects without `Any`) and keeps the mock infrastructure simple.

## cargo test output

```
running 3 tests
test instance::init::tests::fresh_instance_publishes_started_event ... ok
test instance::init::tests::no_event_store_returns_error ... ok
test instance::init::tests::crash_recovery_skips_started_event ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 73 filtered out
```

Full crate: 76 unit tests + 31 integration tests = **107 tests, 0 failures**.

## cargo clippy output

Zero new warnings introduced. Only pre-existing warnings remain (doc-markdown, missing_errors_doc — consistent with crate baseline). No `unwrap_used`, `expect_used`, or `panic` warnings.

```
0 errors, 0 new warnings from changed code
```
