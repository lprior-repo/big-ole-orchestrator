# ADR 037 (v2): Rebuildable Projections and Self-Healing Read Models

## Status
Accepted

## Context
Veloxide depends on operator-facing views such as `instances`, redacted history, effect timelines, and dashboard indexes.

These views are useful, but they are not the source of truth. If a projection drifts, is partially corrupted, or falls behind after a schema change, the Engine must not require manual data surgery.

The event-sourcing ecosystem already treats projections as disposable and rebuildable. We should steal that pattern directly.

## Decision
All operator-facing projections are rebuildable from canonical durable sources.

### 1. Projection Inputs
Projection builders may read from:
1. canonical events,
2. managed-effect journals,
3. workflow version records,
4. canonical payload blobs where necessary,
5. redaction rules for operator views.

### 2. Projection Classes
1. **Operational projections**
   - e.g. `instances`, recovery queue indexes, timer indexes.
   - updated transactionally for fast runtime access.
   - still rebuildable from canonical data if damaged.

2. **Operator projections**
   - dashboards, timelines, redacted summaries, convenience indexes.
   - may lag slightly and may be rebuilt lazily or offline.

### 3. Self-Healing
The Engine and CLI support projection rebuild workflows:
1. detect incompatible projection version,
2. pause or isolate that projection,
3. rebuild from canonical state,
4. swap the rebuilt projection into service.

## Consequences
- **Positive:** Projection corruption no longer threatens the correctness of the Engine core.
- **Positive:** Schema upgrades become safer because read models can be rebuilt instead of hand-migrated in place.
- **Positive:** The UI can remain rich without forcing every convenience index into the exact-once hot path.
- **Negative:** Projection rebuild time and lag now become operational considerations.
