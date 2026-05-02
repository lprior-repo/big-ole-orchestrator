# ADR 061 (v2): Memory Pressure vs QoS Class Interaction

## Status
Proposed

## Context
The Engine has three independent resource management systems that operate in isolation:

1. **QoS Write Router** (ADR-032, `vo-storage/qos_router.rs`): Routes writes to per-class bounded channels (Critical Control Plane, Operator Projections, Bulk Blobs) with fixed capacities. Prevents queue blocking but has no awareness of system-level memory pressure.

2. **Workload Budget & Degraded Mode** (ADR-013, `vo-core/admission/workload.rs`): Manages slot-based admission for workload classes (Live, Recovery, TimerResume, NonCritical, Background) with a degraded mode state machine (Normal -> Degraded -> Critical) triggered by storage pressure indicators (writer queue depth, blob queue depth, compaction stall, storage stall).

3. **Resource Quotas** (`vo-core/resource_quota/`): Enforces per-namespace CPU, memory, and disk limits via `QuotaEnforcer`. Memory checks are simple: `requested_bytes <= max_bytes`. No QoS class awareness, no pressure-based scaling.

**The gap:** When the host OS approaches memory exhaustion (high RSS, cgroup limits, OOM killer activity), the Engine has no QoS-aware response. All classes compete for memory equally. A bulk blob writer can allocate more memory than a control-plane event, potentially causing the OOM killer to terminate the Engine itself — the very failure mode ADR-006 was designed to prevent.

Additionally, the `MmapCache` (`vo-storage/mmap_cache/mod.rs`) and `ThreadLocalCache` (`vo-storage/thread_local_cache.rs`) enforce absolute memory limits (`max_memory_bytes`) but have no visibility into QoS class — evicting a control-plane cache entry with the same priority as a bulk blob entry.

## Decision
We introduce a **MemoryPressureLevel** enum and couple it to QoS class behavior across all three systems. Memory pressure detection, cache eviction priority, and quota scaling all respect QoS class ordering.

