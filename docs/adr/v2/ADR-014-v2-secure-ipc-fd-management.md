# ADR 014 (v2): Secure IPC and File Descriptor Management (FD3/FD4)

## Status
Accepted

## Context
When the Engine spawns a user's workflow binary, passing sensitive data (like API keys) via Environment Variables exposes those secrets to the host OS. Any user running `ps aux` or inspecting `/proc/<pid>/environ` can read the plaintext secrets. 

Furthermore, passing data via `stdout` risks corruption if the user's code prints debug logs. Using a custom File Descriptor (e.g., FD3) solves this, but if the user's binary shells out to another process (like `curl`), that child process inherits the open file descriptors. If the child holds the FD open, the Engine will hang forever waiting for an EOF.

## Decision
We implement a strictly isolated, dual-pipe IPC architecture using `O_CLOEXEC` and in-memory secret parsing.

### 1. Dual-Pipe Architecture
- **FD 3 (Engine -> Task):** Used exclusively to send the JSON state payload and Secrets to the binary.
- **FD 4 (Task -> Engine):** Used exclusively for the binary to return the mutated JSON state.
- **FD 1 & 2 (`stdout`/`stderr`):** Reserved entirely for user logging, which the Engine captures and streams to the UI.

### 2. The CLOEXEC Guarantee
When the Engine creates the pipes for FD3 and FD4, it must set the `O_CLOEXEC` flag. This guarantees that if the user's task binary uses `std::process::Command` to spawn a subprocess, the OS will automatically close the pipes for the subprocess, preventing inherited FD hangs.

### 3. In-Memory Secret Vault
Secrets are never passed as environment variables. They are injected as part of the initial JSON payload sent over FD3. The `vo-sdk` reads this payload into heap memory, zeroizes the buffer if necessary, and exposes the secrets via `ctx.secret("STRIPE_KEY")`. The secrets never touch `procfs`.

## Consequences
- **Positive:** Zero secret leakage to the host OS.
- **Positive:** Impossible for third-party crates or subprocesses to corrupt or hang the IPC pipes.
- **Negative:** Managing custom file descriptors cross-platform (Windows vs Linux) requires careful `cfg` gating in the Engine and SDK.