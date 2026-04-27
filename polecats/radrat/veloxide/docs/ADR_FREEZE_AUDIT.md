# ADR Freeze Audit

This document is the final contradiction and sharp-edge audit for `ADR-001` through `ADR-043`.

## Status

Overall status: **Acceptable to freeze architecturally**.

The ADR set is now coherent enough to implement against. The remaining issues are mostly parameterization and implementation detail, not missing architectural shape.

## What Is Now Consistent

1. **Execution contract**
   - FD3/FD4 is now the canonical execution IPC path.
   - `stdout` / `stderr` are user-log channels only.
   - `ADR-005` and `ADR-009` were revised to match that model.

2. **Exactly-once core claim**
   - No longer hand-wavy.
   - Admission, fencing, managed effects, connector reconciliation, and verification all exist as explicit contracts.

3. **Privacy vs replay truth**
   - Canonical replay data is encrypted.
   - Operator projections are redacted.
   - Blob publication is now explicitly controlled.

4. **Long-lived workflows**
   - Schema evolution, rebuildable projections, lineage rollover, and signal semantics all exist.

5. **Compensation**
   - Clearly separated from exact-once delivery.
   - Tied into the lifecycle model.

## Remaining Sharp Edges

These are not contradictions, but they are still decision-sensitive areas during implementation.

### 1. Dedupe Retention Is a Product Limit
`ADR-028` and `ADR-001` now align, but the exact-once admission guarantee is retention-bounded.

That is acceptable, but it must remain visible in product language and API docs.

### 2. Connector Semantics Will Make or Break Trust
`ADR-041` is solid, but the architecture now depends heavily on high-quality connector implementations.

The first connectors must be chosen conservatively.

Recommendation:
1. ship one HTTP idempotency-key connector,
2. ship one SQL/unique-constraint connector,
3. do not promise exact-once for anything weaker.

### 3. Signal Buffering Needs Tight Defaults
`ADR-042` allows `Reject`, `BufferOne`, or `BufferMany`.

Recommendation:
1. default to `Reject`,
2. require explicit opt-in for buffering,
3. keep buffer bounds very small at first.

### 4. Compensation Complexity Is Real
`ADR-034` is right architecturally, but compensation logic can become the new failure swamp.

Recommendation:
1. support `None` and `Manual` first,
2. add `Automatic` only for connectors with strong reconcile semantics.

### 5. Projection Rebuild Time Can Surprise You
`ADR-037` is good, but rebuild cost may become operationally noticeable on large datasets.

That is acceptable as long as projections stay outside the exact-once hot path.

## Former Contradictions Now Resolved

1. Old `stdout` suspend directive model: resolved.
2. Old `stdin` / `stdout` execution model in multi-task binary: resolved.
3. Blob reference publication before durability: resolved by `ADR-040`.
4. Signal routing across `continue-as-new`: resolved by `ADR-042`.
5. Missing exact-once verification doctrine: resolved by `ADR-043`.

## One Final Recommendation Before Freeze

Treat these ADRs as the semantic freeze set for implementation:

### Core Freeze Set
`001`, `002`, `003`, `004`, `012`, `014`, `016`, `027`, `028`, `029`, `030`, `031`, `032`, `033`, `034`, `035`, `036`, `038`, `039`, `040`, `041`, `042`, `043`

Everything else can evolve more freely as long as it does not violate that semantic core.

## Bottom Line

This architecture is now strong enough to build.

If it fails from here, it is more likely to fail from:
1. implementation sloppiness,
2. weak connector semantics,
3. missing crash-injection coverage,
4. operational tuning,

not from a missing conceptual model.
