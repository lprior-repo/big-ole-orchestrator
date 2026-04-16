# Test Plan: Crash Recovery - Engine Restarts

**Contract**: `crates/vo-core/src/exact_once_verification/`, `crates/vo-core/src/replay/`, `crates/vo-actor/src/reanimator/`
**Issue**: ve-koc4h
**Parent**: ve-hf48p
**Target crates**: `vo-core`, `vo-actor`, `vo-storage`
**Bead**: ve-koc4h

## Scope

This plan covers BDD scenarios for engine crash recovery — specifically the restart and recovery behavior when the engine crashes with active instances, in-flight steps, timers, signals, and managed effects. Recovery must be deterministic, idempotent, and budget-isolated from live ingress.

---

## BDD Scenarios

### Scenario 1: Engine crashes with 100 active instances — all recovered incrementally

```
### Behavior: Engine restart recovers all 100 active instances incrementally
Given: Engine running with 100 active instances in various states (RunningDecision, StepScheduled, StepExecuting, WaitingForTimer)
And: Engine process crashes (simulated via SIGKILL)
When: Engine restarts and recovery begins
Then: All 100 instances are recovered via deterministic replay
And: Recovery processes instances in configurable batches (throttle)
And: Each instance is reconstructed to its exact pre-crash state
And: No instance is lost or duplicated
```

### Scenario 2: Engine crashes during pure step execution — step re-executed (deterministic replay)

```
### Behavior: Crash during pure step execution replays and re-executes step
Given: Instance in state StepExecuting a pure step (no side effects)
And: Engine crashes after step started but before completion
When: Engine restarts and instance is replayed
Then: ReplayEngine reconstructs StepExecuting state from journal
And: Step is re-executed from beginning (deterministic replay)
And: If step is idempotent, final result matches original execution
And: If step is non-idempotent, fence token prevents double execution
```

### Scenario 3: Engine crashes during managed effect commit — fence token prevents double commit

```
### Behavior: Crash during managed effect commit uses fence token to prevent double commit
Given: Instance with EffectPrepared and fence token acquired
And: Engine crashes during EffectCommitted phase
When: Engine restarts and instance is replayed
Then: Fence token is checked before committing
And: If fence token is already committed, effect is skipped (idempotent)
And: If fence token is not committed, effect is re-prepared and committed
And: No effect is executed more than once (exact-once semantics)
```

### Scenario 4: Engine crashes during signal delivery — signal redelivered from journal

```
### Behavior: Crash during signal delivery redelivers signal from journal
Given: Instance in WaitingForSignal state with signal recorded in journal
And: Engine crashes after signal acceptance but before completion
When: Engine restarts and instance is replayed
Then: Signal is re-delivered from journal entry
And: Signal acceptance is idempotent (same signal not delivered twice)
And: Instance resumes execution after signal is re-accepted
```

### Scenario 5: Engine crashes during timer fire — timer state recovered, fires if overdue

```
### Behavior: Crash during timer fire recovers timer state and fires if overdue
Given: Instance in WaitingForTimer state with timer due to fire
And: Engine crashes just before timer fire handler executes
When: Engine restarts and recovery runs
Then: Timer state is recovered from persistent storage
And: If current time >= timer expiry, timer fires immediately
And: If current time < timer expiry, timer is rescheduled
And: Instance transitions correctly after timer fire
```

### Scenario 6: Engine crashes during compensation — compensation resumes from last committed effect

```
### Behavior: Crash during compensation resumes from last committed effect
Given: Instance in Compensating state with effects already committed
And: Engine crashes mid-compensation sequence
When: Engine restarts and compensation recovery runs
Then: Compensation resumes from last committed effect (Saga compensation ADR-034)
And: Effects are rolled back in reverse order
And: Each compensation step is idempotent
And: Saga reaches Compensated or FailedTerminal state
```

### Scenario 7: Recovery ordering — 5000 instances processed in configurable batches (throttle)

```
### Behavior: Recovery of 5000 instances is throttled to prevent overload
Given: Engine crashes with 5000 active instances requiring recovery
And: Recovery throttle is configured (e.g., 100 instances per batch)
When: Recovery begins
Then: Instances are processed in batches of configured size
And: Each batch completes before next batch begins
And: Recovery progress is visible (recovered/total)
And: Live ingress continues to be accepted during recovery
```

### Scenario 8: New ingress during recovery — accepted with budget isolation

```
### Behavior: New ingress during recovery is accepted with budget isolation
Given: Engine is in recovery mode processing 5000 backlogged instances
And: Recovery budget is 80% of capacity
When: New POST /workflow/start request arrives
Then: Request is accepted (20% budget reserved for live traffic)
And: New instance is started immediately (not queued behind recovery)
And: Recovery continues concurrently with live traffic
And: Recovery throughput is degraded gracefully, not blocked
```

