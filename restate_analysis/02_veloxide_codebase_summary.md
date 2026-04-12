# Veloxide Codebase Analysis

**Date:** April 2026  
**Context:** Comprehensive review of the `veloxide` (formerly `vo-engine`) codebase architecture, components, and design philosophies. 
*(Note: `src/restate/` has been explicitly ignored for this analysis).*

## 1. High-Level Overview

Veloxide is an ultra-reliable, deterministic workflow execution engine designed for exactly-once processing, resilient state management, and high-concurrency throughput. It heavily relies on the **Actor Model**, **Event Sourcing**, and **Strict Domain-Driven Design (DDD)**. 

The repository is structured as a Cargo workspace with strict boundaries between domains, moving away from a monolithic `vo-engine` crate into a modular set of smaller crates (e.g., `vo-types`, `vo-storage`, `vo-actor`, `vo-core`). 

### Core Philosophies & EARS Requirements
- **Event Sourcing as the Source of Truth:** The system state is not updated in place; instead, it is deterministically recomputed from an immutable append-only journal of `EventEnvelope`s.
- **Actor Model (via `ractor`):** Workflow instances are isolated state machines (actors). They receive messages like `StartWorkflow`, `StepCompleted`, `StepFailed`, `TimerFired`, and emit durable events.
- **Exactly-Once Processing:** High emphasis on ingress deduplication (`dedupe_partition`), idempotency, and strict completion fencing (managed via `lease_partition`).
- **Extreme Quality Standards:** The codebase employs strict linting (`#![deny(clippy::unwrap_used)]`), widespread property-based testing (Proptest), and symbolic execution (Kani) for critical state verification.
- **Dolt-Backed Task Tracking:** The project explicitly uses a CLI tool called `bd` (beads) integrated with Dolt for task and issue tracking, avoiding external trackers or markdown TODO lists.

---

## 2. Workspace Structure & Crate Breakdown

The workspace comprises several highly focused crates mapped directly to architectural phases (as outlined in `architecture-spec.md`).

### `vo-types` (Phase 0: Domain Types)
This is the foundational vocabulary of the engine. It contains all core data models without implementing heavy logic.
- **Events & Envelopes:** `EventEnvelope`, `EventVersion`.
- **Workflow DAG Definitions:** Types governing workflow shapes (`WorkflowName`, `NodeName`, `EdgeCondition`, `DagNode`).
- **Core Identifiers:** `InstanceId`, `TimerId`, `StepId`.
- **Lifecycle & Compensation:** `LifecycleSuperstate`, `CompensationRecord`, `CompensationStatus`.
- **Dedupe & Fencing:** `DedupeKey`, `FenceToken`, `IdempotencyKey`.

### `vo-storage` (Phase 2: Storage & Partitioning)
The state persistence layer, heavily reliant on `fjall` (an embedded KV store). It is explicitly partitioned by concern:
- **`effect_journal`:** The append-only event store representing the immutable history of all workflow instances.
- **`dedupe_partition`:** Manages exactly-once ingress by tracking idempotency keys.
- **`lease_partition`:** Handles execution leases to prevent split-brain processing and stale writer issues.
- **Indexes:** `timer_index` (for delayed execution scheduling) and `instance_index`/`status_store` (for fast query access).
- **Snapshots:** Provides fast-forward state loading (compatible with the replay engine).

### `vo-core` (Phase 4: Exactly-Once Core & Mechanics)
The brain of the deterministic state machine.
- **`replay::engine::ReplayEngine`:** The pure function engine that reads a slice of `EventEnvelope`s from the `effect_journal` and reduces them into a current `LifecycleState` without side effects.
- **`upcaster`:** Handles schema evolution. If older event versions exist in the journal, the upcaster registry mutates them into the current `EventVersion` prior to replay.
- **`admission`:** Admission control and validation for accepting new workflows/signals.
- **`circuit_breaker` / `debounce`:** Stability mechanisms to handle rapid step failures or high load.
- **`write_class`:** Quality of Service (QoS) tiering for storage writes (control-plane vs projection vs blob).

