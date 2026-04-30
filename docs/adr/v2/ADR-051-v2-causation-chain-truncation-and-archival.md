# ADR 051 (v2): Causation Chain Truncation and Archival

## Status
Accepted

## Context
ADR-036 introduced `causation_id` as a pointer to the immediate parent event or command, enabling full traceability through business flows. However, no bounds were placed on chain depth, and no strategy exists for handling chains that grow unbounded or become broken.

Without limits, deeply nested causation chains from long-running workflows, retries, compensations, and nested AI agent loops can grow to thousands of links. This causes:
- Storage exhaustion from chain metadata in every event
- Query degradation when reconstructing lineage
- Memory pressure during chain traversal for debugging
- No visibility into chain health (broken references go undetected)

The event-sourcing ecosystem handles this with bounded lineage. We should steal the pattern directly.

## Decision

### 1. Max Chain Depth
The system enforces a configurable maximum causation chain depth of **128 links**. This bound covers:
- Normal workflow execution (typically 5-20 links)
- Nested AI agent loops (typically 10-50 links)
- Retry cascades with compensation (typically 10-100 links)
- Headroom for future complexity

When a chain would exceed 128 links, the oldest segment is collapsed.

### 2. Collapse Strategy
When chain depth exceeds the configured maximum:
1. The system identifies the oldest event in the chain (the root causation anchor)
2. It replaces the oldest causation link with a reference to an **archival blob** containing the collapsed segment
3. The new event's `causation_id` points to the event at depth `MAX_DEPTH - 1`
4. The archival blob preserves the full original chain for forensic audit

The collapse is transparent to normal operation - chain reconstruction still works, just with an archival lookup for the deepest segment.

### 3. Broken Chain Detection
The system detects and alerts on broken causation chains:
- **Detection trigger**: When any event's `causation_id` references an event that cannot be found in the store
- **Action**: Log an integrity violation and emit an alert event
- **Recovery**: The broken link is recorded with `causation_id = "archived:<blob-ref>"` or `causation_id = "unknown:<original-id>"` if the archival lookup also fails
- **Alert scope**: Broken chain detection runs as part of the periodic integrity check, and immediately on workflow replay

### 4. Archival Format
Collapsed chain segments are stored as immutable blobs:
```json
{
  "type": "causation_archival",
  "segment_id": "<unique-segment-id>",
  "original_depth": <total depth before collapse>,
  "collapsed_links": [
    {"command_id": "...", "causation_id": "...", "timestamp_ms": ...},
    ...
  ],
  "preserved_anchor": "<causation_id of the link that remains in the active chain>"
}
```

## Consequences
- **Positive**: Chain metadata growth is bounded, preventing storage exhaustion
- **Positive**: Query performance remains stable regardless of workflow age
- **Positive**: Broken chains are detected and logged, preventing silent lineage loss
- **Positive**: Full chain history is preserved in archival blobs for audit/debugging
- **Negative**: Chain reconstruction requires an archival lookup for collapsed segments
- **Negative**: Increased complexity in event store queries that traverse causation chains
