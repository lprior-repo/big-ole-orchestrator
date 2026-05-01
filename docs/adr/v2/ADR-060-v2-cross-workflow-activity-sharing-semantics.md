# ADR 060 (v2): Cross-Workflow Activity Sharing Semantics

## Status
Accepted

## Context
The engine supports concurrent execution of many workflow instances. Without a formal position on whether and how activities (events, effects, state, timers) may be shared across workflow boundaries, future features risk introducing implicit coupling that violates isolation guarantees.

Several cross-instance mechanisms already exist (signals via ADR-042, lineage continue-as-new via ADR-038, namespace tenancy, execution leases via ADR-029), but the boundary between "intentional cross-instance communication" and "accidental state leakage" has never been formally declared.

## Decision
We adopt a **shared-nothing** activity model. Each workflow instance owns its state exclusively. Cross-instance interaction is limited to a small, explicit set of message-passing primitives.

### 1. Isolation Unit
`InstanceId` is the universal isolation boundary. Every storage partition (`events`, `effects`, `leases`, `snapshots`, `timers`, `dedupe`) uses `InstanceId` as the leading key component, ensuring prefix scans cannot cross instance boundaries.

No two instances share:
- event streams,
- effect journals,
- lease spaces,
- timer queues,
- deduplication windows,
- or hibernated state.

### 2. Permitted Cross-Instance Mechanisms
The following are the only sanctioned cross-instance interactions:

| Mechanism | Scope | Direction | ADR |
|-----------|-------|-----------|-----|
| Signals | `(lineage_id, instance_id, wait_key)` | Addressed, point-to-point | ADR-042 |
| Lineage routing | `lineage_id` | Rollover to successor epoch | ADR-038 |
| Namespace scoping | `NamespaceId` | Tenant partitioning | Implicit |
| Workflow-type semaphore | `WorkflowName` | Concurrency limiting | N/A |

None of these mechanisms allow shared mutable state. Signals are asynchronous messages, not shared memory. Lineage creates a new instance; it does not alias the old one.

### 3. Prohibited Patterns
The following are architecturally forbidden:

1. **Shared mutable state between instances.** No global variables, shared caches, or cross-instance locks that allow one instance to observe another's intermediate state.
2. **Cross-instance event reads.** An instance may only read its own event stream. Historical queries across instances use the query/CLI layer, not the actor path.
3. **Cross-instance effect journal access.** Effect journals are strictly per-instance (verified by Red Queen adversarial tests in `vo-storage`).
4. **Broadcast operations.** There is no mechanism to send a single message to all active instances. Signals target exactly one `(lineage_id, instance_id, wait_key)`.
5. **Dedupe scope across instances.** `DedupeScope::Exact` is per-instance. `DedupeScope::Unbounded` disables dedupe entirely. No cross-instance dedupe exists.

### 4. Storage Isolation Guarantees
Fjall key encoding (ADR-020) ensures physical isolation at the storage layer:

- `events`: key = `[instance_id(16)][sequence(8)]` -- prefix scan on instance_id yields only that instance's events.
- `effects`: key = `[instance_id(16)][sequence(8)][0xFF]` -- same prefix isolation.
- `leases`: key = `[instance_id(16)][step_id_len(2)][step_id]` -- per-instance lease namespace.
- `timers`: key = `[fire_at(8)][instance_id(16)]` -- fire-time ordering with instance isolation.
- `instances`: key = `[status(1)][created_at(8)][instance_id(16)]` -- index scan, not cross-instance.

### 5. Actor Isolation
The `InstanceRegistry` guarantees at most one active `ractor` actor per `InstanceId` per node (ADR-012, ADR-015). Actors cannot reference another instance's state handle. `OrchestratorMsg` variants carry a single `(namespace, instance_id)` pair -- there is no multi-instance message type.

### 6. Future Extension Points
If a future feature requires cross-instance coordination beyond signals, it must:
1. Be proposed as a new ADR,
2. Use an explicit message-passing model (not shared state),
3. Preserve the storage-layer isolation guarantees,
4. Be scoped to a specific `NamespaceId` to prevent cross-tenant leakage.

Candidate patterns (not yet decided): parent-child workflow spawning, workflow-to-workflow RPC, shared read-only configuration.

## Consequences
- **Positive:** Reasoning about any single workflow instance never requires understanding other concurrent instances. Debugging, replay, and recovery are purely local to one instance's event stream.
- **Positive:** Storage compaction, snapshot, and archival can operate per-instance without coordination.
- **Positive:** Multi-tenancy via `NamespaceId` is enforceable because no storage key spans namespaces.
- **Positive:** Adversarial testing (Red Queen cross-instance isolation tests) validates the guarantee continuously.
- **Negative:** Any future need for shared state between workflows requires a new ADR and an explicit messaging mechanism.
- **Negative:** Aggregate queries across instances must use the CLI/query layer, not the actor fast path.
