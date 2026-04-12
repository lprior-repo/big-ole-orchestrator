# Restate Deep Dive Analysis

**Date:** April 2026
**Context:** This document provides a stupidly detailed deep dive into the architecture and specific implementation mechanics of the `src/restate` repository. It focuses on how Restate solves durable execution, state management, exactly-once semantics, and journaling, extracting the most relevant code structures to inspire Veloxide.

## 1. Storage & Durability (`RocksDbManager`)

Restate leverages **RocksDB** for storage, but it wraps the raw database engine in an elaborate `RocksDbManager` (`crates/rocksdb/src/db_manager.rs`). This manager acts as a single control point for memory budgeting and IO scheduling.

### Key Mechanics:
1. **Memory Budgeting:** It enforces strict memory limits using `WriteBufferManager` and a shared block cache (`Cache::new_hyper_clock_cache`). The manager dynamically adjusts limits based on the configuration (`CommonOptions::rocksdb_total_memory_size`). If memory bounds exceed 90% or 100% of process memory, it issues critical warnings to prevent OOM kills.
2. **Prioritized IO Thread Pools:** Restate uses dedicated high and low priority background thread pools (`threadpool::ThreadPool` named `rs:io-hi` and `rs:io-lo`) to offload blocking RocksDB operations.
3. **Write Path QoS:** It employs `rocksdb::RateLimiter` to throttle write throughput, ensuring compaction and other background tasks aren't starved.

```rust
// Snippet: db_manager.rs initialization
let cache = Cache::new_hyper_clock_cache(opts.rocksdb_total_memory_size().as_usize(), 0);
let write_buffer_manager = WriteBufferManager::new_write_buffer_manager_with_cache(
    opts.rocksdb_total_memtables_size().as_usize(),
    false,
    cache.clone(),
);
let rate_limiter = RateLimiter::new(
    opts.rocksdb_max_write_rate_per_second.as_u64() as i64,
    100 * 1000,
    10,
    RateLimiterMode::KWritesOnly,
    true,
);
```

## 2. Event Sourcing & Journaling (`journal_table`)

Restate uses an append-only journal model. The journal tracks every step of an invocation. 
In `crates/storage-api/src/journal_table/mod.rs`, the core unit of state is a `JournalEntry`.

### Key Mechanics:
- **Separation of Intent vs Completion:** A `JournalEntry` is either an `Entry` (intent to do something, e.g., run a step or schedule a sleep) or a `Completion` (the result of that operation).
- **Budgeted Reads:** The storage API defines `get_journal_entry_budgeted()`, which takes a `LocalMemoryPool`. It checks the serialized size of the entry on disk and acquires a memory lease *before* deserialization. This strictly bounds the maximum memory the replay path can consume, protecting the system from large payloads.

```rust
// Snippet: Journal Entry Definition
pub enum JournalEntry {
    Entry(EnrichedRawEntry),
    Completion(CompletionResult),
}

pub trait ReadJournalTable {
    fn get_journal_entry_budgeted(
        &mut self,
        invocation_id: &InvocationId,
        journal_index: u32,
        budget: &mut LocalMemoryPool,
    ) -> impl Future<
        Output = std::result::Result<Option<(JournalEntry, LocalMemoryLease)>, BudgetedReadError>,
    > + Send;
}
```

## 3. The Core State Machine (`StateMachine`)

The heart of Restate's deterministic logic lives in `crates/worker/src/partition/state_machine/mod.rs`. This state machine runs *per partition* and applies sequences of `Command`s atomically.

### Key Mechanics:
- **Atomic Application:** The `apply()` method takes a `Command`, an LSN (Log Sequence Number), and a mutable `TransactionType` (the storage transaction). It evaluates the command against the current state and writes changes atomically to multiple tables (e.g., `WriteInboxTable`, `WriteJournalTable`, `WriteTimerTable`, `WriteFsmTable`).
- **Idempotency & Deduplication:** When processing an `Invoke` command, it calls `handle_duplicated_requests`. If the `idempotency_key` exists in the `IdempotencyTable`, or if the `InvocationStatus` is anything other than `Free`, it short-circuits. If the prior invocation completed, it re-returns the exact `ResponseResult`.
- **Managed Timers:** Timers are not held in RAM. The `register_timer` function writes a `TimerKeyValue` to the `TimerTable` (storage) and registers it with the `ActionCollector`.

```rust
// Snippet: Command application in StateMachine
pub async fn apply<TransactionType: restate_storage_api::Transaction + Send>(
    &mut self,
    command: Command,
    record_created_at: MillisSinceEpoch,
    record_lsn: Lsn,
    transaction: &mut TransactionType,
    action_collector: &mut ActionCollector,
    vqueues_cache: &mut VQueuesMetaMut,
    is_leader: bool,
) -> Result<(), Error> {
    let res = StateMachineApplyContext {
        storage: transaction,
        record_created_at,
        record_lsn,
        action_collector,
        // ...
    }.on_apply(command).await;
    res
}
```

## 4. Execution & IPC (`InvocationStateMachine`)

When a workflow runs, it is managed by the `InvocationStateMachine` (`crates/invoker-impl/src/invocation_state_machine.rs`).

### Key Mechanics:
- **Journal Tracker:** To know if it is safe to retry an execution, the ISM maintains a `JournalTracker`. It tracks commands sent to the partition processor vs commands ACKed by the partition processor. Retries are only allowed when `last_acked_command >= last_proposed_command`.
- **Attempt State Transitions:** The ISM transitions through `New -> InFlight -> WaitingRetry`. If a transient error occurs and `error_is_transient == true`, it computes the next backoff and suspends into `WaitingRetry`.
- **Memory Budgets:** Like the journal, the `InvocationStateMachine` carries a `budget: Option<LocalMemoryPool>` to strictly limit the memory a single running task can consume, preventing OOM loops.

```rust
// Snippet: Retry gating in JournalTracker
fn can_retry(&self) -> bool {
    let commands_condition = match (
        self.last_acked_command_from_partition_processor,
        self.last_command_sent_to_partition_processor,
    ) {
        (_, None) => true,
        (Some(last_acked_command), Some(last_proposed_command)) => {
            last_acked_command >= last_proposed_command
        }
        _ => false,
    };
    // ...
}
```

## 5. Side Effects & Workflow Progress

Restate manages side effects by requiring the user-code to submit "Intent" entries. 
For side-effects or waiting on external signals, Restate uses the `Awakeable` pattern (frequently mapped to `NotificationId`). The user code suspends (yielding execution), and the state machine waits for an external `Command::NotifySignal` or completion. When received, the state machine records a `Completion` entry and re-activates the invoker.
