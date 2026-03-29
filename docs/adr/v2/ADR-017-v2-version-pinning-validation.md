# ADR 017 (v2): Binary Version Pinning and Architecture Validation

## Status
Accepted

## Context
When an Engine relies on raw compiled binaries, two massive DevEx footguns emerge:
1. **Semantic Version Confusion:** A developer changes the internal logic of a task (e.g., changes how it calculates tax) and recompiles. They expect in-flight workflows to pick up the new logic, but if the Engine blindly references the new binary, mid-flight workflows could crash due to shape changes in the expected payload.
2. **Build Target Mismatch:** A developer compiles the binary on an M-series Mac (`aarch64-apple-darwin`) and deploys it to the production Linux server (`x86_64-unknown-linux-gnu`). The binary fails to spawn, throwing an obscure OS `exec` error, and the Engine retry-loops infinitely.

## Decision

### 1. Explicit Version Pinning via Content Hash
The Engine enforces strict version pinning for all workflow executions to guarantee determinism.
- When the Engine discovers a new binary, it computes its SHA-256 hash.
- The binary is copied to an immutable path: `/var/wtf/versions/<hash>/binary_name`.
- When a workflow instance starts, it is permanently pinned to that `<hash>`. 
- If a developer updates the binary, it receives a new hash. New workflow instances use the new hash. In-flight instances finish their execution on the old hash.
- The UI and CLI must explicitly expose this hash (`vo-cli status <id>` shows the pinned version) so the developer understands *why* their in-flight workflow is using older logic.

### 2. Startup Architecture Validation
To prevent the Engine from attempting to execute incompatible binaries, the discovery phase acts as a strict architectural gate.
- When the Engine calls `./binary --graph` to discover the workflow topology, it wraps the call in a rigorous error handler.
- If the OS returns an `Exec format error` (e.g., wrong architecture or missing libc), the Engine explicitly logs a "Build Target Mismatch" error, refuses to register the workflow, and prevents any instances from being scheduled against it.
- We will provide a CLI command (`vo-cli check`) that proactively scans all registered binaries and validates their ELF/Mach-O headers against the host OS.

## Consequences
- **Positive:** In-flight workflows are completely insulated from breaking changes deployed mid-execution.
- **Positive:** Developers get clear, immediate feedback if they deploy a binary compiled for the wrong OS.
- **Negative:** The `/var/wtf/versions/` directory will accumulate old binaries over time. We must implement a Garbage Collector that sweeps this directory and deletes binaries that are no longer referenced by any active workflow instance.