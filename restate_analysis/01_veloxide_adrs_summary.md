# Veloxide v2 Architecture Analysis: The North Star

## Introduction: The "Single-Node Supreme" FaaS Orchestrator
Veloxide v2 is a dramatic pivot from its NATS JetStream-based v1 architecture. Instead of mimicking distributed, network-heavy engines like Temporal, Veloxide v2 aims to be a **Single-Node Local FaaS Orchestrator with an Exactly-Once Core**. It combines the visual observability of n8n, the execution speed of Windmill, the durable execution model of Restate, and the actor supervision of the Erlang BEAM.

At its heart, it is a pure Rust framework designed for ruthless execution speed without network overhead. The system operates under the constraint that distributed consensus is sacrificed to achieve maximum bare-metal local execution speed, meaning it scales vertically on a single strong machine, protecting the host OS via rigorous backpressure and admission control rather than relying on a cluster.

## Execution Model & The Code-as-Workflow SDK
Veloxide rejects Wasm sandboxing or HTTP push models for execution, opting instead for **Raw OS Subprocesses**. However, to avoid configuration drift and complex deployments, it employs a "Terraform Provider" model where an entire workflow—comprising all its tasks and DAG topology—is compiled into a **single Rust binary**. 

### The `vo-sdk`
Developers define workflows via the `vo-sdk` using a "Code-as-Workflow" fluent builder. The Rust compiler enforces **DAG Type Safety**, meaning if Step A outputs an `Order` and Step B expects a `Receipt`, the mismatch is caught at compile time. The SDK also detects cycles before execution.
When the Engine discovers the binary, it runs `./binary --graph` to extract the canonical `WorkflowSpec` JSON. This canonical spec is then shared universally across the Rust SDK, the Engine, AI tooling, and the embedded Dioxus WASM UI.

### Execution Boundaries & Hardening
Because subprocesses are inherently dangerous to the host OS, Veloxide employs extreme hardening:
1. **Version Pinning:** Discovered binaries are hashed (SHA-256) and copied to an immutable path (`/var/wtf/versions/<hash>`). Workflows are pinned to this hash, ensuring mid-flight workflows aren't broken by hot-reloads.
2. **Process Grouping:** Linux `PR_SET_PDEATHSIG` ensures child processes die if the Engine parent crashes, preventing zombie processes.
3. **Secure IPC (FD3/FD4):** `stdout` and `stderr` are strictly for user logging (with a 1MB truncation guard to prevent memory bombs). Actual state payloads and secrets are transmitted via dedicated file descriptors (FD3 for input, FD4 for output) using asynchronous pipes to prevent pipe deadlocks. Secrets are read directly into heap memory and never exposed via environment variables.
4. **Current-Thread Async:** To avoid ~200ms cold starts, the SDK sets up a single-threaded Tokio runtime instead of a multi-threaded work-stealing one, providing sub-millisecond initialization while keeping CPU contention low.

## State, Storage & Durability
Veloxide embeds **Fjall**, a pure-Rust LSM-tree, as its durable storage substrate. It abandons external databases entirely. 

### The Partitioned LSM
Storage is partitioned logically (e.g., `events`, `instances`, `timers`, `snapshots`, `dedupe`, `effects`, `leases`, `workflow_versions`, `payload_blobs`). Data is split between the **Hot Path** (small control-plane records required for replay) and the **Cold Path** (large payload blobs).

### Atomic WriteBatches (`DbWriterActor`)
Individual actors do not `fsync` to disk. All state transitions are sent to a single `DbWriterActor` which groups commits using `fjall::Batch`. A control-plane transition atomically updates all necessary partitions (events, timers, dedupe, etc.). If a batch fails, no partial state becomes visible.

### Write-Path QoS
To prevent large payloads or observability data from starving the exact-once control plane, the DbWriterActor enforces Quality of Service (QoS) classes:
- **Critical Control Plane** (never dropped)
- **Operator Projections** (can lag, rebuildable)
- **Bulk Blobs** (deferred under pressure)

Canonical payload blobs have strict publication rules: a control-plane event (like `StepCompleted`) cannot publish an `output_ref` until the blob itself is durably written to the cold path.

## Exactly-Once Core & Deterministic Replay
The engine core provides an honest exactly-once contract. It reconstructs state not by re-executing imperative workflow code, but through **Event-Sourced State Reconstruction**.

### The Replay Strategy
When recovering from a crash:
1. The engine reads events bounded by the latest **Periodic State Snapshot**.
2. It applies events through a pure `apply()` state machine to rebuild the `LifecycleState`.
3. It loads the canonical `WorkflowSpec` via the pinned binary hash.
4. It uses the reconstructed state, the topology, and recorded routing projections to determine the next action.

### Fence-Before-Commit & Leases
To prevent stale actors or late-returning subprocesses from double-committing, Veloxide uses **Execution Leases**. Before spawning a child, the Engine acquires a monotonic fence token for the `(instance_id, step_id)`. Every completion path (output, effects, failure) must carry this fence. If a child returns a stale fence, the `DbWriterActor` ignores it.

