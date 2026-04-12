# Ideas and Concrete Snippets for Veloxide v2

**Date:** April 2026
**Context:** Based on the deep dive into `src/restate`, this document extracts architectural ideas and provides actionable Rust snippets that align with Veloxide's ADRs (Single-Node, Fjall-backed, Process-Isolated).

## Idea 1: Memory-Budgeted Replay (OOM Protection)
**Problem:** In Veloxide, pulling large payload blobs or massive journal histories during deterministic replay could easily OOM the node.
**Restate's Solution:** Restate uses a `LocalMemoryPool` and `LocalMemoryLease`. They read the raw size of an entry from disk, request a lease, and only deserialize if the lease is granted.
**Application to Veloxide:** Integrate a `ReplayBudget` into the `DbWriterActor` and `ReplayEngine`. Before reading canonical blobs from Fjall, require a lease.

**Implementation Snippet (Veloxide `vo-core`):**
```rust
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct GlobalMemoryBudget {
    capacity: usize,
    used: AtomicUsize,
}

impl GlobalMemoryBudget {
    pub fn try_acquire(&self, bytes: usize) -> Option<MemoryLease> {
        let mut current = self.used.load(Ordering::Acquire);
        loop {
            if current + bytes > self.capacity {
                return None; // OOM prevented!
            }
            match self.used.compare_exchange_weak(current, current + bytes, Ordering::Release, Ordering::Relaxed) {
                Ok(_) => return Some(MemoryLease { budget: self, amount: bytes }),
                Err(val) => current = val,
            }
        }
    }
}

pub struct MemoryLease<'a> {
    budget: &'a GlobalMemoryBudget,
    amount: usize,
}

impl<'a> Drop for MemoryLease<'a> {
    fn drop(&mut self) {
        self.budget.used.fetch_sub(self.amount, Ordering::Release);
    }
}
```

## Idea 2: Command/Action Separation in the State Machine
**Problem:** To ensure pure determinism, Veloxide's state transitions must have zero external side effects.
**Restate's Solution:** Restate's `StateMachine` never triggers RPCs or spawns tasks directly. It mutates storage and pushes `Action` enums into an `ActionCollector`. The caller (e.g., the worker loop) handles the `ActionCollector` outputs.
**Application to Veloxide:** Refactor `ReplayEngine` and `vo-actor` to use an `EffectCollector` or `ActionCollector`.

**Implementation Snippet (Veloxide `vo-core/state_machine.rs`):**
```rust
pub enum StateMachineAction {
    WakeUpTimer(TimerId),
    SpawnProcess(StepId),
    AcknowledgeSignal(SignalId),
}

pub struct ActionCollector {
    actions: Vec<StateMachineAction>,
}

impl ActionCollector {
    pub fn push(&mut self, action: StateMachineAction) {
        self.actions.push(action);
    }
}

// In the Replay Engine:
pub fn apply_event(state: &mut LifecycleState, event: EventEnvelope, actions: &mut ActionCollector) {
    match event.payload {
        EventPayload::TimerScheduled { fire_at } => {
            // pure state update
            state.timers.insert(fire_at);
            // declare intent
            actions.push(StateMachineAction::WakeUpTimer(event.id));
        }
        // ...
    }
}
```

## Idea 3: Strict Journal Tracker for Process Invocation
**Problem:** In Veloxide v2, execution is done via Subprocesses over FD3/FD4. If the process crashes or hangs, how do we know exactly what it committed?
**Restate's Solution:** `JournalTracker` maintains `last_acked_command` and `last_proposed_command`. 
**Application to Veloxide:** The `vo-ipc` executor must track which FD4 output boundaries have actually been durably written to the `DbWriterActor`. Veloxide should not allow a retry until `last_durable_fence >= last_proposed_fence`.

**Implementation Snippet (Veloxide `vo-executor`):**
```rust
pub struct SubprocessExecutionTracker {
    last_proposed_fence: u64,
    last_durable_fence: u64,
}

impl SubprocessExecutionTracker {
    pub fn notify_fd4_output(&mut self, fence_id: u64) {
        self.last_proposed_fence = std::cmp::max(self.last_proposed_fence, fence_id);
    }

    pub fn notify_db_writer_ack(&mut self, fence_id: u64) {
        self.last_durable_fence = std::cmp::max(self.last_durable_fence, fence_id);
    }

    pub fn is_safe_to_retry(&self) -> bool {
        // We can only restart the subprocess if all previous FD4 writes are durable in Fjall
        self.last_durable_fence >= self.last_proposed_fence
    }
}
```

## Idea 4: Intent vs Completion Journaling
**Problem:** Exact-once side effects require a 2-phase commit.
**Restate's Solution:** A `JournalEntry` is defined as either `Entry` (Intent) or `Completion`.
**Application to Veloxide:** Explicitly enforce `EffectIntent` and `EffectCompletion` in the `effect_journal`.

**Implementation Snippet (Veloxide `vo-types`):**
```rust
pub enum EffectJournalEntry {
    Intent {
        step_id: StepId,
        connector_id: String,
        payload: Vec<u8>,
    },
    Completion {
        step_id: StepId,
        result: Result<Vec<u8>, ExecutionError>,
    }
}
```

By adopting Restate's Action Collectors, memory-budgeted reads, strict Journal Tracking, and Intent/Completion split, Veloxide v2 can achieve extreme determinism while protecting the host OS from memory bombs and split-brain subprocess execution.
