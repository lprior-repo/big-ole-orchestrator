# GO-PLAN: twerk module plan 14 — Findings

## Context
Bead tw-e7zh: "GO-PLAN: twerk module plan 14" — Phase 1 of GO lifecycle for the twerk rig (synth polecat). Analyzed the veloxide codebase at `veloxide/mayor/rig/` against the ADR freeze set, implementation build order, and the decomposition spec.

## Codebase State Summary

### Workspace Members (13 crates)
| Crate | State | Phase Coverage |
|-------|-------|----------------|
| `vo-types` | Rich — 100+ source files, extensive types, signals, connectors, workspace, command envelope, schema versioning | Phase 0, 1, 5, 8 |
| `vo-common` | Moderate — error types, events, telemetry, data structures (octree, pairing heap) | Cross-cutting |
| `vo-core` | Moderate — admission control, circuit breaker, write class, workflow version, snapshot compat | Phase 0, 2 |
| `vo-storage` | Rich — dedupe partition, effect journal, instance index, lease partition, blob, receipts, snapshots, workflow version partition, query, key partition, projection compat, mmap cache | Phase 2, 4, 6, 7 |
| `vo-actor` | Rich — instance registry, lifecycle, ControlActor (cancel/resume/continue-as-new/accept-and-resume), fairness, semaphores, timer supervisor, signal buffer, spawn supervisor, reanimator, probes, message routing | Phase 3, 5, 6 |
| `vo-ipc` | Implemented — FD3/FD4 pipe protocol, envelope framing, config, error handling | Phase 3 |
| `vo-sdk` | Moderate — node handle, DAG builder | Phase 1 |
| `vo-sdk-macros` | Implemented — `#[vo_task]` macro, UI tests | Phase 1 |
| `vo-api` | Minimal — SSE events handler, v1 types | Phase 9 |
| `vo-cli` | Implemented — lint command, integration tests | Phase 9 |
| `vo-frontend` | Minimal — canvas context menu, payload preview panel | Phase 9 |
| `vo-linter` | Minimal — AST-based linting | Phase 9 |
| `vo-scheduler` | Implemented — job scheduler, queue, API, error handling | Phase 5/8 |
| `vo-executor` | Minimal — execute-node tests | Phase 3 |
| `vo-worker` | TBD — NATS-based distributed worker | Phase 6 |

### Critical Issue: Merge Conflict in vo-actor/src/lib.rs
Lines 178-193 contain an unresolved git merge conflict between HEAD and commit 7e356012 (rustfmt formatting). This must be resolved before any further work.

### Phase-by-Phase Implementation Status

#### Phase 0: Type and State Foundations — MOSTLY COMPLETE
- ADR-039 (Hierarchical lifecycle): `LifecycleState` enum in vo-actor, terminal/non-terminal classification
- ADR-036 (Command envelope): `CommandEnvelope`, `CommandMetadata` in vo-types
- ADR-035 (Event schema versioning): `EventVersion`, `MAX_SUPPORTED_SCHEMA_VERSION`, `State.version`, upcasters
- ADR-020 (Key encoding): key partition module in vo-storage
- **Gap**: Lifecycle state machine transitions need full formal state machine (currently just enum + `is_terminal()`)

#### Phase 1: Canonical Workflow Definition — PARTIAL
- ADR-031 (WorkflowSpec): Workspace module in vo-types (WorkspaceNode, WorkspaceEdge, etc.)
- ADR-004/009 (SDK builder): vo-sdk has DAG builder, vo-sdk-macros has `#[vo_task]`
- ADR-017/022 (Version pinning): `WorkflowVersion` in vo-core
- ADR-003 (Node kinds): Node kinds defined but not all enforced
- **Gap**: `--graph` emission to canonical WorkflowSpec not yet wired. SDK parity with engine validation incomplete.

#### Phase 2: Storage and Atomic Control Plane — SUBSTANTIAL
- ADR-002 (Fjall layout): Multiple partitions implemented (events, instances, leases, dedupe, effects, snapshots, blob, receipts, workflow versions, keys)
- ADR-016 (Atomic batch writer): Effect journal, snapshot modules
- ADR-032 (Write-path QoS): `WriteClass` in vo-core with integration tests
- **Gap**: No `DbWriterActor` for group commits (per CLAUDE.md architecture rule). Individual partitions write independently.

