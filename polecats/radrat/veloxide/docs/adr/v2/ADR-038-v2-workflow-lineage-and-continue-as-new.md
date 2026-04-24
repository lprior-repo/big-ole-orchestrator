# ADR 038 (v2): Workflow Lineage and Continue-As-New

## Status
Accepted

## Context
Snapshots bound replay cost, but they do not solve unbounded history growth, lineage confusion, or compaction debt for very long-lived workflows.

Durable workflow systems solve this with a "continue-as-new" pattern. We should steal that directly.

## Decision
We introduce stable workflow lineage with execution epochs.

### 1. Lineage vs Instance
1. `workflow_lineage_id` identifies the logical long-lived workflow.
2. `instance_id` identifies one execution epoch within that lineage.

### 2. Continue-As-New
The Engine may roll a workflow into a new epoch when:
1. event count exceeds a configured threshold,
2. signal count exceeds a threshold,
3. payload-blob references become too numerous,
4. the workflow explicitly requests rollover.

The rollover atomically:
1. writes `ContinuedAsNew` for the old epoch,
2. creates a new `WorkflowStarted` for the successor epoch,
3. carries forward the minimal canonical state required to continue execution,
4. updates lineage routing so new signals and queries target the active epoch.

### 3. Operator and SDK Semantics
- The UI presents the lineage as one logical workflow with drill-down into epochs.
- The SDK may expose a `continue_as_new` directive for supported long-running patterns.
- Dedupe, compensation, and effect history use lineage-aware routing where required.
- Signal matching and dedupe scope across lineage rollover are governed by ADR-042.

## Consequences
- **Positive:** Very long-lived workflows no longer accumulate unbounded hot history in a single epoch.
- **Positive:** Recovery, compaction, and projection building remain tractable over time.
- **Positive:** Operators keep a stable business identity for the workflow even as storage rolls to new epochs.
- **Negative:** Queries and debugging now need lineage-aware tooling rather than assuming one ID forever.
