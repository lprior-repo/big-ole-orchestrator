# Bead tw-h7w0: Add fjall partition management CLI

## Summary
Implemented `vo-cli partitions` command with three subcommands:
- `vo-cli partitions list` - List all partitions with key count and disk size
- `vo-cli partitions compact` - Run major compaction on partitions
- `vo-cli partitions stats` - Show detailed partition statistics

## Files Changed

### New Files
- `crates/vo-cli/src/commands/partitions.rs` - New partitions command module

### Modified Files
- `crates/vo-cli/src/cli.rs` - Added `Partitions` command variant, `PartitionsSubcommand` enum, CLI parsing, and error handling
- `crates/vo-cli/src/commands/mod.rs` - Added `partitions` module
- `crates/vo-cli/src/registry.rs` - Added `PartitionsHandler` and fixed pre-existing bugs in `execute_with_graph`

## Implementation Details

### CLI Structure
```
vo-cli partitions [OPTIONS] [COMMAND]

Commands:
  list     List all partitions with key count and size
  compact  Run major compaction on partitions
  stats    Show detailed partition statistics
```

### Partitions List Output
Shows: NAME, CLASS (hot/cold/blob), KEYS, DISK_SIZE, TABLES

### Partitions Stats Output
Shows: NAME, KEYS, DISK_SIZE, L0_TABLES, TOTAL_TABLES, BLOB_FILES
With `--json` flag outputs machine-readable JSON

### Partitions Compact
- Without `--partition` flag: compacts all partitions
- With `--partition <name>`: compacts only the specified partition

## Pre-existing Bug Fixed
Also fixed pre-existing bugs in `execute_with_graph` function in `registry.rs`:
- Added `mut` to `stdout` and `stderr` variable declarations
- Fixed nested `Result` handling from `tokio::try_join!`

## Verification
- Build passes: `cargo build --package vo-cli`
- Manual testing confirms all three subcommands work:
  - `vo partitions list` - lists all 13 partitions
  - `vo partitions stats` - shows detailed stats with human and JSON output
  - `vo partitions compact` - compacts all or single partition

## Notes
- Uses fjall Keyspace API: `len()`, `disk_space()`, `table_count()`, `l0_table_count()`, `blob_file_count()`, `major_compact()`
- Storage path defaults to `.vo/storage`, configurable via `--storage-path` flag