#### Phase 3: Execution Boundary — PARTIAL
- ADR-012/014 (FD3/FD4): vo-ipc fully implemented with benches, examples, tests
- ADR-011 (Current-thread runtime): Not visible
- ADR-019 (Signal handling): Not visible
- ADR-023 (Stderr bounds): Not visible
- ADR-006/015 (Semaphores/actor invariants): Semaphore module in vo-actor, spawn supervisor
- **Gap**: vo-executor minimal. No timeout enforcement visible. No SIGTERM race handling.

#### Phase 4: Exactly-Once Core — PARTIAL
- ADR-027 (Replay): Reanimator module in vo-actor (loop_core, mock, recovery tests)
- ADR-028 (Ingress dedupe): Dedupe partition in vo-storage with expiry and purge tests
- ADR-029 (Execution leases/fencing): Lease partition in vo-storage, fence token types, BDD tests
- ADR-013/016 (Snapshotting): Snapshots module in vo-storage, snapshot diff
- ADR-043 (Verification): Adversarial tests exist
- **Gap**: Full crash-point matrix not implemented. Replay engine not integrated with storage partitions.

#### Phase 5: Waiting, Timers, Signals — SUBSTANTIAL
- ADR-005 (Hibernation): BDD tests for hibernation in vo-actor
- ADR-042 (Signal matching): Signal buffer module, wait key, lineage scope, dedupe key, delivery types
- ADR-033 (Fairness): WorkloadClass, ReservedPermitBudget, fairness module
- ADR-036 (Command identity): Integrated into signal messages
- **Gap**: Timer supervisor needs integration with storage timers partition.

#### Phase 6: Managed Effects — PARTIAL
- ADR-030 (Effect journal): Effect journal in vo-storage with lifecycle, codec, proptests
- ADR-041 (Connector runtime): Connector module in vo-types (transition, verification, proptests)
- ADR-034 (Compensation): Compensation types in vo-types, tests
- **Gap**: No actual connector implementations (HTTP, SQL). Receipt store exists but not wired to connector state machine.

#### Phase 7: Privacy and Blob — PARTIAL
- ADR-040 (Blob durability): Blob module in vo-storage
- ADR-025 (Dual representation): Projection compat module
- **Gap**: Encryption/DEK-KEK lifecycle not visible. Blob GC not implemented.

#### Phase 8: Long-Lived Maturity — MINIMAL
- ADR-035 (Upcasters): Event upcaster exists in vo-types
- ADR-037 (Rebuildable projections): Not visible
- ADR-038 (Continue-as-new): `handle_continue_as_new` in ControlActor, `WorkflowContinued` type
- **Gap**: Projection rebuild tooling absent. Lineage rollover is scaffold only.

#### Phase 9: UI, AI, Operator — MINIMAL
- ADR-007/024 (Query APIs, SSE): Minimal SSE handler in vo-api
- ADR-008/025 (AI interfaces): Not visible
- ADR-026 (Circuit breakers): Circuit breaker in vo-core (config, failure window)
- **Gap**: No query APIs. Frontend is minimal stubs.

#### Phase 10: Freeze Gate — NOT STARTED
- ADR-043 (Verification suite): Adversarial tests exist but no formal release gate

## Key Architectural Concerns

1. **Merge conflict** in `vo-actor/src/lib.rs` (lines 178-193) — must resolve immediately
2. **No DbWriterActor** — CLAUDE.md mandates group commits via DbWriterActor but all storage partitions appear to write independently
3. **No composition root** — There is no `vo-engine` crate. The system has no binary entry point that wires actors together.
4. **vo-worker is empty** — NATS-based distributed execution not started
5. **vo-executor is minimal** — Only has execute-node tests, no actual executor logic
6. **No integration wiring** — Individual crates have rich implementations but nothing ties them together into a running system

## Recommended Next Steps for Implementation Agents

1. **Immediate**: Resolve merge conflict in vo-actor/src/lib.rs
2. **Phase 0 completion**: Formalize lifecycle state machine with explicit transition table
3. **Phase 2 critical**: Implement DbWriterActor for group commits (architectural requirement)
4. **Phase 3 completion**: Wire vo-executor with timeout enforcement and subprocess lifecycle
5. **Phase 4 integration**: Connect reanimator/replay engine to actual storage partitions
6. **Composition root**: Decide whether to add a new crate or use existing vo-api as the entry point

## Plan Assessment
The codebase has substantial implementation across all phases but lacks integration wiring. Individual crate implementations are strong (rich type systems, extensive tests including proptest, BDD, adversarial, red-queen). The primary gaps are:
- **Horizontal integration** (connecting crates into a running system)
- **DbWriterActor** (architectural mandate not yet implemented)
- **Merge conflict resolution** (blocking)
- **Composition root** (no binary entry point)