### 1. MemoryPressureLevel Enum

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryPressureLevel {
    /// Normal operation. All classes allocate freely within quotas.
    Normal,
    /// Elevated pressure — approaching memory limits.
    /// Low-priority allocations may be rejected; caches begin evicting.
    Elevated,
    /// Critical pressure — system near OOM.
    /// Only high-priority classes retain memory; caches aggressively evict.
    Critical,
}
```

### 2. Pressure Detection

Pressure level is computed from observable signals (pure function, no I/O):

| Signal | Elevated Threshold | Critical Threshold |
|--------|-------------------|-------------------|
| RSS / cgroup memory usage | >= 75% of limit | >= 90% of limit |
| Available memory pages | < 25% of limit | < 10% of limit |
| OOM killer active on engine process | N/A | Yes |

The `compute_memory_pressure()` function takes these signals and returns `MemoryPressureLevel`:

```rust
#[must_use]
pub fn compute_memory_pressure(rss_percent: f32, available_percent: f32, oom_active: bool) -> MemoryPressureLevel {
    if oom_active || rss_percent >= 90.0 {
        MemoryPressureLevel::Critical
    } else if rss_percent >= 75.0 || available_percent <= 25.0 {
        MemoryPressureLevel::Elevated
    } else {
        MemoryPressureLevel::Normal
    }
}
```

### 3. QoS-Aware Cache Eviction

Both `MmapCache` and `ThreadLocalCache` must associate each cached item with a `WriteClass` (from ADR-032). Eviction under memory pressure follows class priority:

- **Normal:** No pressure-driven eviction. LRU only.
- **Elevated:** Evict `BulkBlob` entries first, then `OperatorProjection`. `CriticalControlPlane` entries are retained until absolutely necessary.
- **Critical:** Only `BulkBlob` entries are evicted. If `BulkBlob` cache is empty and pressure persists, evict `OperatorProjection`. `CriticalControlPlane` entries are **never evicted** — if the control-plane cache is full under critical pressure, new control-plane items are rejected (not evicted to make room).

```rust
/// Returns eviction priority: lower value = evicted first.
#[must_use]
pub fn eviction_priority(class: WriteClass, pressure: MemoryPressureLevel) -> u8 {
    match (class, pressure) {
        (_, MemoryPressureLevel::Normal) => u8::MAX, // No pressure-driven eviction
        (WriteClass::BulkBlob, _) => 0,              // Evict first at any pressure
        (WriteClass::OperatorProjection, MemoryPressureLevel::Elevated) => 1,
        (WriteClass::OperatorProjection, MemoryPressureLevel::Critical) => 1,
        (WriteClass::CriticalControlPlane, MemoryPressureLevel::Elevated) => 2,
        (WriteClass::CriticalControlPlane, MemoryPressureLevel::Critical) => u8::MAX, // Never evict
    }
}
```

### 4. QoS-Aware Quota Scaling

Under memory pressure, the `QuotaEnforcer` scales per-namespace memory quotas based on the namespace's effective QoS class:

| Pressure Level | CriticalControlPlane | OperatorProjection | BulkBlob |
|----------------|---------------------|-------------------|----------|
| Normal | 100% of quota | 100% of quota | 100% of quota |
| Elevated | 100% of quota | 50% of quota | 25% of quota |
| Critical | 100% of quota | 10% of quota | 0% of quota (reject all) |

The `effective_quota()` function scales quotas:

```rust
#[must_use]
pub fn effective_quota(
    base_quota: u64,
    class: WriteClass,
    pressure: MemoryPressureLevel,
) -> u64 {
    let scale = match (class, pressure) {
        (_, MemoryPressureLevel::Normal) => 1.0,
        (WriteClass::CriticalControlPlane, _) => 1.0,
        (WriteClass::OperatorProjection, MemoryPressureLevel::Elevated) => 0.5,
        (WriteClass::OperatorProjection, MemoryPressureLevel::Critical) => 0.1,
        (WriteClass::BulkBlob, MemoryPressureLevel::Elevated) => 0.25,
        (WriteClass::BulkBlob, MemoryPressureLevel::Critical) => 0.0,
    };
    (base_quota as f64 * scale) as u64
}
```

### 5. Integration with Degraded Mode

Memory pressure and storage pressure are orthogonal. The Engine tracks both independently and uses the **more severe** mode:

```rust
#[must_use]
pub fn effective_mode(
    degraded: DegradedMode,
    memory_pressure: MemoryPressureLevel,
) -> SystemMode {
    match (degraded, memory_pressure) {
        (_, MemoryPressureLevel::Normal) => SystemMode::from_degraded(degraded),
        (DegradedMode::Normal, MemoryPressureLevel::Elevated) => SystemMode::MemoryElevated,
        (DegradedMode::Normal, MemoryPressureLevel::Critical) => SystemMode::MemoryCritical,
        (DegradedMode::Degraded { .. }, MemoryPressureLevel::Elevated) => SystemMode::Degraded,
        (DegradedMode::Degraded { .. }, MemoryPressureLevel::Critical) => SystemMode::MemoryCritical,
        (DegradedMode::Critical { .. }, _) => SystemMode::Critical,
    }
}
```

### 6. Memory Pressure Propagation

The `MemoryPressureLevel` flows through the system as a shared, read-only value (e.g., `Arc<AtomicU8>` or a `watch` channel):

- **Storage layer:** `QosRouter` reads pressure level to decide whether to reject enqueues from low-priority classes.
- **Actor layer:** `vo-actor` reads pressure level to scale `MmapCache` eviction aggressiveness.
- **Scheduler layer:** Reads pressure level to adjust `WorkloadBudget` slot allocations under pressure.

### 7. OOM Protection Hardening

In `Critical` memory pressure, the Engine proactively reduces its own memory footprint:

1. Flush and clear `MmapCache` entries for `BulkBlob` and `OperatorProjection` classes.
2. Reject all new `BulkBlob` allocations (return `Err` rather than block).
3. Reduce `ThreadLocalCache` capacity by 90%.
4. Trigger aggressive GC of unused actor state.
5. If pressure persists after these steps, trigger graceful shutdown with `SIGTERM` rather than await the OOM killer.

## Consequences

- **Positive:** High-priority workloads (control-plane events, critical workflow state) survive memory pressure that would otherwise trigger the OOM killer.
- **Positive:** Cache eviction is predictable and class-aware — operators know that under memory pressure, bulk blob cache is shed before control-plane cache.
- **Positive:** Quota scaling provides fine-grained control — operators can configure per-namespace base quotas and the Engine automatically rescales under pressure.
- **Positive:** Memory pressure and storage pressure are tracked orthogonally, preventing one from masking the other.
- **Negative:** `MmapCache` and `ThreadLocalCache` must annotate each entry with a `WriteClass`, adding ~8 bytes per entry.
- **Negative:** `QuotaEnforcer` must track both the namespace's base quota and the current pressure level, adding slight complexity to quota checks.
- **Negative:** Operators must configure memory limits for pressure detection to work (RSS percentage thresholds are relative to a known limit).
- **Negative:** The `Critical` mode shutdown path adds a new failure mode that must be tested — a misconfigured threshold could trigger unnecessary graceful shutdowns.

## Implementation Notes

### Where to implement:
1. **`vo-core/src/memory_pressure/`** — New module for `MemoryPressureLevel`, `compute_memory_pressure()`, and `effective_mode()`.
2. **`vo-storage/src/qos_router.rs`** — Add `MemoryPressureLevel` parameter to `enqueue()` and rejection logic.
3. **`vo-storage/src/mmap_cache/`** — Add `WriteClass` annotation to cache entries; update eviction logic.
4. **`vo-core/src/resource_quota/enforcer.rs`** — Add pressure-aware quota scaling to `check_memory()`.
5. **`vo-core/src/admission/workload.rs`** — Integrate memory pressure into degraded mode state machine.

### Testing:
- Unit tests for `compute_memory_pressure()` with all signal combinations.
- Property tests: under `Critical` pressure, no `BulkBlob` allocations are accepted.
- Integration tests: simulate memory pressure and verify cache eviction order follows QoS class priority.
- Red Queen tests: verify that memory pressure never causes a `CriticalControlPlane` entry to be evicted.

### Related ADRs:
- **ADR-006:** Backpressure and load shedding (execution layer)
- **ADR-013:** System resilience (degraded mode)
- **ADR-032:** Write-path QoS and hot/cold storage (write classes)
- **ADR-033:** Fairness and workload classes (workload class taxonomy)
- **ADR-056:** Quota exhaustion fallback behavior
