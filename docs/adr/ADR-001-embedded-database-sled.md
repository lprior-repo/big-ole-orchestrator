# ADR-001: Use sled as Embedded Database

## Status

Amended by ADR-013, ADR-014, ADR-019

sled is no longer the primary source of truth. NATS JetStream (ADR-013) is the immutable event log. NATS KV (ADR-014) is the materialized view. sled is retained exclusively as the **local snapshot store** (ADR-019): it caches derived state at known event sequence numbers so recovery can replay only the tail of the log rather than from seq=1. The original rationale for sled (single binary, no external process) still holds for the snapshot role.

## Context

wtf-engine requires a durable storage layer for:
- **Append-only journal** (every workflow step produces a journal entry)
- **Column families** (instances, journal, timers, signals, run_queue)
- **Async support** (Tokio runtime integration)
- **Single binary deployment** (embedded, no separate DB server)
- **Write-heavy workload** (high-frequency appends)
- **Low-latency reads** (journal replay must be fast)

### Candidate Databases Evaluated

| Database | Async | Column Families | Write Throughput | Rust Native | Maturity |
|----------|-------|----------------|-----------------|-------------|----------|
| RocksDB | Excellent | Native | Excellent | Excellent | Very High |
| **sled** | **Native** | **Trees** | **Good** | **Yes** | Medium |
| redb | None | No | Moderate | Yes | Low |
| DuckDB | None | N/A (SQL) | Good | Yes | High |
| Badger | None | No | Excellent | No | High |
| LMDB | None | No | Excellent | Unmaintained | Very High |

### Why Not RocksDB

- **C++ FFI** - introduces complexity, harder to audit
- **Async wrapper** - `tokio-rocksdb` still uses `spawn_blocking` internally
- **Complexity** - LSM tree tuning, compaction management

### Why Not Others

- **redb**: No async support, no column families
- **DuckDB**: OLAP database, wrong workload (read-heavy vs write-heavy)
- **Badger**: Go only, no Rust bindings
- **LMDB**: Single writer bottleneck, unmaintained Rust bindings

## Decision

We will use **sled** as the embedded database for wtf-engine.

### Why sled Wins

1. **Native async** - `flush_async()`, `watch_prefix()` return Futures
2. **Trees = Column Families** - `db.open_tree(b"journal")`, `db.open_tree(b"timers")`
3. **Multi-tree transactions** - atomic operations across trees
4. **Pure Rust** - no FFI, easier to audit
5. **Restate compatibility** - Restate uses sled in some configurations
6. **Modern design** - B+tree with lock-free optimizations

### sled Limitations Accepted

- **Pre-1.0** - disk format may change, but acceptable for v1
- **Lower write throughput than RocksDB** - acceptable for our scale
- **Less tuning options** - acceptable for simplicity

## Consequences

### Positive

- Native async integration with Tokio
- Simple Tree API maps cleanly to our schema
- Pure Rust = easier cross-compilation
- No separate database process = single binary deployment
- Transaction support for atomic journal appends

### Negative

- Pre-1.0 stability risk (monitor closely)
- Lower write throughput than RocksDB
- Less operational tooling

### Mitigations

- Monitor sled stability in production
- Implement write batching for journal appends
- Have RocksDB migration path if needed

## Implementation

```rust
// Tree names as byte vectors
const INSTANCES: &[u8] = b"instances";
const JOURNAL: &[u8] = b"journal";
const TIMERS: &[u8] = b"timers";
const SIGNALS: &[u8] = b"signals";
const RUN_QUEUE: &[u8] = b"run_queue";
const ACTIVITIES: &[u8] = b"activities";
const WORKFLOWS: &[u8] = b"workflows";

// Open database
let db = sled::open("wtf-engine.db")?;

// Open column families (Trees)
let instances = db.open_tree(INSTANCES)?;
let journal = db.open_tree(JOURNAL)?;
let timers = db.open_tree(TIMERS)?;
let signals = db.open_tree(SIGNALS)?;

// Multi-tree transaction for atomic operations
(&instances, &journal).transaction(|(tx_inst, tx_journal)| {
    tx_inst.insert(invocation_id.as_bytes(), instance_data)?;
    tx_journal.insert(key, journal_entry)?;
    Ok(())
})?;
```
