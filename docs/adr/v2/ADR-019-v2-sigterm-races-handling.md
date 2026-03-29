# ADR 019 (v2): SIGTERM Races and Signal Handling

## Status
Accepted

## Context
When the Engine's timeout fires, it sends `SIGTERM` to the child binary. The child is expected to flush state and exit cleanly. However, in Rust, signal handlers cannot safely allocate memory or perform I/O. The standard pattern of setting an `AtomicBool` and checking it in a loop fails here because the user's task code *is* the main loop, and it might be blocked on a 45-second ML inference call. The `SIGTERM` would be ignored, forcing the Engine to escalate to `SIGKILL` and losing the graceful shutdown.

## Decision
The `vo-sdk` will intercept and manage signals using a dedicated background thread.

### The Implementation
When the `vo_sdk::start()` function initializes the current-thread runtime:
1. It spawns a dedicated background `std::thread` to listen for OS signals (using `ctrlc` or a `SignalFd`).
2. The main thread executes the user's task function.
3. If the background thread receives `SIGTERM`, it is allowed to perform any necessary SDK-level cleanup (because it is a normal thread, not a restricted signal handler context).
4. It is given a strict 2-second grace period.
5. It then calls `std::process::exit(1)`, aggressively tearing down the main thread regardless of what the user's task is doing.

## Consequences
- **Positive:** Deterministic, reliable shutdown behavior that doesn't rely on the developer writing cooperative loop-checking code.
- **Positive:** Prevents the Engine from having to wait the full 5-second `SIGKILL` escalation window for unresponsive tasks.
- **Negative:** The user's task is abruptly aborted without being able to run its own custom `Drop` or cleanup logic. (Mitigated by the overarching Event Sourcing at-least-once replay guarantees).