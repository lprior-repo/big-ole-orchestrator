# ADR-005: Journal-Based Replay for Durability

## Status

Superseded by ADR-013 (NATS JetStream Event Log) and ADR-016 (Deterministic Replay Model)

The journal concept is correct but the storage backend changed. The append-only event log now lives in NATS JetStream (replicated, durable, ordered), not sled. The `JournalEntry` enum is replaced by the typed `WorkflowEvent` enum defined in ADR-013. The replay algorithm described here is preserved in ADR-016 with the critical addition of the write-ahead guarantee: events are committed to JetStream *before* side effects execute, not after.

## Context

wtf-engine must survive **process crashes** and **restarts**. When a workflow is interrupted mid-execution:

1. **What happens to in-flight steps?**
2. **How do we resume without re-executing completed steps?**
3. **How do we maintain exactly-once semantics?**

### Alternative Approaches

| Approach | How It Works | Pros | Cons |
|----------|--------------|------|------|
| **Journal Replay** | Record every step result, re-run on resume, skip completed | Exactly-once, simple | Storage overhead |
| **Checkpointing** | Save state snapshots periodically | Less storage | May re-execute steps |
| **Event Sourcing** | Events are the source of truth | Complete audit trail | Complex replay |
| **Stackful Coroutines** | Save/restore stack state | Fast suspension | Complex implementation |

## Decision

We will use **journal-based replay** (inspired by Restate and Temporal).

### How Journal Replay Works

1. **Every step result is journaled** before returning to caller
2. **On replay** (after crash), the workflow re-executes from the start
3. **At each step**, the journal is checked:
   - If the step has a cached result → return it (skip execution)
   - If the step has no result → execute and journal the result

### JournalEntry Types

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JournalEntry {
    Run {
        name: String,
        input: Vec<u8>,
        output: Option<Vec<u8>>,
    },
    Sleep {
        duration_ms: u64,
        fire_at: Option<i64>,
    },
    WaitForSignal {
        name: String,
        payload: Option<Vec<u8>>,
    },
    GetState {
        key: String,
        value: Option<Vec<u8>>,
    },
    SetState {
        key: String,
        value: Vec<u8>,
    },
    Choice {
        condition: String,
        branch: String,
    },
    Parallel {
        branches: Vec<String>,
        results: Option<Vec<Vec<u8>>>,
    },
    Complete {
        output: Vec<u8>,
    },
    Error {
        error_type: String,
        message: String,
    },
}
```

### Replay Algorithm

```rust
impl WorkflowInstance {
    pub async fn execute_step(&mut self, step_id: u32) -> Result<StepOutput, StepError> {
        // Check journal for existing result
        if let Some(entry) = self.journal.get(step_id as usize) {
            if let Some(output) = entry.get_output() {
                // REPLAY PATH: Return cached result
                tracing::debug!("Replaying step {step_id} from journal");
                return Ok(output);
            }
        }

        // LIVE PATH: Execute step
        let output = self.execute_step_live(step_id).await?;

        // Journal the result
        let journal_entry = JournalEntry::from_output(step_id, &output);
        self.append_journal(journal_entry)?;

        Ok(output)
    }
}
```

### Durability Guarantees

1. **Before** returning from `execute_step`, the journal entry is **flushed to sled**
2. **On crash**, the journal persists
3. **On restart**, the workflow resumes from `journal.len()`

### Optimizations

```rust
// Batch journal writes for performance
struct JournalBuffer {
    entries: Vec<JournalEntry>,
    pending_flush: bool,
}

impl JournalBuffer {
    fn append(&mut self, entry: JournalEntry) {
        self.entries.push(entry);
        self.pending_flush = true;

        // Flush every N entries or on step boundary
        if self.entries.len() >= 10 {
            self.flush();
        }
    }
}
```

## Consequences

### Positive

- **Exactly-once semantics** - Completed steps never re-execute
- **Simple mental model** - Journal is source of truth
- **Crash recovery** - Process can restart at any time
- **Audit trail** - Complete history of step executions
- **Temporal compatibility** - Matches proven Temporal/Restate approach

### Negative

- **Storage overhead** - Every step result is stored
- **Replay cost** - On very long workflows, replay from start may be slow
- **Large inputs** - If steps have large inputs/outputs

### Mitigations

- Periodic snapshots (checkpoint every N steps)
- Compression for large payloads
- Maximum workflow length limits
