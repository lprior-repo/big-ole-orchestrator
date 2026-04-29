# ADR-003 Subprocess Spawn Audit — Findings

## Audit Scope
All `tokio::process::Command` / `std::process::Command` usage across the veloxide codebase, verified against ADR-003, ADR-012, and ADR-014.

## Production Subprocess Spawn Sites

### 1. `vo-executor/src/subprocess.rs:114-199` — **COMPLIANT**
- Uses `tokio::process::Command`
- FD3/FD4 via `libc::pipe2(O_CLOEXEC)` + `dup2` in `pre_exec`
- `FD_CLOEXEC` set on both FD3 and FD4 after dup2 (lines 139-144)
- `PR_SET_PDEATHSIG(SIGTERM)` configured (line 127)
- `setpgid(0,0)` for process group isolation (line 130)
- `stdout` = `Stdio::null()`, `stderr` = `Stdio::null()`, `stdin` = `Stdio::null()`
- **No stdout state reads** — all state via FD3/FD4 with 4-byte BE length-prefix framing
- Timeout enforcement via `tokio::time::timeout`
- Async concurrent IPC (ADR-018 compliant) with `tokio::join!` for write/read
- Bounded buffer (64KB) for FD4 reads
- 10MB payload size limit on FD4 reads

### 2. `vo-ipc/src/run.rs:26-255` — **VIOLATION: Missing FD_CLOEXEC on dup2'd FD3/FD4**
- Uses `tokio::process::Command`
- FD3/FD4 via `libc::pipe2(O_CLOEXEC)` + `dup2` in `pre_exec`
- `PR_SET_PDEATHSIG(SIGTERM)` configured (line 39)
- `setpgid(0,0)` for process group isolation (line 42)
- `stdout` = `Stdio::null()`, `stderr` = `Stdio::piped()` (for observability)
- **No stdout state reads** — all state via FD3/FD4
- **BUG**: After `dup2(fd3_read, 3)` and `dup2(fd4_write, 4)`, the code does NOT call `fcntl(3, F_SETFD, FD_CLOEXEC)` or `fcntl(4, F_SETFD, FD_CLOEXEC)`. Per ADR-014: "if the user's task binary uses `std::process::Command` to spawn a subprocess, the OS will automatically close the pipes for the subprocess." The `pipe2(O_CLOEXEC)` only sets the flag on the *original* pipe FDs. After `dup2`, the new FDs (3 and 4) inherit flags from the target FD number, not the source. Since FD 3 and 4 were not previously open with CLOEXEC, the dup2'd FDs may NOT have CLOEXEC set. This means if the child binary spawns a subprocess, FDs 3 and 4 could leak to grandchildren, causing IPC hangs.
- **Contrast**: `vo-executor/src/subprocess.rs` correctly calls `fcntl(3, F_SETFD, FD_CLOEXEC)` and `fcntl(4, F_SETFD, FD_CLOEXEC)` after dup2 (lines 139-144). `vo-ipc/src/bus.rs` also does this correctly (lines 92-97).

### 3. `vo-ipc/src/bus.rs:67-130` — **COMPLIANT**
- Uses `tokio::process::Command`
- FD3/FD4 via `libc::pipe2(O_CLOEXEC)` + `dup2` in `pre_exec`
- `FD_CLOEXEC` set on both FD3 and FD4 after dup2 (lines 92-97)
- `setpgid(0,0)` for process group isolation (line 83)
- `stdout` = `Stdio::null()`, `stderr` = `Stdio::piped()` (for observability)
- **No stdout state reads** — all state via FD3/FD4
- NOTE: Missing `PR_SET_PDEATHSIG` — only has `setpgid`. This is a minor gap vs ADR-012 which requires both.

### 4. `vo-actor/src/probe/exec_probe.rs:41-52` — **COMPLIANT (different purpose)**
- Uses `tokio::process::Command` with `.output()` (not `.spawn()`)
- This is a health-check probe, NOT a workflow subprocess execution
- stdout/stderr are captured for status checks, not for workflow state
- Does NOT use FD3/FD4 — correctly, because this is not a workflow binary
- No ADR-003/012/014 requirements apply here

## Test/Fixture Subprocess Spawns (not production code, exempt from ADR)

- `vo-executor/tests/fixtures/src/bin/test_subprocess_helper.rs` — test helper, spawns self for fork tests
- `vo-ipc/tests/fixtures/src/bin/fixture_driver.rs` — test fixture, spawns self for IPC tests
- `vo-sdk/tests/integration.rs` — spawns `cargo run` to test SDK binary
- `vo-types/tests/scaffold_compliance.rs` — spawns `cargo check`/`cargo clippy`
- `vo-storage/tests/redqueen_dolt.rs` — spawns `dolt` CLI for DB tests
- `vo-storage/tests/blackhat_dolt.rs` — spawns `bd` CLI for security tests

All test code correctly uses stdout for its own purposes (test verification, not workflow state).

## Summary of Findings

| Site | File | ADR-003 Compliant | ADR-012 Compliant | ADR-014 Compliant | Issues |
|------|------|-------------------|-------------------|-------------------|--------|
| Engine subprocess | `vo-executor/subprocess.rs` | YES | YES | YES | None |
| IPC run_subprocess | `vo-ipc/run.rs` | YES | PARTIAL | **NO** | Missing FD_CLOEXEC after dup2 |
| IPC MessageBus | `vo-ipc/bus.rs` | YES | PARTIAL | YES | Missing PR_SET_PDEATHSIG |
| Health probe | `vo-actor/probe/exec_probe.rs` | N/A | N/A | N/A | Different purpose |

## Required Fixes

### P0: `vo-ipc/src/run.rs` — Add FD_CLOEXEC after dup2
After lines 48-49 (`dup2(fd3_read, 3)` and `dup2(fd4_write, 4)`), add:
```rust
if libc::fcntl(3, libc::F_SETFD, libc::FD_CLOEXEC) == -1 {
    return Err(std::io::Error::last_os_error());
}
if libc::fcntl(4, libc::F_SETFD, libc::FD_CLOEXEC) == -1 {
    return Err(std::io::Error::last_os_error());
}
```
This matches the pattern already used in `vo-executor/src/subprocess.rs:139-144` and `vo-ipc/src/bus.rs:92-97`.

### P1: `vo-ipc/src/bus.rs` — Add PR_SET_PDEATHSIG
The `bus.rs` MessageBus spawn is missing `PR_SET_PDEATHSIG(SIGTERM)`. If the engine crashes, bus-spawned children won't auto-terminate. Add in `pre_exec`:
```rust
if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM, 0, 0, 0) != 0 {
    return Err(std::io::Error::last_os_error());
}
```
This matches the pattern in `vo-executor/src/subprocess.rs:127` and `vo-ipc/src/run.rs:39`.

## Conclusion
The codebase is **mostly compliant** with ADR-003/012/014. Two gaps found:
1. **P0**: `vo-ipc/src/run.rs` missing `FD_CLOEXEC` after `dup2` — potential FD leak to grandchildren causing IPC hangs
2. **P1**: `vo-ipc/src/bus.rs` missing `PR_SET_PDEATHSIG` — children survive engine crash as orphans

No stdout-based state reads found anywhere in production code. All workflow subprocess communication correctly uses FD3/FD4 with length-prefix framing.