### Scenario 9: Recovery budget — 80% capacity used, live ingress has reserved 20%

```
### Behavior: Recovery at 80% capacity still allows live ingress at 20%
Given: Engine has 1000 capacity units
And: Recovery is consuming 800 units (80%)
When: Live ingress request arrives
Then: Request is accepted using reserved 20% (200 units)
And: If live traffic exceeds 20%, excess is shed (load shedding)
And: Recovery is not paused to make room for live traffic
And: Budget isolation between recovery and live is enforced
```

### Scenario 10: Recovery with corrupted event — instance quarantined, others unaffected

```
### Behavior: Corrupted event during replay quarantines instance, others recover
Given: 100 instances being replayed after crash
And: Instance #42 has a corrupted event (malformed JSON, invalid payload)
When: Recovery encounters the corrupted event
Then: Instance #42 is quarantined (moved to Failed state with Quarantined reason)
And: Remaining 99 instances continue recovering normally
And: Corrupted event is logged for investigation
And: Quarantine does not block or slow recovery of other instances
```

### Scenario 11: Recovery with newer schema version — upcaster applied if registered

```
### Behavior: Event with newer schema version is upcasted if upcaster exists
Given: Instance has events from schema version 0 and version 1
And: Engine was upgraded to schema version 1
And: Upcaster from v0 to v1 is registered in UpcasterRegistry
When: Recovery replays events
Then: v0 events are upcasted to v1 using registered upcaster
And: Upcasted events are replayed correctly
And: If upcaster is not registered, ReplayError::UpcastingFailed is returned
And: Instance is quarantined if upcasting fails
```

### Scenario 12: Clean SIGTERM — in-flight steps complete, instances suspended to disk

```
### Behavior: Clean SIGTERM completes in-flight steps and suspends instances
Given: Engine running with active instances
And: SIGTERM is received (graceful shutdown)
When: Engine initiates graceful shutdown
Then: In-flight steps are allowed to complete
And: All instances are suspended to disk (persisted state)
And: Engine waits for completion timeout before force-exiting
And: On restart, instances resume from suspended state (not full replay)
```

### Scenario 13: Force kill (SIGKILL) — recovery on next start replays all from journal

```
### Behavior: Force kill (SIGKILL) triggers full journal replay on restart
Given: Engine running with active instances
And: SIGKILL is received (force kill, no graceful shutdown)
When: Engine restarts
Then: Full replay of all journaled events is performed
And: No instances are assumed to be complete unless journaled
And: No reliance on in-memory state (all state is journaled)
And: Recovery is deterministic and idempotent
```

### Scenario 14: Recovery after multiple crashes — each recovery is idempotent

```
### Behavior: Multiple crash loops result in correct idempotent recovery
Given: Engine crashes, recovers, crashes again, recovers again
And: Each recovery is idempotent (can be run multiple times safely)
When: Third crash occurs and recovery runs
Then: Recovery replays journal correctly despite previous incomplete recovery
And: No state is lost or duplicated from previous recovery attempts
And: Each recovery produces same final state
And: Crash loop detection triggers if recovery threshold exceeded
```

### Scenario 15: Recovery progress — recovered/total instances visible

```
### Behavior: Recovery progress queryable showing recovered and total instances
Given: Engine is recovering from crash with N instances
When: Recovery progress is queried
Then: Response includes: { recovered: M, total: N, percent_complete: M/N }
And: Progress is updated in real-time as instances recover
And: If instance fails quarantine, it is counted as failed, not pending
And: Recovery can be paused and resumed without losing progress
```

---

## Trophy Allocation

| Layer | Count | Rationale |
|-------|-------|-----------|
| Unit | 25 | Pure recovery state machine, idempotency proofs, throttle logic |
| Integration | 15 | Full crash injection at each of 12 crash points + multi-instance scenarios |
| Property (proptest) | 10 | Invariants: determinism, idempotency, budget isolation |
| Fuzz | 5 | Corrupted event variants, schema evolution edge cases |
| Kani | 5 | Critical: exact-once, no double-commit, no state loss |
| **Total** | 60 | |

---

## Acceptance Criteria

1. All 15 BDD scenarios have passing tests
2. Crash injection at all 12 crash points results in recovery
3. Recovery is deterministic (same events → same state)
4. Recovery is idempotent (run twice → same result)
5. Budget isolation between recovery and live traffic is enforced
6. Corrupted events quarantine instance without blocking others
7. Upcaster integration works for schema evolution
8. Recovery progress is observable and queryable
