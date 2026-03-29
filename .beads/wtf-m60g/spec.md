# vo-m60g — instance: Publish InstanceStarted event

**Bead ID:** vo-m60g
**Status:** ready
**Priority:** 1
**Type:** feature
**Effort:** 15min

---

## 1. Objective

Publish a `WorkflowEvent::InstanceStarted` event to NATS JetStream during instance initialization, so that every workflow instance has a durable first event in the journal before any transitions occur.

## 2. Context

Currently, `WorkflowInstance::pre_start` (in `crates/vo-actor/src/instance/actor.rs:21-62`) performs replay, transitions to live, spawns subscriptions, and sets the phase to `Live` — but never writes an `InstanceStarted` event. The event variant already exists in `crates/vo-common/src/events/mod.rs:18-22`:

```rust
InstanceStarted {
    instance_id: String,
    workflow_type: String,
    input: Bytes,
}
```

The `EventStore::publish` method (in `crates/vo-common/src/storage.rs:28-33`) provides the write path:

```rust
async fn publish(
    &self,
    ns: &NamespaceId,
    inst: &InstanceId,
    event: WorkflowEvent,
) -> Result<u64, VoError>;
```

## 3. Scope

### In Scope
- Write one `WorkflowEvent::InstanceStarted` event via `EventStore::publish` after `spawn_live_subscription` succeeds in `pre_start`
- Only publish on fresh instances (not on crash recovery replay)
- Unit test verifying the event is constructed correctly

### Out of Scope
- Changing `WorkflowEvent::InstanceStarted` variant shape
- Adding fields like `namespace` or `paradigm` to the variant
- Modifying the `EventStore` trait or `append_event` function
- Changing replay logic or snapshot handling

## 4. Contract

```rust
// In crates/vo-actor/src/instance/init.rs

/// Publish the InstanceStarted event for a fresh (non-replayed) instance.
/// Must be called AFTER spawn_live_subscription and BEFORE phase transitions to Live.
///
/// # Arguments
/// * `args` - The InstanceArguments containing namespace, instance_id, workflow_type, input
///
/// # Returns
/// * `Ok(seq)` - The JetStream sequence number of the published event
/// * `Err(ActorProcessingErr)` - If no event_store is configured or publish fails
///
/// # Guards
/// - Returns Ok(()) immediately if event_log is non-empty (crash recovery path)
/// - Returns Err if args.event_store is None
pub async fn publish_instance_started(
    args: &InstanceArguments,
    event_log: &[WorkflowEvent],
) -> Result<(), ActorProcessingErr>
```

## 5. Invariants

1. **Fresh-only guard**: `InstanceStarted` is published IFF `event_log.is_empty()` (empty replay means fresh instance)
2. **Ordering**: `InstanceStarted` is published after `spawn_live_subscription` and before `state.phase = InstancePhase::Live`
3. **ADR-015 compliance**: Publish goes through `EventStore::publish` (which calls `append_event`), never direct `jetstream.publish()`
4. **No side effects before ack**: The caller awaits the publish result before proceeding

## 6. Affected Files

| File | Change |
|------|--------|
| `crates/vo-actor/src/instance/init.rs` | Add `publish_instance_started` function |
| `crates/vo-actor/src/instance/actor.rs` | Call `publish_instance_started` in `pre_start` |
| `crates/vo-actor/src/instance/mod.rs` | Export `publish_instance_started` if needed |

## 7. Dependencies

- `vo-common::WorkflowEvent::InstanceStarted` — already exists
- `vo-common::storage::EventStore::publish` — already exists
- `crate::messages::InstanceArguments` — provides `namespace`, `instance_id`, `workflow_type`, `input`, `event_store`

## 8. Risks

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| Publishing on crash recovery creates duplicate InstanceStarted | Medium | Guard with `event_log.is_empty()` — non-empty means replay found events |
| EventStore not configured (`None`) | Low | Return `Err(ActorProcessingErr)` — fail fast during init |
| Publish timeout blocks actor startup | Low | `append_event` already has 5s timeout (PUBLISH_ACK_TIMEOUT) |

## 9. Given-When-Then

### Test 1: Fresh instance publishes InstanceStarted
```gherkin
Given an InstanceArguments with event_store = Some(store)
  And event_log is empty (no prior events replayed)
When publish_instance_started(&args, &event_log) is called
Then EventStore::publish is called exactly once
  With WorkflowEvent::InstanceStarted { instance_id, workflow_type, input }
  And the namespace and instance_id match args.namespace and args.instance_id
```

