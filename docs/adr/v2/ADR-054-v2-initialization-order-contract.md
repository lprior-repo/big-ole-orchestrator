# ADR 054: Initialization Order Contract

## Status

Accepted

## Context

The Veloxide workflow engine comprises multiple subsystems that must be initialized in a specific order to ensure correctness. Without a formal contract:

1. Storage (Fjall) must be ready before any subsystem that persists data
2. The Reanimator Loop requires timer storage and work queue before spawning
3. The Scheduler requires a JobStore before accepting jobs
4. The Actor system (MasterOrchestrator) must be ready before processing messages
5. Crash recovery must complete before accepting new work

Currently, initialization order is implicit and enforced only through Rust's type system at the call site. This has several problems:

- No documented contract for component initialization order
- No way to verify initialization invariants at compile time or runtime
- No clear error handling when dependencies are not ready
- Risk of use-before-init bugs in new code

This ADR defines the canonical initialization order contract for the Veloxide engine.

## Decision

### 1. Initialization Phases

The Veloxide engine initializes in five distinct phases:

```
Phase 1: Storage Foundation
Phase 2: Storage Partitions
Phase 3: Actor System
Phase 4: Background Services
Phase 5: Runtime Acceptance
```

### 2. Phase Specifications

#### Phase 1: Storage Foundation

**Components**: Fjall Database

**Required Action**:
```rust
let db = fjall::Database::open(path)?;
```

**Contract**:
- The Fjall database MUST be opened and verified before any other storage operations
- Database handle MUST be shared via `Arc<fjall::Database>` to all partitions
- If database open fails, the entire engine MUST fail to start
- No other component may access storage until this phase completes

**Invariants**:
- `db.is_open() == true` before proceeding
- Database file locks acquired

#### Phase 2: Storage Partitions

**Components**: All keyspace-backed stores

**Required Action**:
```rust
let layout = FjallPartitionLayout::open(&db)?;
let stores = StorageStores::from_layout(&layout)?;
```

**Contract**:
- All stores MUST be opened from the same `FjallPartitionLayout`
- The following stores MUST be opened in this order:
  1. `DedupeStore` - exactly-once ingress deduplication
  2. `EffectJournal` - event sourcing journal
  3. `LeaseStore` - distributed leasing
  4. `EventStore` - workflow events
  5. `WorkflowVersionStore` - workflow definitions
  6. `SnapshotStore` - state snapshots
  7. `InstanceIndex` - instance lookup
  8. `TimerIndex` - timer wheel

- Each store MUST verify its keyspace is accessible before returning
- If any store fails to open, the engine MUST fail with descriptive error

**Invariants**:
- All stores return `Result<T, StorageError>`
- No store may be used before successful open
- Stores are `Send + Sync` for multi-threaded access

#### Phase 3: Actor System

**Components**: MasterOrchestrator, ControlActor

**Required Action**:
```rust
let orchestrator = MasterOrchestrator::new(config);
let control_actor = ControlActor::with_state_lookup(
    signal_storage,
    work_queue,
    state_lookup,
);
```

**Contract**:
- Actor system initialization requires Phase 2 complete
- `MasterOrchestrator` MUST be spawned in the Ractor actor system
- `ControlActor` MUST be created with valid storage handles
- Actor system MUST enter `Running` state before accepting messages

**Invariants**:
- Actor references are `Clone + Send + Sync`
- No messages processed until actor enters `Running` state
- Actor mailbox initialized with bounded capacity

#### Phase 4: Background Services

**Components**: Scheduler, ReanimatorLoop

**Required Action**:
```rust
// Scheduler
let scheduler = Scheduler::new(capacity, store, dispatcher);

// Reanimator
let reanimator = ReanimatorLoop::spawn(config, timer_storage, work_queue);
```

**Contract**:
- Scheduler MUST be created before ReanimatorLoop (for job scheduling)
- ReanimatorLoop runs crash recovery synchronously before returning
- Crash recovery MUST complete before accepting new timer work
- Both services enter `Running` state before accepting work

**Invariants**:
- Scheduler tick runs on interval, no immediate tick on spawn
- Reanimator loop scans every `config.scan_interval`
- Crash recovery is idempotent - safe to run multiple times

#### Phase 5: Runtime Acceptance

