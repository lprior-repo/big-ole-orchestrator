# DLQ Implementation Findings - Bead tw-k9g1

## Summary

Implemented a Dead Letter Queue (DLQ) for failed events in vo-storage. The DLQ stores events that fail processing after all retries.

## Implementation Details

### 1. Storage Layer (vo-storage)

**New files created:**
- `crates/vo-storage/src/dlq_partition/mod.rs` - Trait definition, DlqEntry data structure, encoding/decoding functions
- `crates/vo-storage/src/dlq_partition/fjall_dlq.rs` - Fjall-backed production implementation
- `crates/vo-storage/src/dlq_partition/in_memory_dlq.rs` - In-memory implementation for testing

**Modified files:**
- `crates/vo-storage/src/lib.rs` - Added `dlq_partition` module export
- `crates/vo-storage/src/partitions.rs` - Added DLQ_PARTITION constant, added to ALL_PARTITIONS and HOT_PARTITIONS, added dlq_store field to StorageEngine

**DlqEntry structure:**
```rust
pub struct DlqEntry {
    pub instance_id: String,
    pub sequence: u64,
    pub event_payload: serde_json::Value,
    pub failure_reason: String,
    pub failure_count: u32,
    pub last_attempted_at: u64,
    pub original_timestamp_ms: u64,
}
```

**DlqStore trait operations:**
- `push()` - Add failed event to DLQ
- `list()` - List all DLQ entries with optional instance_id filter
- `get()` - Get specific entry by instance_id and sequence
- `remove()` - Remove entry (for manual replay consumption)
- `count()` - Get DLQ entry count
- `clear()` - Clear all DLQ entries

### 2. API Layer (vo-api)

**New files created:**
- `crates/vo-api/src/handlers/dlq.rs` - DLQ handlers for API endpoints

**Modified files:**
- `crates/vo-api/src/handlers/mod.rs` - Added dlq module export
- `crates/vo-api/src/router.rs` - Added dlq_store to AppState, added DLQ routes

**API endpoints:**
- `GET /api/v1/dlq` - List DLQ entries (optional `?instance_id=` filter)
- `GET /api/v1/dlq/count` - Get DLQ entry count

### 3. CLI (vo-cli)

**New files created:**
- `crates/vo-cli/src/commands/dlq.rs` - CLI command implementation

**Modified files:**
- `crates/vo-cli/src/cli.rs` - Added Dlq command variant, DlqAction enum, error handling
- `crates/vo-cli/src/commands/mod.rs` - Added dlq module
- `crates/vo-cli/src/registry.rs` - Added DlqHandler to registry

**CLI commands:**
- `vo dlq list --engine-url=<url> [--instance-id=<id>]` - List DLQ entries
- `vo dlq count --engine-url=<url>` - Get DLQ entry count

## Compilation Status

- **vo-storage**: Compiles successfully
- **vo-api**: Compiles successfully (warnings are pre-existing)
- **vo-cli**: Has **pre-existing compilation errors** unrelated to DLQ implementation

The vo-cli compilation errors are in `serve.rs` and relate to missing items in `vo_actor`:
- `vo_actor::WorkQueue` trait not found
- `vo_actor::TimerSupervisor` not found
- `vo_actor::TimerStorage` trait not found

These errors exist in the base codebase before this DLQ implementation and need to be fixed separately.

## Design Decisions

1. **Partition**: DLQ is classified as a HOT partition since failed events may need processing
2. **Key format**: Uses `[instance_id(16 bytes)][sequence(8 bytes)][separator]` for DLQ keys
3. **Value format**: JSON serialization for DlqEntry (following dedupe_partition pattern)
4. **Trait-based**: DlqStore trait allows for both Fjall (production) and in-memory (testing) implementations

## Testing

Basic unit tests added for:
- FjallDlqStore: push, list, get, remove, count, clear operations
- InMemoryDlqStore: push, list, get, remove operations
- CLI: DlqConfig, DlqError display, serialization

## Files Changed

```
M crates/vo-api/src/handlers/mod.rs
M crates/vo-api/src/router.rs
M crates/vo-cli/src/cli.rs
M crates/vo-cli/src/commands/mod.rs
M crates/vo-cli/src/commands/serve.rs
M crates/vo-cli/src/registry.rs
M crates/vo-storage/src/lib.rs
M crates/vo-storage/src/partitions.rs
A crates/vo-api/src/handlers/dlq.rs
A crates/vo-cli/src/commands/dlq.rs
A crates/vo-storage/src/dlq_partition/
```

## Next Steps

1. Fix pre-existing vo_cli compilation errors in serve.rs
2. Add replay functionality to move events back from DLQ to the event stream
3. Add API endpoint for replaying specific DLQ entries
4. Add CLI command for replay (`vo dlq replay <instance_id> <sequence>`)