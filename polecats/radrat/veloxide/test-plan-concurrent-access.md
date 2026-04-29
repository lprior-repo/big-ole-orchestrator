# Test Plan: Concurrent Access & Race Conditions

## Summary
- Behaviors identified: 12
- Trophy allocation: 6 unit / 4 integration / 2 e2e
- Proptest invariants: 3
- Fuzz targets: 1
- Kani harnesses: 1

## 1. Behavior Inventory

### Deduplication & Uniqueness

1. "Deduplication prevents double instance creation on simultaneous start"
2. "Exactly one instance wins the dedupe race"

### Signal & Timer Concurrency

3. "Signal and timer racing for same instance results in exactly one winner"
4. "No double resume occurs on signal/timer race"

### Compensation Concurrency

5. "Concurrent compensate requests result in exactly one compensation"
6. "Second compensate request is idempotent after first starts"

### Priority-Based Conflict Resolution

7. "Terminate wins over signal when racing for same instance"
8. "Start completes before terminate when racing for same instance"

### Read Consistency

9. "Concurrent status queries return consistent snapshot"
10. "No partial state visible during concurrent queries"

### Fence Token

11. "Fence token prevents double managed effect commit"
12. "Crash recovery restores in-flight effect from journal"

### Broadcast & Backpressure

13. "SSE broadcast delivers events to all 10 connected clients"
14. "SSE client lagging beyond 1000 events triggers connection drop"
15. "WebSocket client lagging beyond 1000 events silently drops events"

### Search Consistency

16. "Concurrent search and modification returns consistent results"

## 2. Trophy Allocation

| Layer | Count | Rationale |
|-------|-------|-----------|
| Unit (Calc) | 6 | `dedupe_key` hashing, fence token logic, priority ordering, snapshot isolation |
| Integration | 4 | SSE/WebSocket broadcast, concurrent query consistency, crash recovery |
| E2E | 2 | Full racing scenario simulation, SSE/WebSocket backpressure |
| Static | 1 | Clippy on all modules |

## 3. BDD Scenarios

### Behavior 1: Deduplication prevents double instance creation on simultaneous start

```
Given: WorkflowEngine with dedupe_key="workflow-123", no existing instance
And: two concurrent start requests R1 and R2 arriving within same tick
When: both R1 and R2 execute start_instance for same dedupe_key simultaneously
Then: exactly one instance is created with dedupe_key="workflow-123"
And: the other request receives InstanceError::DeduplicateKeyExists
```

**Test name**: `fn concurrent_start_same_dedupe_key_one_wins`

### Behavior 2: Exactly one instance wins the dedupe race

```
Given: WorkflowEngine processing two identical dedupe_key start requests
When: requests race at the instance creation point
Then: one request's instance is committed to the registry
And: the other request's instance is rejected with DeduplicateKeyExists
And: instance count remains exactly 1
```

**Test name**: `fn dedupe_race_exactly_one_instance_created`

### Behavior 3: Signal and timer racing for same instance results in exactly one winner

```
Given: Instance I with signal handler and active timer
And: signal SIG and timer TIMER both targeting instance I
When: SIG and TIMER fire in same event loop tick
Then: exactly one of signal handler or timer callback executes
And: the other is skipped (not queued for next tick)
```

**Test name**: `fn signal_timer_race_exactly_one_wins`

### Behavior 4: No double resume occurs on signal/timer race

```
Given: Instance I in paused state waiting for resume
And: signal handler would call resume_on_signal(I)
And: timer callback would call resume_on_timer(I)
When: signal and timer fire simultaneously
Then: instance I is resumed exactly once
And: instance I is not double-resumed (which would cause state corruption)
```

**Test name**: `fn signal_timer_race_no_double_resume`

### Behavior 5: Concurrent compensate requests result in exactly one compensation

```
Given: Instance I currently executing with active effects
And: two concurrent compensate requests C1 and C2 for same instance I
When: C1 and C2 race to initiate compensation
Then: exactly one compensation process begins
And: the other request receives CompensateError::AlreadyCompensating
And: compensation state transitions: Idle -> Compensating -> Completed
```

**Test name**: `fn concurrent_compensate_exactly_one_runs`

### Behavior 6: Second compensate request is idempotent after first starts

```
Given: Instance I currently compensating
When: a second compensate request arrives for instance I
Then: second request returns CompensateError::AlreadyCompensating
And: compensation continues to completion for first request
And: no additional effects are triggered
```

**Test name**: `fn compensate_idempotent_second_request_rejected`

### Behavior 7: Terminate wins over signal when racing for same instance

```
Given: Instance I in active state
And: signal SIG and terminate TERM both targeting instance I
When: SIG and TERM arrive in same event loop tick
Then: TERM takes precedence over SIG
And: instance I transitions to terminating state
And: signal SIG is not processed (terminating supersedes signaling)
```