### Test 2: Crash recovery skips InstanceStarted
```gherkin
Given an InstanceArguments with event_store = Some(store)
  And event_log contains previously replayed events (non-empty)
When publish_instance_started(&args, &event_log) is called
Then EventStore::publish is NOT called
  And Ok(()) is returned
```

### Test 3: No event_store returns error
```gherkin
Given an InstanceArguments with event_store = None
  And event_log is empty
When publish_instance_started(&args, &event_log) is called
Then an ActorProcessingErr is returned containing "No event store"
```

## 10. Data Flow

```
pre_start (actor.rs:21)
  │
  ├─ load_initial_state (init.rs:13)
  ├─ replay_events (init.rs:30) → event_log: Vec<WorkflowEvent>
  ├─ transition_to_live (init.rs:77)
  ├─ spawn_live_subscription (init.rs:94)
  │
  ├─ [NEW] publish_instance_started(&state.args, &event_log)
  │    ├─ guard: if !event_log.is_empty() → return Ok(())
  │    ├─ guard: if args.event_store.is_none() → return Err
  │    └─ store.publish(ns, inst, WorkflowEvent::InstanceStarted { ... })
  │
  └─ state.phase = InstancePhase::Live
```

## 11. Acceptance Criteria

- [ ] `publish_instance_started` function added to `crates/vo-actor/src/instance/init.rs`
- [ ] Called in `pre_start` after `spawn_live_subscription` (line 48 of actor.rs)
- [ ] Publishes `WorkflowEvent::InstanceStarted { instance_id, workflow_type, input }`
- [ ] Skipped on crash recovery (non-empty `event_log`)
- [ ] Returns `Err` if `event_store` is `None`
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace -- -D warnings` passes

## 12. Implementation Sketch

```rust
// Add to crates/vo-actor/src/instance/init.rs

pub async fn publish_instance_started(
    args: &InstanceArguments,
    event_log: &[WorkflowEvent],
) -> Result<(), ActorProcessingErr> {
    if !event_log.is_empty() {
        return Ok(());
    }

    let store = args
        .event_store
        .as_ref()
        .ok_or_else(|| ActorProcessingErr::from("No event store available for InstanceStarted publish"))?;

    let event = WorkflowEvent::InstanceStarted {
        instance_id: args.instance_id.to_string(),
        workflow_type: args.workflow_type.clone(),
        input: args.input.clone(),
    };

    store
        .publish(&args.namespace, &args.instance_id, event)
        .await
        .map_err(|e| ActorProcessingErr::from(Box::new(e)))?;

    tracing::info!(
        instance_id = %args.instance_id,
        "InstanceStarted event published"
    );

    Ok(())
}
```

```rust
// Add to crates/vo-actor/src/instance/actor.rs pre_start, after spawn_live_subscription:
if let Some(c) = consumer {
    init::spawn_live_subscription(&mut state, &myself, c);
}

init::publish_instance_started(&state.args, &event_log).await?;

state.phase = InstancePhase::Live;
```

## 13. Test Plan

| Test | File | Assert |
|------|------|--------|
| Fresh instance publishes | `crates/vo-actor/src/instance/init.rs` (mod tests) | Mock `EventStore::publish` called once with correct variant |
| Crash recovery skips | `crates/vo-actor/src/instance/init.rs` (mod tests) | Non-empty event_log → publish not called |
| Missing event_store | `crates/vo-actor/src/instance/init.rs` (mod tests) | Returns ActorProcessingErr |

## 14. Non-Goals

- Modifying the `InstanceStarted` variant to include `namespace` or `paradigm`
- Publishing `InstanceStarted` from the API layer instead of the actor
- Adding snapshot logic tied to this event

## 15. Rollback Plan

Revert the two changed files (`init.rs`, `actor.rs`). No schema changes, no migrations. Pure additive change — removing it restores prior behavior exactly.

## 16. Definition of Done

1. `publish_instance_started` implemented in `init.rs` with fresh-instance guard
2. Called in `actor.rs:pre_start` after `spawn_live_subscription`, before phase switch
3. Unit tests pass for all three scenarios (fresh, replay, no store)
4. `cargo test --workspace` — green
5. `cargo clippy --workspace -- -D warnings` — clean
6. No new warnings, no unwrap/expect additions
