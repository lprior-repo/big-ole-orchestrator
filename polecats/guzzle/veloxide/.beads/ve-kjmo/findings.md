# Findings: ve-kjmo - QA: vo-api 3 test files broken

## Summary
Fixed compilation errors in vo-storage and vo-types that were blocking vo-api tests.

## Issues Fixed

### 1. vo-storage: DedupeEntry instance_id type mismatch
**Files:**
- `crates/vo-storage/src/dedupe_partition/fjall_dedupe.rs:77`
- `crates/vo-storage/src/dedupe_partition/in_memory_dedupe.rs:53`

**Problem:** `AdmissionResult::Duplicate { instance_id: InstanceId }` expects `InstanceId`, but code was passing `entry.instance_id().to_string()` which returns `String`.

**Fix:** Changed to `InstanceId::parse(entry.instance_id()).expect("valid instance_id in dedupe entry")`

### 2. vo-storage: FenceToken::new_unchecked removed
**Files:**
- `crates/vo-storage/src/lease_partition/fjall_lease_store.rs:167`
- `crates/vo-storage/src/lease_partition/in_memory_lease.rs:93`

**Problem:** Code used `FenceToken::new_unchecked()` which no longer exists. Only `FenceToken::new()` and `FenceToken::parse()` are available.

**Fix:** Changed to `FenceToken::new(fence_token).expect("valid fence token")` since fence tokens are guaranteed non-zero.

### 3. vo-types: Missing search module export
**File:** `crates/vo-types/src/lib.rs`

**Problem:** `vo_api` imported `vo_types::search::{QueryParser, SearchEngine, ...}` but the `search` module was not exported from `vo_types::lib.rs`.

**Fix:** Added `pub mod search;` to `vo-types/src/lib.rs`

## Verification
- `cargo build -p vo-storage` ✓
- `cargo build -p vo-actor` ✓
- `cargo build -p vo-api` ✓
- `cargo test -p vo-api --test bdd_sse_ws_streaming_tests` - 24 tests pass
- `cargo test -p vo-api --test ingress_admission_tests` - 12 tests pass

## Note on Test Files
The bead mentioned `ingress_error_tests.rs` and `ingress_happy_tests.rs` as broken test files, but these files do not exist in the main `vo-api` test directory. Only `ingress_admission_tests.rs` exists and it passes.
