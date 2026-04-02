# ADR 003 (v2): Raw Binary Execution via OS Subprocesses

## Status
Accepted

## Context
v1 used a complex NATS pull-queue architecture where worker SDKs connected over the network to pull tasks. Other orchestrators use WebAssembly sandboxes (Spin/Golem) or HTTP push (Restate).

We need an execution boundary that is unconditionally fast, requires zero network configuration, and allows users to bring their own Rust binaries without the limitations of Wasm/WASI networking.

However, raw subprocesses create an exact-once problem: an opaque child process that directly mutates Stripe, SQL, email, or arbitrary HTTP APIs cannot honestly be treated as exactly-once.

## Decision
We keep the Windmill/Lambda paradigm of **Execution via Raw OS Subprocesses**, but we split node semantics into explicit execution classes.

### Step Classes
1. **Pure Step**
   - The child reads input, performs deterministic computation, and returns output.
   - It must not perform irreversible external side effects.
   - If it needs wall-clock time, randomness, or external I/O to produce its result, it is not a Pure Step.

2. **Managed Effect Step**
   - The child computes and returns a typed `EffectIntent`.
   - The Engine, not the child, commits the external side effect through an engine-managed connector.
   - This is the only path eligible for exactly-once external effect semantics.

3. **Wait / Signal Step**
   - The workflow suspends until a timer or external signal resumes it.
   - No child-owned side effect is involved.

4. **Unsafe Activity**
   - The child may perform arbitrary external side effects directly.
   - The Engine treats it as **at-least-once only**.
   - Exact workflows must reject graphs containing Unsafe Activities.

### CLI Contract
When the Engine dictates that a node should execute:
1. It uses `tokio::process::Command` to spawn the local workflow binary.
2. It sends the input payload and execution metadata over the dedicated IPC contract.
3. For Pure Steps, it awaits deterministic output.
4. For Managed Effect Steps, it awaits a typed `EffectIntent` envelope.
5. It captures `stderr` for bounded observability and debugging.

### Security and Isolation
Because we do not use Docker or Wasm sandboxing, the Engine protects itself via the OS and strict contracts:
- **Timeouts:** Every subprocess is wrapped in `tokio::time::timeout` and fenced by the Engine.
- **Secret Injection:** Secrets travel over the dedicated IPC channel, never through host environment variables.
- **Admission Control:** OS-level resource exhaustion is prevented by bounded permits, mailbox limits, and workload classes (ADR-006 and ADR-033).

### The SDK Macro
To make this ergonomic, `vo-sdk` still generates the binary entry point, but the node kind becomes explicit. The developer writes task logic against a typed contract and the SDK generates the runtime envelope.

## Consequences
- **Positive:** We preserve the raw speed and simplicity of local subprocess execution.
- **Positive:** Exact-once becomes honest for Pure Steps and Managed Effect Steps.
- **Positive:** Developers still get an escape hatch for arbitrary binaries through Unsafe Activities.
- **Negative:** The engine must own more of the side-effect surface through connectors and effect journaling.
- **Negative:** Some existing "just call Stripe in the task" workflows must be re-modeled as managed effects to qualify for exact-once behavior.
