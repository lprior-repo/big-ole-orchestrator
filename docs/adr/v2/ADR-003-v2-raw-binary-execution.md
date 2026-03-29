# ADR 003 (v2): Raw Binary Execution via OS Subprocesses

## Status
Accepted

## Context
v1 used a complex NATS pull-queue architecture where worker SDKs connected over the network to pull tasks. Other orchestrators use WebAssembly sandboxes (Spin/Golem) or HTTP Push (Restate). 

We need an execution boundary that is unconditionally fast, requires zero network configuration, and allows users to bring their own Rust binaries without the limitations of Wasm/WASI networking.

## Decision
We adopt the Windmill/Lambda paradigm: **Execution via Raw OS Subprocesses**.

When the Engine dictates that a `Task` node should execute:
1. It uses `tokio::process::Command` to spawn a local compiled Rust binary (e.g., `./data/binaries/charge_stripe`).
2. It pipes the JSON payload state into the process via `stdin`.
3. It awaits the mutated JSON payload state on `stdout`.
4. It captures `stderr` for UI observability and debugging.

### Security and Isolation
Because we do not use Docker or Wasm sandboxing, the Engine protects itself via the OS:
- **Timeouts:** Every subprocess is wrapped in a `tokio::time::timeout`. Hanging binaries are aggressively killed via `SIGKILL`.
- **Secret Injection:** The Engine clears the host's environment variables and explicitly injects scoped vault secrets (e.g., `VO_SECRET_STRIPE_KEY`) directly into the child process.

### The SDK Macro
To make this ergonomic, we provide a `vo-sdk` crate. The developer writes:
```rust
#[vo_task]
async fn my_task(ctx: Context, input: Input) -> Output { ... }
```
The macro generates the `main()` function that handles reading `stdin`, parsing JSON, managing errors, and serializing `stdout`.

## Consequences
- **Positive:** Absolute fastest possible execution. No HTTP/TCP overhead.
- **Positive:** Ultimate language agnosticism at the OS layer (though we strictly target Rust).
- **Positive:** DevEx is incredibly simple. Just compile and drop the binary.
- **Negative:** The engine is vulnerable to OS-level resource exhaustion if not gated. (Mitigated by ADR-027: Execution Semaphores).