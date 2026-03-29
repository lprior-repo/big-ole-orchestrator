# ADR 001 (v2): The V2 North Star Architecture

## Status
Accepted

## Context
The v1 architecture of `vo-engine` relied heavily on NATS JetStream for durable event logs and queueing, mimicking the Temporal architecture. While highly scalable for distributed datacenters, it violated the core goals of the project:
1. True "Single Binary" deployments (zero external infrastructure/daemons).
2. Pure Rust ecosystem.
3. Ruthless execution speed without network overhead.
4. "Oban-like" Developer Experience (DevEx).

## Decision
We are pivoting the entire architecture to a **Local FaaS (Functions-as-a-Service) Orchestrator**. 

The engine is a single Rust framework. It combines the visual observability of n8n, the raw execution speed of Windmill, the durable execution of Restate, and the lock-free concurrency of the Erlang BEAM.

### Core Pillars
1. **Storage:** `fjall` (Pure Rust LSM-Tree). Provides face-melting disk I/O without requiring external databases.
2. **Concurrency:** `ractor`. Every workflow is a lock-free actor that sequentially processes events and hibernates to disk when suspended.
3. **Execution:** Standard OS Subprocesses. The engine spawns compiled Rust binaries via `tokio::process::Command`, piping JSON payloads via `stdin`/`stdout`.
4. **Definition:** Code-as-Workflow. Workflows are defined strictly in Rust code (`main.rs`) using the `vo-sdk`, not in JSON files.
5. **Observability:** An embedded Axum router serving a Dioxus WASM UI that tails the LSM-Tree via Server-Sent Events (SSE) for real-time glowing node graphs.

## Consequences
- **Positive:** We achieve absolute bare-metal speed with zero network latency between tasks.
- **Positive:** Developers get compile-time safety for their workflow graphs.
- **Positive:** No-Code operators get an n8n-style visual graph generated directly from the compiled code.
- **Negative:** We lose out-of-the-box distributed clustering (Raft). Scaling beyond a single 16-core machine requires a different paradigm, which is an acceptable tradeoff for 99.9% of use cases.