# ADR-007: sled Schema Design (Trees = Column Families)

## Status

Amended by ADR-013, ADR-014, ADR-019

The full 7-tree schema is superseded. NATS JetStream and NATS KV now own the primary data. sled retains a single tree: `snapshots` (key: `invocation_id`, value: serialized actor state + the JetStream sequence number at which it was taken). All other trees (journal, timers, signals, run_queue, activities, workflows) are replaced by the NATS streams and KV buckets defined in ADR-013 and ADR-014.

## Context

sled uses **Trees** as isolation mechanisms (similar to column families). We need to design a schema that supports:

1. **Instances** - Workflow instance metadata and status
2. **Journal** - Append-only event log (key: invocation_id:seq)
3. **Timers** - Scheduled timer events (key: invocation_id:seq)
4. **Signals** - External events waiting (key: invocation_id:signal_name)
5. **Run queue** - Ready-to-execute instances
6. **Activities** - Registered activity handlers
7. **Workflows** - Workflow definitions

### Key Constraints

- **Append-heavy** - Journal entries are only appended, never updated
- **Point queries** - Get instance by invocation_id, get journal by invocation_id:seq
- **Prefix scans** - Get all journal entries for an instance
- **TTL/cleanup** - Old completed instances should be purgeable

## Decision

We will use **7 Trees** with the following key encoding scheme:

### Tree Definitions

```rust
// Tree names
const INSTANCES: &[u8] = b"instances";
const JOURNAL: &[u8] = b"journal";
const TIMERS: &[u8] = b"timers";
const SIGNALS: &[u8] = b"signals";
const RUN_QUEUE: &[u8] = b"run_queue";
const ACTIVITIES: &[u8] = b"activities";
const WORKFLOWS: &[u8] = b"workflows";
```

### Key Encoding

```rust
// INSTANCES: InvocationId → InstanceData (JSON)
// Key: invocation_id (ULID as string)
// Value: InstanceData JSON

// JOURNAL: InvocationId:seq → JournalEntry (JSON)
// Key: invocation_id:seq (e.g., "01ARZ3NDEKTSV4RRFFQ69G5FAV:0")
// Value: JournalEntry JSON
// Note: Append-only, sorted by seq

// TIMERS: InvocationId:seq → TimerData (JSON)
// Key: invocation_id:seq
// Value: TimerData { fire_at, entry }

// SIGNALS: InvocationId:signal_name → SignalData (JSON)
// Key: invocation_id:signal_name
// Value: SignalData { payload, received_at }

// RUN_QUEUE: InvocationId → enqueued_at (i64 big-endian)
// Key: invocation_id
// Value: enqueued_at timestamp
// Note: Sorted by timestamp for FIFO ordering

// ACTIVITIES: name:version → ActivityData (JSON)
// Key: "charge_card:v1"
// Value: ActivityData { description, registered_at }

// WORKFLOWS: name → WorkflowDef (JSON)
// Key: workflow name
// Value: WorkflowDef JSON (DAG + metadata)
```

### Data Structures

```rust
#[derive(Serialize, Deserialize)]
struct InstanceData {
    name: String,
    status: InstanceStatus,
    input: Vec<u8>,
    output: Option<Vec<u8>>,
    current_step: u32,
    created_at: i64,
    updated_at: i64,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum InstanceStatus {
    Running,
    Completed,
    Failed(String),
    Suspended,
}

#[derive(Serialize, Deserialize)]
struct TimerData {
    invocation_id: String,
    seq: u32,
    fire_at: i64,
}

#[derive(Serialize, Deserialize)]
struct SignalData {
    invocation_id: String,
    name: String,
    payload: Vec<u8>,
    received_at: i64,
}
```

### Transaction Patterns

```rust
// Start new workflow instance
fn start_instance(db: &sled::Db, name: &str, input: Vec<u8>) -> Result<String> {
    let invocation_id = ulid::Ulid::new().to_string();
    let now = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let instance = InstanceData {
        name: name.to_string(),
        status: InstanceStatus::Running,
        input,
        output: None,
        current_step: 0,
        created_at: now,
        updated_at: now,
    };

    let instances = db.open_tree(INSTANCES)?;
    let run_queue = db.open_tree(RUN_QUEUE)?;

    // Multi-tree transaction
    (&instances, &run_queue).transaction(|(tx_inst, tx_queue)| {
        tx_inst.insert(invocation_id.as_bytes(), serde_json::to_vec(&instance)?)?;
        tx_queue.insert(invocation_id.as_bytes(), &now.to_be_bytes())?;
        Ok(())
    })?;

    Ok(invocation_id)
}

// Append journal entry
fn append_journal(db: &sled::Db, invocation_id: &str, seq: u32, entry: &JournalEntry) -> Result<()> {
    let journal = db.open_tree(JOURNAL)?;
    let key = format!("{}:{}", invocation_id, seq);
    journal.insert(key.as_bytes(), serde_json::to_vec(entry)?)?;
    journal.flush_async()?;
    Ok(())
}

// Get all journal entries for instance
fn get_journal(db: &sled::Db, invocation_id: &str) -> Result<Vec<(u32, JournalEntry)>> {
    let journal = db.open_tree(JOURNAL)?;
    let prefix = format!("{}:", invocation_id);
    let mut entries = Vec::new();

    for item in journal.scan_prefix(prefix.as_bytes()) {
        let (key, value) = item?;
        // Parse seq from key
        let key_str = std::str::from_utf8(&key)?;
        let seq = key_str.split(':').nth(1).unwrap().parse::<u32>()?;
        let entry: JournalEntry = serde_json::from_slice(&value)?;
        entries.push((seq, entry));
    }

    entries.sort_by_key(|(seq, _)| *seq);
    Ok(entries)
}

// Get expired timers
fn get_expired_timers(db: &sled::Db) -> Result<Vec<TimerData>> {
    let timers = db.open_tree(TIMERS)?;
    let now = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let mut expired = Vec::new();

    for item in timers.iter() {
        let (key, value) = item?;
        let timer: TimerData = serde_json::from_slice(&value)?;
        if timer.fire_at <= now {
            expired.push(timer);
        }
    }

    Ok(expired)
}
```

## Consequences

### Positive

- Clear separation of concerns (7 trees)
- Prefix scans for journal loading
- Sorted run queue for FIFO ordering
- Multi-tree transactions for atomicity

### Negative

- Manual key encoding/decoding
- No foreign key constraints (sled is KV)

### Future Considerations

- Add secondary indexes for queries (e.g., by workflow name)
- Implement TTL via compaction
- Consider prefix compression for storage optimization
