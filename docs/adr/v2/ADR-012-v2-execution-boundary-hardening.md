# ADR 012 (v2): Execution Boundary Hardening (Zombies, FD3, Memory Bombs)

## Status
Accepted

## Context
When an orchestrator spawns un-sandboxed OS child processes, the boundary between the Engine and the Child is the most dangerous surface in the system. The "Black Hat" failure modes include:
1. **Zombies:** If the Engine crashes, child processes might survive as orphans, eventually consuming all OS resources.
2. **IPC Corruption:** `stdout` is easily corrupted by random `println!` statements or third-party crate logging.
3. **Memory Bombs:** A child could return a 10GB JSON string or deeply nested JSON, instantly OOMing or stack-overflowing the Engine.
4. **File Locking:** Executing binaries in place prevents `cargo build` hot-reloads and breaks version pinning.

## Decision
We enforce a strictly hardened OS boundary using the following mechanisms.

### 1. Process Grouping and Graceful Death
- **Linux:** The `vo-sdk` generated `main()` must call `prctl(PR_SET_PDEATHSIG, SIGTERM)` as its first instruction. This ensures children receive a kill signal if the parent engine dies.
- **Graceful Exit:** We use `SIGTERM` (not `SIGKILL`) to allow the child to flush local state, catch the signal, and exit cleanly. If a child hangs, a sweeping `SIGKILL` on Engine startup will clear leftover binaries based on process path.

### 2. IPC via File Descriptor 3 (FD3)
- `stdout` is strictly reserved for user logging.
- The Engine maps a new pipe to **FD 3** before spawning the child.
- The `vo-sdk` writes the state payload exclusively to FD 3, formatted with a **4-byte big-endian length prefix**.
- The Engine reads exactly that many bytes, preventing ambiguous boundaries.

### 3. Memory Bomb Protection
- The Engine enforces a strict `MAX_STEP_OUTPUT_BYTES` limit (default 5MB) on the FD 3 read.
- If the child attempts to write more than the limit, the Engine closes the pipe and marks the step as failed.
- The Engine uses a bounded buffer for reads, preventing memory exhaustion attacks.

### 4. Binary Versioning (Content-Hash Copy)
- The Engine never executes a binary directly from the user's target directory.
- Upon discovery, the Engine hashes the binary and copies it to `/var/wtf/versions/<sha256>/binary_name`.
- Active workflows pin to that hash. This solves the Windows file-lock problem for hot-reloads and guarantees version stability for long-running workflows.

## Consequences
- **Positive:** Unbreakable IPC. Developers can use `stdout/stderr` freely without breaking the Engine.
- **Positive:** System stability. Zombies are eradicated and OOM bombs are blocked.
- **Positive:** Hot reloading works seamlessly alongside long-running instance pinning.
- **Negative:** Slightly more complex SDK internals to handle FD3 and length-prefixed protocols (abstracted from the user).