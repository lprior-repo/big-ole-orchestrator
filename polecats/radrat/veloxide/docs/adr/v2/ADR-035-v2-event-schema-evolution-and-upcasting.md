# ADR 035 (v2): Event Schema Evolution and Upcasting

## Status
Accepted

## Context
Veloxide is a durable system. Long-running workflows, snapshots, effect journals, and operator history may survive for months.

That creates a hard problem: the Engine, UI, CLI, and AI tooling will evolve, but old event streams must still replay correctly. Without an explicit upcasting strategy, exact-once semantics degrade into "works until the first incompatible deploy."

This is a solved problem in mature event-sourcing ecosystems and we should steal it directly.

## Decision
We adopt explicit event schema evolution with upcasting.

### 1. Version Every Durable Record
Every durable record that participates in replay or operator inspection carries a logical schema version:
1. workflow events,
2. snapshots,
3. managed-effect journal entries,
4. canonical `WorkflowSpec` records,
5. operator projection records.

### 2. Upcasters Normalize Before Use
- The write path always emits the newest schema version.
- The read path normalizes older records through an ordered upcaster chain before replay, projection building, or privileged history export.
- `apply()` and all higher-level Engine logic operate on the newest logical schema only.

### 3. Snapshot Compatibility
- Snapshots carry their own schema version.
- If a snapshot cannot be safely upcast, the Engine discards it and rebuilds from the event log.
- Snapshots are therefore a cache, not a schema authority.

### 4. Deprecation Discipline
Fields may not be silently renamed or removed from stable operator/API contracts.
If a field must change:
1. add the replacement first,
2. upcast old records,
3. keep the deprecated field readable for at least one compatibility window,
4. remove only after the compatibility window has expired.

## Consequences
- **Positive:** Long-lived workflows remain replayable across Engine upgrades.
- **Positive:** UI, CLI, and AI tooling can target one stable logical contract instead of many historical wire shapes.
- **Positive:** Schema evolution becomes deliberate instead of accidental breakage.
- **Negative:** Every durable schema change now requires an upcaster and compatibility review.
