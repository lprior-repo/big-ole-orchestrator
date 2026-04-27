# tw-ytzj: vo-executor Pipe stderr instead of discarding

## Problem
`vo-executor/src/subprocess.rs:123` set stderr to `Stdio::null()`, discarding all diagnostic data from subprocess execution. This made debugging workflow failures impossible since subprocess errors were silently swallowed.

## Root Cause
The `vo-executor` crate had its own subprocess implementation that was never updated when `vo-ipc` got proper stderr capture. The `vo-ipc` crate already has full stderr support (`stderr_bytes`, `stderr_truncated`, `StderrCapture` with 1MB bounded capture and truncation marker).

## Fix Applied
Changed `vo-executor/src/subprocess.rs`:

1. **`SubprocessOutput` struct**: Added `stderr_bytes: Vec<u8>` and `stderr_truncated: bool` fields
2. **`run_subprocess`**: Changed `stderr(Stdio::null())` to `stderr(Stdio::piped())`
3. **`run_subprocess`**: Added concurrent stderr capture via `tokio::spawn(read_bounded_stderr(...))` running alongside the IPC timeout
4. **New function `read_bounded_stderr`**: Generic async reader that captures stderr up to 1MB with truncation marker, matching `vo-ipc/src/stderr.rs` behavior

## Constants
- `MAX_STDERR_BYTES = 1_048_576` (1MB, matches vo-ipc)
- `STDERR_TRUNCATION_MARKER = b"\n[... TRUNCATED AT 1MB ...]"` (matches vo-ipc)

## Tests Added
- `test_read_bounded_stderr_small_input` - normal capture
- `test_read_bounded_stderr_empty` - empty stderr
- `test_read_bounded_stderr_truncation` - truncation at 1MB boundary
- `test_subprocess_output_has_stderr_fields` - struct field verification

## Note
The `vo-executor` subprocess module is a parallel implementation to `vo-ipc/src/run.rs`. A future refactor should consider deduplicating these, having `vo-executor` delegate to `vo-ipc` for subprocess execution.
