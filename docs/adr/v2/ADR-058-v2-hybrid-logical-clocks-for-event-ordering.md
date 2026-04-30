# ADR 058: Hybrid Logical Clocks for Cross-Partition Event Ordering

## Status
Proposed

## Context
Veloxide's event-sourced architecture requires deterministic, causally-correct event ordering across all storage partitions. The current timestamp model uses a single `timestamp_ms: u64` field in `EventEnvelope` that carries a wall-clock value (milliseconds since UNIX epoch, via `TimestampMs::now()`).

This wall-clock model works within a single partition where events are ordered by the per-instance `sequence` field. However, it breaks down when events from multiple partitions, nodes, or recovery paths must be globally ordered:

1. **No causal ordering across partitions.** Two events emitted by different partition actors at wall-clock time `T` have no way to establish which causally preceded the other. The `sequence` field is per-instance and provides no cross-instance or cross-partition ordering.

2. **Clock skew causes ordering errors.** NTP adjustments, wall-clock drift, and time-zone changes can cause two nodes to assign timestamps that violate causal order. An event emitted "after" another can receive an earlier `timestamp_ms` if the receiver's clock is behind.

3. **Recovery creates temporal ambiguity.** During crash recovery (ADR-027), events replayed from snapshots may interleave with newly-emitted events. The replayed events carry their original timestamps; new events get fresh timestamps. Without a logical component, recovery events can appear to precede live events that causally follow them.

4. **ADR-027 requires deterministic ordering.** Section 6 states "No wall-clock time in decisions" and "Parallel fan-out ordering is read from the event log." Deterministic replay depends on a stable, reproducible ordering that does not change based on wall-clock time at replay time.

5. **Existing ADRs partially address clock skew but not event ordering.** ADR-013 handles clock skew for hibernation timers using a dual-clock (absolute + monotonic) approach. ADR-036 treats `issued_at` as "physical timestamp for observability only." Neither ADR addresses the fundamental problem of causally-correct event ordering across partitions.

6. **Causation chains (ADR-051) carry `issued_at_ms` but no causal clock.** The `CollapsedLink.issued_at_ms` field in causation archival is wall-clock only. Chain advancement tracks depth but not logical time, so a deep causation chain cannot be ordered against a shallow one that causally preceded it.

### Why Not Pure Vector Clocks?
Vector clocks provide causal ordering but have three disqualifying problems for Veloxide:

1. **Unbounded growth.** Each vector clock carries one entry per partition node. With N partitions, each event carries O(N) entries. Over thousands of workflows with many partitions, this becomes a storage and serialization burden.

2. **No total order.** Vector clocks provide partial order only. To serialize events into a deterministic log, a tie-breaking scheme (partition ID + sequence) must be layered on top, adding complexity.

3. **Merge cost.** When events from two partitions merge, vector clocks must be element-wise maximized. This is O(N) per merge and must happen during upcasting (ADR-035).

### Why Not Pure Logical Clocks?
Pure logical clocks (Lamport timestamps) advance on every send and receive. They provide total ordering but have two problems:

1. **Loss of temporal fidelity.** A clock that advances only on events provides no information about real elapsed time. Debugging and observability become harder.

2. **Artificial ordering.** Events that are causally unrelated receive an arbitrary order determined by the clock advancement pattern, not by any physical relationship.

## Decision

### 1. Adopt Hybrid Logical Clocks (HLC)

Veloxide adopts Hybrid Logical Clocks as specified by Per-Åke Larson and Mani Sadagopan (2010). An HLC timestamp combines:

- **Physical component (`physical_ms`):** Wall-clock time in milliseconds, providing temporal fidelity for observability.
- **Logical component (`logical`):** A monotonically increasing counter that resolves ties and handles clock skew.

