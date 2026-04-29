# Findings: tw-sucv - Fjall Partition Compaction Must Not Block Reads

## Issue Summary

The issue reports that in `crates/vo-storage/src/`, when Fjall triggers compaction on a partition, **read operations block until compaction completes**, causing read latency spikes of 500ms+ under heavy write load.

## Root Cause

The Fjall LSM-tree storage engine performs background compaction to merge SSTables and remove obsolete versions. **Direct keyspace reads** (`partition.get(key)`) can block when compaction is rewriting the SSTable containing the requested key.

### Affected Files and Locations

All Fjall-backed stores use direct keyspace reads that can block on compaction:

1. **`crates/vo-storage/src/dedupe_partition/fjall_dedupe.rs`**
   - Line 73: `self.partition.get(&encoded_key)` in `check_and_insert()`
   - Line 146: `self.partition.get(&encoded_key)` in `contains()`

2. **`crates/vo-storage/src/effect_journal/fjall_journal.rs`**
   - Line 205: `self.partition.get(key)` in `get_impl()` (used by commit, rollback)
   - Line 50: `self.partition.get(&key)` in `prepare()`

3. **`crates/vo-storage/src/workflow_version_partition/fjall_store.rs`**
   - Line 41: `self.partition.get(&key)` in `get()`
   - Line 65: `self.partition.get(&key)` in `contains()`
   - Line 76: `self.partition.get(&key)` in `delete()`

4. **`crates/vo-storage/src/key_partition/fjall_dek_store.rs`**
   - Line 51: `self.dek_partition.get(&key)`
   - Line 68: `self.index_partition.get(&key)`
   - Line 120: `self.dek_partition.get(&key)`

5. **`crates/vo-storage/src/receipts/fjall_receipt_store.rs`**
   - Line 51: `self.partition.get(&key)` in `insert_if_new()`
   - Line 69: `self.partition.get(&key)` in `get()`
   - Line 87: `self.partition.get(&key)` in `contains()`

## Fjall Read Consistency Architecture

Based on Fjall V3.1.4 API (`references/api-surface.md`):

### Snapshot-Based Reads (Non-Blocking)

```rust
// Create a snapshot for MVCC-consistent reads
let snapshot = db.snapshot();

// Read from snapshot - never blocks on compaction
let value = snapshot.get(&partition, &key)?;
```

- Snapshots provide **point-in-time MVCC-consistent views**
- Snapshots pin a `SeqNo`, preventing GC of visible versions
- Reads from snapshots **never block on compaction** since they see a frozen view

### Direct Keyspace Reads (Can Block)

```rust
// Direct read - may block if target SSTable is being compacted
let value = partition.get(&key)?;
```

- Direct reads go to the active memtable and SSTables
- If the target SSTable is being compacted, the read must wait for compaction to complete

## Fix Strategy

Replace direct keyspace reads with snapshot-based reads:

### Pattern 1: Cached Snapshot Per Request

```rust
impl FjallDedupeStore {
    fn get_with_snapshot(&self, key: &[u8]) -> Result<Option<Vec<u8>>, DedupeStoreError> {
        let snapshot = self.db.snapshot();
        snapshot.get(&self.partition, key)
            .map_err(|e| DedupeStoreError::Storage { reason: e.to_string() })
    }
}
```

### Pattern 2: Shared Long-Lived Snapshot

For read-heavy workloads, share a snapshot across multiple reads:

```rust
pub struct FjallDedupeStore {
    db: Arc<fjall::Database>,
    partition: Arc<fjall::Keyspace>,
    snapshot: Arc<Snapshot>,  // Shared, periodically refreshed
    stripes: Vec<Mutex<()>>,
}
```

### Pattern 3: Snapshot Per Read Operation

Simple but higher overhead per operation:

```rust
fn contains(&self, key: &DedupeKey) -> Result<bool, DedupeStoreError> {
    let snapshot = self.db.snapshot();  // Create fresh snapshot per read
    let encoded_key = super::encode_dedupe_key(key);
    match snapshot.get(&self.partition, &encoded_key) {
        Ok(Some(value_bytes)) => ...,
        Ok(None) => Ok(false),
        Err(e) => Err(...),
    }
}
```

## Benchmark Requirements

The issue requests: **concurrent read/write benchmark** measuring p99 read latency with and without compaction:
- One writer thread appending 10K events
- One reader thread querying by prefix
- Measure p99 read latency during active compaction vs. idle

## Current Codebase Status

**The vo-storage crate currently has compilation errors:**
- Missing module `atomic_wait_commit`
- Missing module `event_summary_commit`
- Unresolved import `zeroize`
- Private associated function `error` in `event_log.rs`

This appears to be a work-in-progress branch with incomplete code.

## Recommendations

1. **Immediate**: Use `db.snapshot()` for all read operations in Fjall stores
2. **Short-term**: Add snapshot refresh interval for long-lived snapshots
3. **Testing**: Add concurrent read/write benchmark to verify non-blocking behavior
4. **Monitoring**: Add `active_compactions()` and `time_compacting()` diagnostics

## Related Fjall APIs

From `references/api-surface.md`:
- `db.snapshot()` - Create MVCC snapshot
- `Snapshot::get(&self, keyspace, key)` - Non-blocking read
- `db.active_compactions()` - Hidden diagnostic
- `db.time_compacting()` - Hidden diagnostic