### Schema Evolution & Upcasting
Because workflows can run for months, every durable record carries a schema version. When reading old events, the Engine runs them through an ordered **Upcaster Chain** to normalize them to the latest schema before `apply()` runs. 

## Side Effects, Connectors, & Sinks
Veloxide classifies workflow steps into distinct execution classes:
1. **Pure Step:** Deterministic computation. No external side effects. Safe to physically recompute on recovery, though results are exactly-once observable.
2. **Managed Effect Step:** The child computes an `EffectIntent`. The Engine, not the child, executes the side effect through a managed connector.
3. **Wait / Signal Step:** workflow suspension via timers or signals.
4. **Unsafe Activity:** The child performs arbitrary side effects directly. Rejected in exact workflows; carries at-least-once semantics.

### The Managed Connector Contract
To qualify for exact-once, a connector must support idempotency keys, compare-and-set, or reconciliation. The connector runtime contract follows:
`prepare() -> commit() -> reconcile() -> compensate()`
The Engine writes an `EffectPrepared` journal entry before committing. If the Engine crashes during commit, recovery invokes `reconcile()` to handle ambiguity before any retries.

### Saga Compensation
Business rollback is separated from exact-once delivery. Connectors can define compensation policies (`Automatic` or `Manual`). Compensation is treated as its own first-class Managed Effect with its own journal, executed in reverse dependency order when a workflow enters the `Compensating` superstate.

## Concurrency, Scheduling, & Backpressure
The engine handles millions of operations in memory but protects the OS from process-spawning exhaustion.

### Actor Invariants
Every workflow instance is modeled as a single logical `ractor` actor. The Engine enforces a strict **Single-Writer Invariant**: at most one active actor per workflow instance exists at any time, controlled by a global registry lock.

### Backpressure Inversion
If the system is overloaded (e.g., 100,000 webhooks), spawning subprocesses is throttled by a global Execution Semaphore. If the `DbWriterActor`'s mailbox gets too deep, sending actors block. When the internal queue grows, the HTTP ingress router automatically sheds load with `HTTP 429` or `503`, pushing backpressure to the network edge rather than OOMing the host.

### Fairness & Workload Classes
To prevent a noisy workflow from starving the engine, workloads are classed (`ExactCritical`, `Standard`, `UnsafeBulk`, `Recovery`). Each class receives a reserved budget of subprocess permits and stderr capacity, ensuring recovery and critical workflows always make forward progress.

## Timers, Hibernation, & Signals
A durable engine must support sleeping workflows without consuming RAM.

### Suspend-to-Disk Hibernation
When an actor hits a `Wait` node, it atomically writes a wake-up entry to the `timers` partition and a `TimerScheduled` event, then calls `stop()` on itself, freeing all RAM. A single background **Reanimator loop** continuously scans the `timers` partition using a fixed-width big-endian binary key encoding, waking up actors when their time arrives. To survive NTP clock skew, timers record both absolute `fire_at` and monotonic duration.

### Lineage & Continue-As-New
For workflows that run forever, the history grows unboundedly. Veloxide implements `continue-as-new`, where a workflow lineage rolls over into a new execution epoch (`instance_id`), carrying forward only the minimal necessary state. Signals are lineage-routed by default, ensuring they hit the currently active epoch.

### Signal Matching & Deduplication
To guarantee exactly-once admission, incoming webhooks/signals must provide a stable dedupe key (`command_id`). The Engine atomically writes the dedupe record and the `WorkflowStarted` or `SignalAccepted` event. If the same key is seen within the retention window, the duplicate is ignored.

## Resilience, Privacy & AI
- **System Resilience:** If `fjall` compaction stalls or the disk slows down, a Storage Watchdog puts the Engine into **Degraded Mode**, rejecting non-critical ingress to protect the write path.
- **GDPR Purging:** The engine uses a dual-representation model: canonical encrypted replay data vs. redacted Operator Projections. A GDPR purge crypto-shreds the per-instance Data Encryption Key (DEK), making the canonical blobs unreadable while maintaining minimal pseudonymous control-plane facts required for replay correctness.
- **AI-Native Interfaces:** AI agents use the exact same Rust SDK and canonical JSON history as humans. To prevent LLM hallucination loops (where an AI compiles and deploys broken binaries endlessly), the Engine implements an **AI Circuit Breaker**: strict API rate limits and automatic Quarantine if $N$ consecutive deployments fail within a short window, forcing a human-in-the-loop intervention.

## Conclusion
Veloxide v2 defines an incredibly strict, deterministic FaaS orchestrator. By forcing all side-effects through Engine-managed connectors, relying on an atomic LSM-tree write path, leveraging hierarchical state machines (`Active`, `Suspended`, `Recovering`, `Compensating`, `Terminal`), and treating subprocesses as untrusted compute nodes fenced by FD3/FD4 and monotonic execution leases, the architecture honestly achieves exactly-once control-plane state transitions at bare-metal speeds.