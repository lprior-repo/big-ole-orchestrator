# ADR 001 (v2): The V2 North Star Architecture

## Status
Accepted

## Context
The v1 architecture of `vo-engine` relied heavily on NATS JetStream for durable event logs and queueing, mimicking the Temporal architecture. While highly scalable for distributed datacenters, it violated the core goals of the project:
1. True "Single Binary" deployments (zero external infrastructure/daemons).
2. Pure Rust ecosystem.
3. Ruthless execution speed without network overhead.
4. "Oban-like" Developer Experience (DevEx).

The v2 architecture also targets a stronger contract than generic durable background work: the engine core must provide **exactly-once admission and exactly-once control-plane state transitions** while preserving single-node throughput.

## Decision
We are pivoting the entire architecture to a **Single-Node Local FaaS Orchestrator with an Exactly-Once Core**.

The engine is a single Rust framework. It combines the visual observability of n8n, the raw execution speed of Windmill, the durable execution model of Restate, and the actor/supervision mentality of the Erlang BEAM.

### Core Pillars
1. **Storage:** `fjall` is the embedded durable control-plane store. It holds events, timers, dedupe records, effect journals, leases, snapshots, workflow versions, and cold payload blobs without requiring external databases.
2. **Concurrency:** `ractor`. Every workflow instance is modeled as a single logical actor with strict single-active-instance invariants, bounded mailboxes, and hibernation to disk when suspended.
3. **Execution:** Standard OS subprocesses remain the local compute boundary, but exact-safe external side effects move behind engine-managed connectors. Raw subprocesses are fast; opaque side effects are not trusted for exact-once semantics.
4. **Definition:** Workflows are defined through a canonical `WorkflowSpec` shared by the Rust SDK, the engine, AI tooling, and the future drag-and-drop UI.
5. **Observability:** An embedded Axum router serves a Dioxus WASM UI. Timeline and history APIs are authoritative; SSE is a best-effort live tail.
6. **Guarantees:**
   - Exactly-once admission for supported external triggers and signals within the configured dedupe retention window.
   - Exactly-once control-plane transitions inside the engine.
   - Deterministic replay from durable state.
   - Exactly-once managed effects for supported sinks.
   - At-least-once only for explicitly unsafe activities.

### Product Boundary
Veloxide is intentionally **not**:
1. A hostile multi-tenant sandbox for arbitrary untrusted code.
2. A distributed consensus workflow cluster.
3. An exactly-once engine for arbitrary opaque binaries that directly mutate external systems.
4. A general-purpose document store for massive payloads and unbounded logs.

## Consequences
- **Positive:** We preserve bare-metal local execution speed while giving the control plane honest exactly-once semantics.
- **Positive:** SDK, AI tooling, and the future drag-and-drop UI can all target one canonical workflow model.
- **Positive:** The architecture stays single-node and Rust-native without pretending that raw subprocesses can magically provide exact-once side effects.
- **Negative:** Exact-once is now a capability-based contract. Only supported ingress types and managed-effect connectors qualify.
- **Negative:** The engine is more opinionated. Users who want unrestricted subprocess behavior must accept weaker guarantees.
- **Negative:** We still give up out-of-the-box distributed clustering. Scaling beyond a single strong machine requires a different architecture.
