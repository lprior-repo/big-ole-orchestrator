# ADR 032 (v2): Write-Path QoS and Hot/Cold Storage

## Status
Accepted

## Context
The single `DbWriterActor` is both the strength and the main risk of a throughput-first single-node engine. If all writes are treated equally, large payloads, stderr blobs, snapshots, and secondary projections can consume the same flush budget as exact-once control records.

## Decision
We classify writes into explicit QoS classes.

### 1. Write Classes
1. **Critical Control Plane**
   - events,
   - instances,
   - dedupe,
   - effects,
   - leases,
   - timers,
   - snapshots.

2. **Operator Projections**
   - dashboard views,
   - redacted history enrichments,
   - UI convenience indexes.

3. **Bulk Blobs**
   - large canonical payloads,
   - bounded stderr blobs,
   - optional large outputs.

### 2. Service Policy
- Critical control-plane writes are never dropped.
- Operator projections may lag and be rebuilt or reconciled later.
- Bulk blobs are written through bounded queues and may be deferred under pressure, but replay-critical canonical blobs may not be published into control-plane records until their durability boundary is satisfied (ADR-040).

### 3. Admission Coupling
Ingress shedding and degraded mode must consider:
1. writer queue depth,
2. batch commit latency,
3. blob queue depth,
4. compaction or storage stall indicators.

## Consequences
- **Positive:** Exact-once control records remain protected when the system is under heavy observability or payload pressure.
- **Positive:** Large outputs and logs no longer automatically dominate the hot path.
- **Negative:** Some UI history and blob visibility may become eventually consistent under stress.