### `vo-actor` (Phase 5 & 6: Runtime & Signals)
Uses the `ractor` framework to execute the workflows dynamically.
- **Instance Actors:** A `ractor` actor represents a live workflow instance, reacting to `InstanceActorMessage` (e.g., `StepCompleted`, `TimerFired`).
- **Control Actors:** Manages the overarching lifecycle (Cancel, Resume, Suspend).
- **`reanimator`:** Responsible for reviving idle/sleeping workflow actors from storage upon receiving new events or signals.
- **Signal Buffering:** Handles `WaitKey` based event routing, allowing workflows to suspend until external signals match their criteria.

### `vo-executor` (Phase 3: Execution Boundary)
Responsible for safely executing a pure workflow step.
- Handles standard timeout enforcement and wraps node execution.
- Enforces `RetryPolicy` rules when steps fail, bridging the gap between raw execution and the actor state machine.

### `vo-ipc` (Phase 3: Fast Process Isolation)
Handles inter-process communication for running steps in true isolation.
- Communicates purely over Unix pipes using File Descriptors `3` (parent writes payload to child) and `4` (child writes binary response payload to parent).
- Captures and enforces stderr bounding and tight process kill/timeout mechanics, ensuring isolated steps cannot indefinitely block the engine.

### `vo-api` & `vo-frontend` (Phase 9: API & UI)
- **`vo-api`:** An `axum` based HTTP server providing endpoints for managing workflows, signals, and querying events.
- **`vo-frontend`:** A `dioxus` powered UI application, serving as the management dashboard.

### `vo-sdk` & `vo-sdk-macros` (Phase 1: Definition)
The developer interface for defining workflows. Exposes proc-macros (e.g., `#[task]`) to construct DAG nodes easily while preserving compile-time validation.

### `vo-cli`, `vo-worker`, `vo-common`, `vo-linter`
- **`vo-cli`:** Uses `clap` for subcommands like `serve`, `gc` (garbage collection), `check`, and `lint`.
- **`vo-worker`:** The long-running daemon that pulls from work queues, manages the local timer supervisors, and spins up `vo-executor` calls.
- **`vo-linter`:** Custom AST-based linters (using `syn` and `quote`) to enforce engine-specific static analysis rules on user-written workflows.

---

## 3. Notable Architectural Patterns

1. **Anti-Corruption & Isolation (IPC):** 
   By isolating step execution via `vo-ipc` using raw file descriptors (FD3/FD4), Veloxide guarantees that panicking, crashing, or memory-leaking user-code (steps) will not corrupt the control plane.
   
2. **Upcasting (Event Schema Evolution):**
   Instead of supporting multiple branches of application logic for old events, Veloxide uses an `UpcasterRegistry` (`vo-core/src/upcaster/`). This transforms old event formats into the current format *during the replay step*, meaning the `LifecycleState` reducer only ever needs to understand the `vLatest` schema.

3. **Adversarial Verification:**
   The repository heavily relies on `kani` and `proptest`. There are explicit `red_queen_tests` (evolutionary adversarial QA) and `adversary_tests` within `vo-ipc` and `vo-storage`. The system assumes the disk, network, and execution steps are actively hostile.

4. **Strict Bead (Task) Management:**
   The `architecture-spec.md` makes it incredibly clear that vague meta-tasks are strictly forbidden. Implementation happens via highly atomic "Beads" (managed via Dolt/`bd`). Active work must target explicit crates (no legacy `vo-engine` or `vo-ui` assumptions) and conform strictly to a 10-Phase implementation order.

## 4. State Machine Workflow

A typical execution flow:
1. **Ingress:** API receives a request to start a workflow. Checked against `dedupe_partition`.
2. **Journaling:** A `WorkflowStarted` event is appended to `effect_journal`.
3. **Reanimation:** The `reanimator` spins up an Actor (`vo-actor`) for the instance.
4. **Replay:** The Actor uses `vo-core`'s `ReplayEngine` to catch up to the current state.
5. **Execution:** The Actor schedules the next step. `vo-worker` picks it up and uses `vo-ipc` & `vo-executor` to run it safely.
6. **Completion:** The step returns via FD4, the result translates into a `StepCompleted` event, which is appended to the journal, advancing the state machine.