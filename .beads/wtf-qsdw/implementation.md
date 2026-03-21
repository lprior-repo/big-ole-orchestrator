# Implementation Summary: Per-Activity Timeout Support

## Changes Made

### 1. `crates/wtf-worker/src/queue.rs`
- Added `timeout_ms: Option<u64>` field to `ActivityTask` struct
- Updated `make_task` test helper to include `timeout_ms: None`

### 2. `crates/wtf-worker/src/worker.rs`
- Added `tokio::time::{Duration, timeout}` imports
- Modified `process_task` to wrap handler execution in `tokio::time::timeout` when `timeout_ms` is `Some`
- On timeout: calls `fail_activity` with error "Activity timeout elapsed" and appropriate `retries_exhausted` flag
- Task is acked after timeout failure is recorded

## Key Design Decisions

1. **Timeout as milliseconds**: `timeout_ms: Option<u64>` aligns with existing `RetryPolicy` pattern using millisecond intervals
2. **No timeout extension**: Once a task starts, its timeout is fixed (no renewal mid-execution)
3. **Timeout is permanent failure**: Timed-out activities are not retried automatically; the retry decision follows the existing `retries_exhausted` logic
4. **tokio::time::timeout**: Used over `tokio::time::sleep` + race to cleanly cancel the future

## Behavior Summary

| `timeout_ms` | Behavior |
|---|---|
| `None` | No timeout enforcement, runs to completion |
| `Some(ms)` | Fails with "Activity timeout elapsed" if handler exceeds `ms` milliseconds |
