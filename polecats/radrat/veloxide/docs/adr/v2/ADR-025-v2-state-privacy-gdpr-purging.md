# ADR 025 (v2): State Privacy and GDPR Purging

## Status
Accepted

## Context
Because `vo-engine` uses event sourcing and managed-effect journaling, the durable store can contain a complete history of all data that flowed through the system.

Under GDPR, a user has a "Right to Erasure." At the same time, exact-once replay and recovery require the Engine to retain certain canonical facts. A lossy redaction strategy that mutates replay truth before durability breaks exactness.

## Decision
We implement a dual-representation privacy model.

### 1. Canonical Replay Data vs Operator Projection
For every payload-bearing transition, the Engine produces two representations:
1. **Canonical replay data**
   - the exact payload required for deterministic replay, effect reconciliation, and exact-once recovery,
   - stored encrypted at rest,
   - never lossy-redacted before durability if the data may affect routing, retries, or reconciliation.

2. **Operator projection**
   - a redacted JSON view intended for UI, CLI, and default AI consumption,
   - produced by applying the configured `state_filter` recursively,
   - may omit or redact sensitive fields because it is not the source of truth for replay.

### 2. Encryption and Key Lifecycle
- Canonical payload blobs are encrypted with a per-instance data encryption key (DEK).
- The DEK is wrapped by an engine-managed key-encryption key (KEK).
- Operator projections remain redacted and queryable without decrypting canonical state.

### 3. The GDPR Purge Tool
We provide `vo-cli purge --instance <id>`.

Purge performs the following steps:
1. Destroy the per-instance DEK, rendering canonical payload blobs unreadable.
2. Delete redacted operator projections, instance indexes, and payload blob references.
3. Queue physical blob and key removal in Fjall for compaction-time reclamation.

Minimal pseudonymous control-plane facts such as dedupe-key hashes, effect IDs, version hashes, sequence numbers, and external receipts may be retained until their configured retention window expires, because they are required for exact-once correctness and contain no business payload.

## Consequences
- **Positive:** Exact replay truth and privacy no longer fight each other.
- **Positive:** UI, CLI, and AI tooling can default to safe redacted views.
- **Positive:** Crypto-shredding gives GDPR purge a strong answer even on LSM storage where tombstones may live until compaction.
- **Negative:** Key management becomes a real subsystem.
- **Negative:** Some forensic workflows require privileged access to canonical history rather than the default redacted view.
