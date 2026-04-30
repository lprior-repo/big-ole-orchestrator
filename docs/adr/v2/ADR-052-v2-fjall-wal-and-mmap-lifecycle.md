# ADR 052 (v2): Fjall WAL and Memory-Mapped File Lifecycle

## Status

Accepted

## Context

ADR-002 established Fjall as the storage substrate. ADR-016 established atomic WriteBatches. However, neither ADR documents:

1. **Fjall's persistence model** - how does data survive crashes?
2. **Write coalescing** - how does the DbWriterActor batch commits?
3. **Memory-mapped file lifecycle** - when are files mapped/unmapped?
4. **Persistence modes** - when to use `Buffer` vs `SyncAll`?

This ADR fills those gaps.

## Decision

### 1. Fjall Persistence Model: No Traditional WAL

Fjall v3 is an **LSM-tree** (Log-Structured Merge-tree), not a B-tree with a WAL. This has critical implications:

| Traditional WAL DB | Fjall (LSM-tree) |
|--------------------|------------------|
| Write goes to WAL first, then to memtable | Write goes directly to memtable |
| WAL is append-only, fsynced | Memtable is flushed to SST files |
| Crash recovery replays WAL | Crash recovery replays memtable flush |

**Durability guarantee**: A write is durable when the memtable containing it is flushed to an SST file and the SST file is synced.

### 2. Memtable Flush as Durability Boundary

The memtable (in-memory write buffer) is flushed to disk when:
- It reaches `max_memtable_size` (configured per partition class: 64MB hot, 256MB cold, 1GB blob)
- Compaction triggers a flush
- Process exits cleanly

**Implication**: If the process crashes with data only in the memtable, that data is LOST. This is a known trade-off of LSM trees.

### 3. Persistence Modes

Fjall provides two persistence modes via `db.persist()`:

| Mode | Behavior | Use Case |
|------|----------|----------|
| `PersistMode::Buffer` | Flush to OS page cache, return immediately | Hot path: API handlers (ADR-002) |
| `SyncAll` | Force fsync to durable storage | Critical commits, tests, snapshots |

**Hot path** (`PersistMode::Buffer`):
```rust
// event_log.rs:51
db.persist(fjall::PersistMode::Buffer)?;
```
This is safe because the `DbWriterActor` periodically calls `SyncAll` to flush the page cache to disk.

**Critical path** (`PersistMode::SyncAll`):
- Snapshot writes
- Saga recovery operations
- Backup/restore operations
- Test assertions requiring durability

### 4. DbWriterActor Group Commit

ADR-002 mandates that "API handlers must not fsync per event." The `DbWriterActor` implements **group commit**:

1. Actors send writes to `DbWriterActor` via channels
2. `DbWriterActor` collects writes and batches them into `fjall::Batch`
3. Batch is committed with `PersistMode::Buffer` (fast path)
4. A background task periodically calls `db.persist(SyncAll)` to durability-sync

This maximizes NVMe IOPS while ensuring eventual durability.

### 5. Memory-Mapped File Lifecycle

Two distinct mmap usages exist:

#### 5a. Fjall Internal SST Mmap

Fjall memory-maps SST (Sorted String Table) files for reading data:
- OS manages mapping via page cache
- No explicit unmapping needed (OS handles it)
- Benefits from OS file cache optimizations

#### 5b. MmapCache (Application-Level)

`vo-storage/src/mmap_cache.rs` provides application-level mmap for blob data:

```
MmapCache lifecycle:
  insert(key, data)
    → allocate_region(key, size)
    → write_data_to_region(key, data)  // writes to file
    → file.flush()                     // OS may still buffer
    → LRU tracking

  get(key)
    → open file
    → Mmap::map()                      // OS maps file into address space
    → read data
    → drop mmap (OS may unmap or keep)

  evict (LRU)
    → remove file
    → OS automatically unmaps
```

**Configuration**:
- `max_memory_bytes`: Maximum aggregated size of mapped regions
- LRU eviction when capacity exceeded
- Files stored in `base_path` directory

### 6. Write Coalescing

The `BudgetQueues` in `append.rs` implement **write coalescing** via class-based batching:

```
CriticalControlPlane → highest priority, never dropped
OperatorProjection   → may lag under pressure
BulkBlob            → deferred under pressure
```

Dequeue order: Critical → Projection → Blob

## Consequences

- **Positive:** High write throughput via group commit and page cache buffering
- **Positive:** No WAL overhead - simpler architecture
- **Negative:** Small window of data loss on crash (data in memtable not yet flushed)
- **Negative:** Recovery may replay more events than a WAL-based system (no partial transaction recovery)
- **Negative:** MmapCache consumes address space proportional to blob access patterns

## Invariants

1. **WAL is NOT used** - Fjall's memtable flush IS the durability boundary
2. **SyncAll is called periodically** - The DbWriterActor background task ensures durability
3. **Memtable flush triggers durability** - Data survives crash only after SST flush
4. **MmapCache LRU respects capacity** - Eviction happens before exceeding limit

## Test Requirements

Tests must verify:
1. WAL replay recovers uncommitted transactions (if any)
2. mmap works correctly after restart
3. WAL corruption is detected on startup
4. mmap failure is handled gracefully