# ADR 031 (v2): Canonical WorkflowSpec Shared by SDK and UI

## Status
Accepted

## Context
If the Rust SDK, Engine, AI tooling, and future drag-and-drop UI each evolve their own workflow definition format, the product will drift into incompatible semantics and hidden feature gaps.

## Decision
We define a versioned canonical `WorkflowSpec` as the sole workflow definition contract.

### 1. Sources of Truth
1. The Rust SDK builder compiles to `WorkflowSpec`.
2. `--graph` emits `WorkflowSpec` JSON.
3. The Engine validates, hashes, and stores `WorkflowSpec` in `workflow_versions`.
4. The Dioxus UI reads and edits the same `WorkflowSpec`.

### 2. Node Metadata
`WorkflowSpec` must encode at least:
1. node kind (`Pure`, `ManagedEffect`, `Wait`, `Signal`, `Unsafe`),
2. retry policy,
3. routing metadata,
4. guarantee class,
5. connector capability requirements for managed effects,
6. compensation policy (`None`, `Automatic`, `Manual`) for managed effects,
7. signal matching and dedupe scope requirements for `Signal`/`Wait` nodes.

### 3. Publish-Time Validation
Before activation, the Engine validates the spec for:
1. cycle safety,
2. exact-workflow eligibility,
3. unsupported sink usage,
4. missing dedupe requirements,
5. canonical serialization stability,
6. lineage rollover policy if the workflow is marked long-running (ADR-038).

## Consequences
- **Positive:** SDK, Engine, AI, and UI now share one workflow language.
- **Positive:** The future drag-and-drop UI can be first-class without inventing a second semantic model.
- **Negative:** `WorkflowSpec` becomes a stable compatibility surface that must be versioned carefully.
