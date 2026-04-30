# ADR 054 (v2): File Descriptor Leak Detection and Recovery

## Status
Proposed

## Context

ADR-014 established the dual-pipe IPC architecture using FD3/FD4 with `O_CLOEXEC` to prevent inherited FD hangs in subprocesses. However, `O_CLOEXEC` only prevents FDs from being inherited by future child processes—it does not prevent leaks within the Engine process itself.

**Leak Scenarios Identified:**

1. **Spawn Failure Leak**: In `vo-ipc/src/subprocess.rs:53-54`, if `child.spawn()` fails after pipe creation, `fd3_read` and `fd4_write` are dropped without closing:
   ```rust
   let _ = fd3_read;
   let _ = fd4_write;
   ```
   `let _ = x` in Rust drops the value, but since these are raw file descriptors (not `OwnedFd`), they are not automatically closed on drop. This is a latent FD leak.

2. **Panic Recovery Gap**: If the Engine panics during subprocess execution, open FDs may not be cleaned up before the process restarts. The Reanimator Loop (ADR-005) handles timer-based resumption, but there is no mechanism to audit or clean up leaked FDs after a crash.

3. **Unbounded Resource Growth**: Without FD counting, the Engine cannot detect or alert on FD exhaustion before it causes failures.

## Decision

We implement a three-layer FD hygiene system:

### 1. OwnedFd Wrapper (Prevention)

Replace all raw FD usage in `vo-ipc` and `vo-executor` with an `OwnedFd` wrapper that closes the FD on drop:

```rust
struct OwnedFd(RawFd);

impl Drop for OwnedFd {
    fn drop(&mut self) {
        if self.0 >= 0 {
            unsafe { libc::close(self.0) };
        }
    }
}
```

**Scope**: All pipe creation sites in `vo-ipc/src/subprocess.rs` and `vo-executor/src/subprocess.rs`.

### 2. Spawn-Guard Cleanup (Prevention)

In `vo-ipc/src/subprocess.rs`, wrap pipe creation in a scope guard:

```rust
let (fd3_read, fd3_write) = create_pipe()?;
let _guard = ScopeGuard::new(|| {
    // On any early return before child.spawn succeeds,
    // close the FDs that were created
});
```

Alternative: Use `std::mem::ManuallyDrop` and explicit `libc::close` in error paths before returning.

### 3. FD Audit Service (Detection)

Add a background task that periodically:
- Reads `/proc/self/fd` to enumerate open FDs
- Logs the count with structured metadata (FD number, path if available)
- Emits a metric `engine_fd_count` with labels for `fd_type` (pipe, socket, file)
- Alerts if count exceeds a configured threshold (default: 80% of `RLIMIT_NOFILE`)

### 4. Leak Recovery Protocol (Recovery)

When the audit detects an anomaly:

1. **Log the stack trace** of all open FDs at WARNING level
2. **Attempt self-healing**: If the leak is traced to a known pipe, close it
3. **Escalate**: If self-healing fails or leak is unknown, signal the Engine to initiate a graceful restart via the watchdog
4. **Preserve evidence**: Before restart, write leak report to `var/wtf/crash-reports/fd-leak-<timestamp>.json`

### 5. Panic Hook Integration

Register a panic hook that:
- Captures `/proc/self/fd` snapshot at panic time
- Writes to `var/wtf/crash-reports/fd-snapshot-<pid>-<timestamp>.json`
- Ensures FDs are not inherited by the recovery process

## Consequences

- **Positive**: FD leaks in spawn paths are impossible by construction with `OwnedFd`
- **Positive**: Panics produce artifacts for post-mortem analysis
- **Positive**: Early warning via metrics before exhaustion occurs
- **Negative**: Small overhead from periodic `/proc` scans (negligible—once per second)
- **Negative**: Crash recovery now requires FD audit, adding coupling between vo-actor and crash reporting

## References

- ADR-005: Hibernation and Timers (Reanimator Loop)
- ADR-014: Secure IPC and FD Management
- ADR-012: Execution Boundary Hardening
- Linux `/proc/self/fd` interface for FD enumeration