**Test name**: `fn terminate_wins_over_signal_race`

### Behavior 8: Start completes before terminate when racing for same instance

```
Given: no instance with dedupe_key="workflow-X" exists
And: start request S and terminate request T for dedupe_key="workflow-X" race
When: S and T arrive simultaneously
Then: start request S creates instance I
And: terminate request T then applies to instance I
And: final state reflects terminate applied after start completes
```

**Test name**: `fn start_terminate_race_start_wins_then_terminate_applies`

### Behavior 9: Concurrent status queries return consistent snapshot

```
Given: Instance I with complex nested state (effects, timers, signals)
And: N concurrent status_query(I) requests where N >= 10
When: all N queries execute simultaneously
Then: all N queries return identical snapshot of instance I state
And: no query observes partial state (e.g., mid-transition effects)
And: snapshot reflects a consistent point in time
```

**Test name**: `fn concurrent_status_queries_return_identical_snapshot`

### Behavior 10: No partial state visible during concurrent queries

```
Given: Instance I undergoing state transitions
When: concurrent reads occur during transition
Then: read either returns pre-transition state or post-transition state
And: no hybrid/partial state is observable (e.g., effects list partially updated)
```

**Test name**: `fn concurrent_reads_never_see_partial_state`

### Behavior 11: Fence token prevents double managed effect commit

```
Given: ManagedEffect M with fence_token="token-abc"
And: M is in pending state awaiting commit
And: two concurrent commit requests C1 and C2 for effect M with same fence_token
When: C1 and C2 race at commit point
Then: exactly one commit succeeds
And: fence_token is consumed on successful commit
And: second commit returns EffectError::FenceTokenAlreadyConsumed
```

**Test name**: `fn fence_token_prevents_double_commit`

### Behavior 12: Crash recovery restores in-flight effect from journal

```
Given: Instance I with in-flight effect E in pending_commit state
And: journal J contains EffectState for E
When: engine crashes and restarts
Then: upon restart, engine replays journal J
And: effect E is recovered to pre-crash state
And: effect E can continue/commit after recovery
```

**Test name**: `fn in_flight_effect_recovered_after_crash`

### Behavior 13: SSE broadcast delivers events to all connected clients

```
Given: 10 SSE clients C1-C10 subscribed to instance I events
When: instance I emits event E
Then: all 10 clients C1-C10 receive event E
And: delivery occurs within single event loop tick
And: broadcast semantics (each client receives exactly once)
```

**Test name**: `fn sse_broadcast_to_10_clients_all_receive`

### Behavior 14: SSE client lagging beyond 1000 events triggers connection drop

```
Given: SSE client C connected to instance I
And: C has fallen behind by more than 1000 events
When: lag is detected by event delivery subsystem
Then: connection to C is dropped
And: C is removed from subscriber list
And: other clients continue to receive events
```

**Test name**: `fn sse_client_lag_1000_dropped`

### Behavior 15: WebSocket client lagging beyond 1000 events silently drops events

```
Given: WebSocket client W connected to instance I
And: W has fallen behind by more than 1000 events
When: lag is detected by event delivery subsystem
Then: events for W are silently dropped (not queued)
And: connection to W remains open
And: W can catch up when ready (only receives new events)
```

**Test name**: `fn websocket_client_lag_1000_silent_drop`

### Behavior 16: Concurrent search and modification returns consistent results

```
Given: Instance I with search index S
And: concurrent search query Q running against S
And: concurrent instance modification M updating I
When: Q and M execute simultaneously
Then: search Q returns results consistent with pre-modification or post-modification state
And: no dirty reads occur (modified fields not partially visible)
And: search index remains valid after modification
```

**Test name**: `fn concurrent_search_modification_no_dirty_reads`

## 4. Invariant Properties (Proptest)

### Invariant 1: Dedupe key uniqueness
```
prop: for all dedupe_keys, instance_count(dedupe_key) <= 1
```

### Invariant 2: State transition atomicity
```
prop: for all state transitions, either fully complete or fully roll back
```

### Invariant 3: Broadcast delivery guarantee
```
prop: for all broadcasts, if client connected at time of broadcast, client receives event
```

## 5. Fuzz Targets

### Target 1: Concurrent start race
```
fuzz_target: concurrent_start_race_with_dedupe
- Generate: dedupe_key, timing delta between requests
- Input: two start requests with same dedupe_key
- Oracle: exactly one instance created, exactly one success response
```

## 6. Kani Harnesses

### Harness 1: Fence token atomicity
```
 harness: verify_fence_token_prevents_double_commit
 - Model: fence token as discrete state machine
 - Property: commit can only succeed once per token
```