The HLC timestamp format is a two-field struct:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct HlcTimestamp {
    physical_ms: u64,
    logical: u32,
}
```

The total order is the natural `(physical_ms, logical)` lexicographic order.

### 2. HLC Advance Rule

When an event is emitted (whether locally or via recovery), the HLC timestamp advances as follows:

```
new_physics = wall_clock_ms()
if new_physics > current.physical_ms:
    current.physical_ms = new_physics
    current.logical = 0
elif new_physics == current.physical_ms:
    current.logical = current.logical + 1
else:
    // Clock went backwards (skew detected)
    current.physical_ms = current.physical_ms
    current.logical = current.logical + 1
```

This rule guarantees:
- **Monotonicity:** HLC timestamps never decrease.
- **Physical fidelity:** When the clock is synchronized, `physical_ms` reflects real time.
- **Skew resilience:** When the clock goes backward, the logical counter advances without changing `physical_ms`, preserving causal order.
- **Bounded size:** `physical_ms` is u64 (sufficient for ~584 million years from epoch). `logical` is u32 (over 4 billion increments per millisecond before wrap).

### 3. Replace `timestamp_ms` in `EventEnvelope`

The `EventEnvelope` struct must change from:

```rust
pub struct EventEnvelope {
    pub schema_version: u8,
    pub instance_id: String,
    pub sequence: u64,
    pub timestamp_ms: u64,
    pub payload: serde_json::Value,
    pub metadata: EventMetadata,
}
```

To:

```rust
pub struct EventEnvelope {
    pub schema_version: u8,
    pub instance_id: String,
    pub sequence: u64,
    pub hlc_timestamp: HlcTimestamp,
    pub payload: serde_json::Value,
    pub metadata: EventMetadata,
}
```

The `sequence` field remains for per-instance ordering within a partition. The `hlc_timestamp` provides cross-instance, cross-partition, and cross-recovery ordering.

### 4. Global Event Ordering

Events are globally ordered by the tuple:
```
(hlc_timestamp, instance_id, sequence)
```

This ordering is:
- **Deterministic:** Given the same events, the order is identical on every replay.
- **Causally-correct:** If event A causally precedes event B (via causation chain), then `A.hlc_timestamp < B.hlc_timestamp`.
- **Stable across recovery:** Replay does not reassign timestamps; original HLC values are preserved from the durable log.

### 5. HLC State Per Partition Actor

Each partition actor (workflow instance) maintains its own HLC state:
- Initialized to `(0, 0)` on first event.
- Advanced on every event emission (including recovered events).
- The partition actor's HLC is never reset, even across hibernation cycles.
- On crash recovery, the actor's HLC state is restored from the last emitted event's HLC timestamp (read from the event log).

### 6. HLC in Command Metadata

`CommandMetadata.issued_at` changes from `TimestampMs` to `HlcTimestamp`:

```rust
pub struct CommandMetadata {
    pub command_id: IdempotencyKey,
    pub correlation_id: IdempotencyKey,
    pub causation_id: IdempotencyKey,
    pub issuer: Issuer,
    pub issued_at: HlcTimestamp,  // Changed from TimestampMs
}
```

The `issued_at` field was documented as "physical timestamp for observability only" (ADR-036). With HLC, it carries both physical and logical components, providing causal ordering while retaining temporal information.

### 7. HLC in Causation Archival

`CollapsedLink.issued_at_ms` changes from `u64` to `HlcTimestamp`:

```rust
pub struct CollapsedLink {
    pub command_id: String,
    pub causation_id: String,
    pub issued_at: HlcTimestamp,  // Changed from u64
}
```

This ensures causation chain depth validation can correctly order archived links against active events.

### 8. Backward Compatibility via Schema Version Bump

The `EventEnvelope.schema_version` must be bumped (currently `MAX_SUPPORTED_VERSION = 1`). The upcaster (ADR-035) handles migration of existing events:

- **Old events (schema_version < new):** Migrated to the new schema by replacing `timestamp_ms` with an `HlcTimestamp` where `physical_ms = timestamp_ms` and `logical = 0`. This is a safe downgrade because old events already have a stable per-instance `sequence` ordering that is preserved.
- **New events (schema_version >= new):** Use full HLC timestamps.

The upcaster must preserve the original `sequence` field to maintain per-instance ordering during and after migration.

### 9. Overflow Handling

The `logical` counter is `u32`, providing ~4.3 billion increments per millisecond. Overflow wraps to zero. When `logical` wraps:
1. The `physical_ms` component is advanced by 1ms (even if wall clock has not advanced).
2. `logical` resets to 0.

This prevents wrap-around from causing timestamp regression. In practice, overflow requires emitting more than 4.3 billion events per millisecond, which is far beyond any realistic throughput.

### 10. Observability: Physical Time Exposure

For logging, metrics, and UI display, the `physical_ms` component is extracted and converted to human-readable form:

```rust
impl HlcTimestamp {
    pub fn physical_time(self) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_millis(self.physical_ms)
    }
}
```

The `logical` component is surfaced in debug/audit contexts to explain ordering among events at the same physical time.

## Consequences

- **Positive:** Events are causally-correctly ordered across all partitions, recovery paths, and crash-restart cycles without requiring NTP synchronization.
- **Positive:** HLC timestamps are bounded and fixed-size (12 bytes: 8 + 4), unlike vector clocks which grow with partition count.
- **Positive:** Deterministic replay (ADR-027) is preserved because HLC ordering is purely a function of the event data, not of wall-clock time at replay.
- **Positive:** Observability is maintained through the `physical_ms` component, which reflects real elapsed time when clocks are synchronized.
- **Positive:** Clock skew is handled gracefully without system errors — the logical counter absorbs backward time jumps.
- **Negative:** `EventEnvelope` schema must be bumped and all existing events must be upcast (ADR-035).
- **Negative:** `TimestampMs` type is superseded by `HlcTimestamp` in event-related code, requiring updates across `events/envelope.rs`, `command_metadata.rs`, and `causation_chain.rs`.
- **Negative:** Partition actors must maintain and restore HLC state across hibernation and crash recovery.
- **Negative:** The `TimestampMs` type remains useful for non-event uses (e.g., timer `fire_at_ms`, duration calculations), so it is not removed — only its event-related usages are replaced.

## Non-Decision

- **No distributed clock synchronization protocol.** HLC handles skew locally; no NTP correction or distributed consensus on time is required.
- **No vector clock fallback.** HLC is sufficient for Veloxide's single-node architecture. Vector clocks are not needed unless distributed clustering is added in the future.
- **No change to `fire_at_ms` in timer events.** Timer fire times are absolute wall-clock targets, not ordering keys. ADR-013's dual-clock timer verification remains the correct approach for timer correctness.
- **No change to per-instance `sequence` field.** The `sequence` field remains the primary intra-instance ordering mechanism. HLC provides the inter-instance ordering layer.

## References

- ADR-013: System Resilience (dual-clock timer verification for hibernation)
- ADR-027: Deterministic Replay and Exactly-Once Core Semantics
- ADR-035: Event Schema Evolution and Upcasting
- ADR-036: Command Identity, Correlation, and Causation
- ADR-051: Causation Chain Truncation and Archival
- Larson, Per-Åke and Sadagopan, Mani H. "Hybrid Logical Clocks: Guaranteed Latency for Highly Available Systems." ICDE 2010.
- `crates/vo-types/src/events/envelope.rs`: Current `EventEnvelope` with `timestamp_ms: u64`
- `crates/vo-types/src/command_metadata.rs`: Current `CommandMetadata.issued_at: TimestampMs`
- `crates/vo-types/src/causation_chain.rs`: Current `CollapsedLink.issued_at_ms: u64`
- `crates/vo-types/src/integer_types.rs`: Current `TimestampMs` definition
