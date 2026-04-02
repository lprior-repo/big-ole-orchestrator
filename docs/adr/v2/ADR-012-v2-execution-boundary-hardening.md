# ADR 012 (v2): Execution Boundary Hardening (Zombies, FD3/FD4, Memory Bombs)

## Status
Accepted

## Context
When an orchestrator spawns un-sandboxed OS child processes, the boundary between the Engine and the Child is the most dangerous surface in the system. The failure modes include:
1. **Zombies:** If the Engine crashes, child processes might survive as orphans, eventually consuming OS resources.
2. **IPC Corruption:** `stdout` is easily corrupted by random `println!` statements or third-party crate logging.
3. **Memory Bombs:** A child could send a 10GB payload or deeply nested JSON, instantly OOMing or stack-overflowing the Engine.
4. **Input Bombs:** A workflow instance could attempt to feed an enormous payload into a child and stall the pipe boundary before execution even begins.
5. **File Locking:** Executing binaries in place prevents `cargo build` hot-reloads and breaks version pinning.

## Decision
We enforce a strictly hardened OS boundary using the following mechanisms.

### 1. Process Grouping and Graceful Death
- **Linux:** The `vo-sdk` generated `main()` must call `prctl(PR_SET_PDEATHSIG, SIGTERM)` as its first instruction. This ensures children receive a kill signal if the parent Engine dies.
- **Graceful Exit:** We use `SIGTERM` first to allow the SDK to flush local state and exit cleanly. If a child hangs, the Engine escalates and sweeps leftovers on startup.

### 2. IPC via Dedicated File Descriptors
- **FD 3:** Engine -> Task payload and execution metadata.
- **FD 4:** Task -> Engine output envelope.
- **FD 1 and FD 2 (`stdout`/`stderr`):** Reserved for user logging and captured separately.
- Both FD3 and FD4 use explicit framing with a 4-byte big-endian length prefix.

### 3. Memory Bomb Protection
- The Engine enforces strict `MAX_STEP_INPUT_BYTES` and `MAX_STEP_OUTPUT_BYTES` limits.
- If the input payload exceeds the configured cap, the step fails before spawn.
- If the child attempts to write more than the output cap, the Engine closes the read side and marks the step as failed.
- All reads and writes use bounded buffers.

### 4. Binary Versioning (Content-Hash Copy)
- The Engine never executes a binary directly from the user's target directory.
- Upon discovery, the Engine hashes the binary and copies it to `/var/wtf/versions/<sha256>/binary_name`.
- Active workflows pin to that hash. This solves hot-reload file-lock issues and guarantees version stability for long-running instances.

## Consequences
- **Positive:** Unbreakable IPC. Developers can use `stdout/stderr` freely without breaking the Engine.
- **Positive:** System stability. Zombies, input bombs, and output bombs are blocked at the boundary.
- **Positive:** Hot reloading works alongside long-running instance pinning.
- **Negative:** Slightly more complex SDK internals are required to handle FD3/FD4 and length-prefixed protocols.
