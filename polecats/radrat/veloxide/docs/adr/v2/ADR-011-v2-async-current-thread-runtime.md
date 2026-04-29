# ADR 011 (v2): Asynchronous Task Execution (Current-Thread Runtime)

## Status
Accepted

## Context
A workflow step (e.g., "Charge Stripe") almost always requires network I/O, meaning the developer's function must be `async` to use crates like `reqwest` or `sqlx`. 

However, because the `vo-engine` executes each step by spawning a completely fresh OS subprocess (ADR-003), spinning up a full Tokio multi-threaded work-stealing runtime inside that child process just to run a single `async` function introduces unacceptable cold-start latency (~200ms per step).

We need a way to support `async` Rust inside the binary without paying the cost of a full async runtime initialization.

## Decision
We will use Tokio's **Current-Thread Runtime** to execute the user's workflow steps inside the SDK.

When the Engine invokes a binary with `./binary --execute-node <name>`, the `vo_sdk::start()` initialization logic will not use `#[tokio::main]`. Instead, it will manually construct an ultra-lightweight, single-threaded async runtime.

### Implementation
```rust
// Inside vo_sdk::start() execution path
let rt = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()
    .expect("Failed to build lightweight tokio runtime");

let result = rt.block_on(async {
    // Invoke the user's async function
    func(input).await
});
```

## Consequences
- **Positive:** Sub-millisecond runtime initialization. The binary starts and executes almost instantly.
- **Positive:** Developers can freely write `async fn` and use standard async libraries (`reqwest`, `sqlx`) exactly as they expect.
- **Positive:** CPU efficiency. The multi-threading and concurrency are handled by the parent Orchestrator process (via `ractor`). The child process is strictly single-threaded, avoiding OS-level thread contention during viral load spikes.
- **Negative:** If a user accidentally writes computationally blocking synchronous code inside their `async` task, they will block the entire child process (though the Engine's timeout wrapper will eventually kill it). This is acceptable as the child process is intentionally short-lived and single-purpose.