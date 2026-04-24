# ve-zcak Findings

## Summary
Bead referenced `GitHubClientImpl` which does not exist in the codebase. The actual bug per ADR-011 is that tokio runtimes were being created on every API call in `vo-sdk/src/runtime.rs` and `vo-executor/src/runtime.rs`.

## Bugs Found

### 1. vo-executor/src/runtime.rs — Use-after-free (Critical)
`Runtime::new()` created a `tokio::runtime::Runtime`, extracted only the `Handle`, and dropped the `Runtime`. The `Handle` becomes invalid once the owning `Runtime` is dropped, making all subsequent `block_on` calls undefined behavior.

### 2. vo-sdk/src/runtime.rs — Redundant runtime creation per call
`start()`, `spawn_and_wait()`, and `in_current_thread()` each created a new `tokio::runtime::Runtime`. A `LazyLock<Runtime>` ensures exactly one runtime is created and reused across all calls.

## Fix Applied

### vo-executor/src/runtime.rs
- Replaced per-instance `Handle` storage with a process-wide `LazyLock<TokioRuntime>`
- `Runtime` is now a zero-sized `Copy` struct that delegates to the shared runtime
- Removed the now-unnecessary `RuntimeError` variant (runtime creation cannot fail with LazyLock unless at process startup)

### vo-sdk/src/runtime.rs
- Added `static RUNTIME: LazyLock<tokio::runtime::Runtime>` initialized once
- All public functions (`start`, `spawn_and_wait`, `in_current_thread`) now use the shared runtime
- `current_thread_runtime()` return type changed to `Result<&'static Runtime, StartError>` (always returns Ok since LazyLock handles init)

## Build Status
Pre-existing compilation errors in vo-sdk (12 errors in dag.rs, unrelated). Changes do not introduce new errors.

## Not Changed (Intentionally)
- `vo-cli/src/main.rs`: Creates one multi-threaded runtime in `main()` — correct for a long-running CLI process
- `vo-sdk-macros/src/task.rs`: Generates runtime creation in subprocess `main()` — correct per ADR-011 (one runtime per subprocess binary)
- Test files: Runtime creation in tests is acceptable (isolated test environments)
