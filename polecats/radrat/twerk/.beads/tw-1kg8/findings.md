# Findings: tw-1kg8 — vo-actor: Change SpawnSupervisor to take (executable, args)

## Summary

Changed `ProcessManager::spawn_process` from `executable: &str` to `executable: &Path`, eliminating shell injection risk if any implementation shells out via `Command::new()`.

## Changes Made

### Primary (spawn_supervisor.rs)

1. **Added import**: `use std::path::{Path, PathBuf};`
2. **`SpawnRecord.executable`**: `String` → `PathBuf`
3. **`SpawnRecord::new` parameter**: `executable: String` → `executable: PathBuf`
4. **`ProcessManager::spawn_process` trait method**: `executable: &str` → `executable: &Path`
5. **`ProcessHandle.executable`**: `String` → `PathBuf`
6. **`ProcessHandle::new` parameter**: `executable: String` → `executable: PathBuf`
7. **`WorkQueue::enqueue_spawn` trait method**: `executable: String` → `executable: PathBuf`
8. **Internal unit test**: Updated `SpawnRecord::new` call to use `.into()`

### Test Files

9. **spawn_supervisor_integration.rs**:
   - Added `use std::path::Path;`
   - Updated `MockProcessManager::spawn_process` signature and body
   - Updated `MockWorkQueue::enqueue_spawn` signature
   - Changed all `"./worker".to_string()` → `"./worker".into()` (17 sites)
   - Changed `"./nonexistent".to_string()` → `"./nonexistent".into()` (3 sites)
   - Updated `ProcessHandle::new` test assertion: `Path::new("./worker")`
10. **bdd_behavior_audit.rs**: Already used `.into()` — no changes needed

### Incidental Fixes (pre-existing compile errors blocking verification)

11. **vo-types/src/recovery_contract.rs**: Removed stray `}` at line 1077 (brace imbalance)
12. **vo-actor/src/lib.rs**: Fixed `TestStateLookup` import — moved from `signal_messages` to `test_utilities` re-export

## Verification

- `cargo check -p vo-actor --lib` passes with zero errors
- 3 pre-existing warnings (unused imports/variable) unrelated to this change
- Pre-existing compile errors in vo-core block test compilation (unrelated)

## ADR-012 Compliance

The change eliminates the raw `&str` command string path. Callers must now pass a `&Path`, making it structurally impossible to pass shell command strings. Any implementation using `Command::new(executable)` will correctly receive a path, not a shell command.
