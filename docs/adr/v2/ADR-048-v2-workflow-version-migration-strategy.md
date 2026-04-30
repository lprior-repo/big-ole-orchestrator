# ADR 048 (v2): Workflow Version Migration Strategy

## Status

Accepted

## Context

v1 had no formal version migration strategy. When a workflow binary changed, in-flight instances had no protection against semantic drift, and the system could not reason about whether an upgrade was safe or catastrophic.

v2 builds on ADR-004 (Code-as-Workflow SDK), ADR-017 (Version Pinning), ADR-021 (Ghost Workflows), and ADR-038 (Continue-As-New). These provide the raw materials: content-hashed versions, pinned execution, lifecycle states, and epoch rollover. But we need an explicit strategy that governs WHEN and HOW a running workflow can migrate between versions.

The core tension: developers need to deploy bug fixes and new features, but in-flight workflows must not be silently corrupted by semantic changes. We need a classification system that lets the Engine make deterministic decisions about upgrade safety.

## Decision

We adopt a three-layer version migration strategy:

### 1. Version Identification

Every `WorkflowSpec` carries two identifiers:

- **`version_hash`**: SHA-256 content hash of the canonical `WorkflowSpec` JSON (same mechanism as ADR-017 binary hashing)
- **`semver`**: Optional semantic version tag for human convenience (e.g., `1.2.0`)

The `version_hash` is the authoritative identity. The Engine never upgrades an in-flight instance based on `semver` alone — it always validates against the actual content hash.

### 2. Change Classification

When a new `WorkflowSpec` is registered, the Engine classifies the change against the currently-active version:

| Classification | Criteria | In-Flight Behavior |
|---------------|---------|-------------------|
| **Additive** | New optional nodes, new optional fields, new edge connections that don't alter existing paths | May upgrade at next checkpoint |
| **Deprecating** | Fields marked deprecated but still functional, default values changed for absent fields | Continue pinned; new instances use new version |
| **Breaking** | Removed nodes, changed node semantics, changed edge signatures, removed required fields | Continue pinned; new instances use new version |

The classification is computed by diffing the `WorkflowSpec` JSON structures. The Engine stores the classification result alongside the new version record.

### 3. Migration Checkpoints

Migration evaluation happens ONLY at defined checkpoints. A checkpoint is:

1. **Event threshold**: When `event_count` exceeds a configured limit (e.g., 1000 events per epoch)
2. **Signal threshold**: When `signal_count` exceeds a configured limit
3. **Explicit rollover**: When the workflow calls `continue_as_new` (ADR-038)
4. **Suspended transition**: When a suspended instance is resumed

At each checkpoint, the Engine evaluates:
- Current classification of the running instance's version vs the latest active version
- If additive and the instance opted in to auto-upgrade: proceed with migration
- If deprecating or breaking: continue pinned, no action

### 4. Pinned Execution Enforcement

This is the non-negotiable invariant:

> **In-flight instances MUST NOT run an unvalidated version combination.**

When a workflow instance is created, it records:
- `pinned_version_hash`: the hash it was started with
- `migration_policy`: `always`, `additive-only`, or `never`

The Engine's runtime enforces that all task dispatches, effect invocations, and state mutations use the `pinned_version_hash`. If a bug attempts to load code from a different hash, the Engine panics — this is a logic error, not a recoverable one.

### 5. Compatibility Matrix

```
                    │ In-Flight: Additive │ In-Flight: Deprecating │ In-Flight: Breaking │
────────────────────┼──────────────────────┼────────────────────────┼─────────────────────┤
New: Additive       │ checkpoint upgrade   │ continue pinned        │ continue pinned     │
New: Deprecating    │ continue pinned      │ continue pinned        │ continue pinned     │
New: Breaking       │ continue pinned      │ continue pinned        │ continue pinned     │
```

Only `Additive → Additive` allows mid-flight upgrade. All other paths preserve pinned execution.

## Consequences

- **Positive:** Developers can deploy additive changes and have running workflows pick them up automatically at a safe checkpoint.
- **Positive:** Breaking changes are mechanically prevented from corrupting in-flight workflows.
- **Positive:** The classification system is deterministic and auditable — operators can inspect why a given instance did or did not upgrade.
- **Negative:** Additive changes require discipline; a seemingly additive change that affects a workflow's hot path is effectively breaking.
- **Negative:** Classification diffing is not free; for large workflows with hundreds of nodes, the diff computation adds latency to registration.