**Components**: API Server, Health Checks

**Required Action**:
```rust
// All systems nominal
let health = HealthStatus {
    storage: true,
    actors: true,
    scheduler: true,
    reanimator: true,
};
```

**Contract**:
- All phases 1-4 MUST complete successfully
- Health endpoint returns `healthy` only when all components ready
- Engine accepts workflow submissions only in this phase
- Graceful shutdown initiates from this state

**Invariants**:
- No workflow may be created before all systems ready
- Health check is eventual - may return degraded during transitions

### 3. Error Handling

| Phase | Failure Mode | Required Action |
|-------|-------------|----------------|
| 1 | Database open fails | Fail fast, exit process |
| 2 | Store open fails | Fail fast, exit process |
| 3 | Actor spawn fails | Fail fast, exit process |
| 4 | Service spawn fails | Fail fast, exit process |
| 5 | Health check fails | Return degraded status |

### 4. Shutdown Order

Shutdown MUST happen in reverse order:

```
Phase 5: Stop accepting new work
Phase 4: Stop Scheduler, then ReanimatorLoop
Phase 3: Stop all actors (supervised shutdown)
Phase 2: Close all stores
Phase 1: Close database
```

### 5. Dependency Graph

```
                    ┌─────────────────┐
                    │  Fjall Database │
                    └────────┬────────┘
                             │
              ┌──────────────┼──────────────┐
              │              │              │
              ▼              ▼              ▼
        ┌──────────┐ ┌──────────┐ ┌──────────┐
        │ Dedupe   │ │ Events   │ │ Timers   │
        └────┬─────┘ └────┬─────┘ └────┬─────┘
             │             │             │
             └─────────────┼─────────────┘
                           │
                    ┌──────┴──────┐
                    │    Stores   │
                    └──────┬──────┘
                           │
         ┌─────────────────┼─────────────────┐
         │                 │                 │
         ▼                 ▼                 ▼
   ┌──────────┐     ┌──────────────┐  ┌──────────┐
   │ Scheduler│     │ReanimatorLoop│  │  Actor   │
   └────┬─────┘     └──────┬───────┘  │  System  │
        │                  │          └────┬─────┘
        └──────────────────┼───────────────┘
                           │
                    ┌──────┴──────┐
                    │   Runtime   │
                    │  Acceptance │
                    └─────────────┘
```

### 6. Initialization Barrier

All phases after Phase 1 MUST use an initialization barrier:

```rust
pub struct InitBarrier {
    phases: [AtomicBool; 5],
}

impl InitBarrier {
    pub fn phase_complete(&self, phase: Phase) -> bool {
        self.phases[phase as usize].load(Ordering::SeqCst)
    }

    pub fn mark_complete(&self, phase: Phase) {
        self.phases[phase as usize].store(true, Ordering::SeqCst);
    }
}
```

Components MUST check `init_barrier.phase_complete(phase)` before starting work.

### 7. Test Scenarios

#### 7.1 Init Order Happy Path

```
Given: Fresh database
When: Engine initializes
Then: All 5 phases complete
And: Health endpoint returns healthy
And: Engine accepts workflow
```

#### 7.2 Storage Failure

```
Given: Corrupted database
When: Phase 1 initializes
Then: Engine fails with descriptive error
And: No partial state written
```

#### 7.3 Reanimator Crash Recovery

```
Given: Previous crash with pending timers
When: Phase 4 initializes
Then: run_crash_recovery() completes
And: All non-terminal timers replayed
And: Stale timers cleaned up
And: Loop begins accepting new timers
```

#### 7.4 Scheduler Before Reanimator

```
Given: Both services initializing
When: Phase 4 runs
Then: Scheduler created before ReanimatorLoop
And: Scheduler can enqueue jobs while Reanimator recovers
```

## Consequences

### Positive

- Initialization order is now explicit and documented
- Fail-fast behavior prevents partial initialization states
- Crash recovery is first-class concern in init order
- Health checks can accurately report subsystem status

### Negative

- Stricter initialization sequence may limit startup parallelization
- Additional ceremony for adding new subsystems
- Must update ADR when adding new initialization phases

### Neutral

- Existing code already follows this order implicitly
- ADR serves as documentation, not enforcement mechanism
