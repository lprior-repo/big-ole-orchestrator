## Contract: Resource Quota Enforcer

### 1. Purpose

Defines the contract for resource quota enforcement in the veloxide event-sourced actor system. This contract establishes the types, invariants, and error taxonomy for per-namespace resource quota checking and enforcement.

### 2. Source ADRs

- `docs/adr/v2/ADR-033-v2-fairness-and-workload-classes.md` (fairness baseline, overcommit policy)

### 3. Resource Types

#### 3.1 ResourceKind

Represents the category of resource being quota-controlled.

```rust
enum ResourceKind {
    Cpu,     // CPU core count
    Memory,  // Memory in bytes
    Disk,    // Disk storage in bytes
}
```

#### 3.2 Individual Quota Types

```rust
struct CpuQuota {
    max_cores: NonZeroU64,  // Must be >= 1
}

struct MemoryQuota {
    max_bytes: NonZeroU64,  // Must be >= 1
}

struct DiskQuota {
    max_bytes: NonZeroU64,  // Must be >= 1
}
```

#### 3.3 NamespaceQuota

Aggregates all quotas for a single namespace with overcommit policy.

```rust
struct NamespaceQuota {
    namespace: String,                    // Unique identifier
    cpu: Option<CpuQuota>,                // Optional CPU limit
    memory: Option<MemoryQuota>,           // Optional memory limit
    disk: Option<DiskQuota>,               // Optional disk limit
    overcommit: OvercommitPolicy,          // Overcommit behavior
}
```

#### 3.4 QuotaUsage

Tracks current resource consumption for a namespace.

```rust
struct QuotaUsage {
    cpu_cores_used: u64,
    memory_bytes_used: u64,
    disk_bytes_used: u64,
}
```

### 4. Overcommit Policy

```rust
enum OvercommitPolicy {
    NoOvercommit,      // Hard limit; requests above quota are rejected
    AllowOvercommit,    // Requests above quota are allowed (best-effort)
}
```

Default: `NoOvercommit`

### 5. Invariants (INV-*)

- **INV-001**: Namespace names are unique within a `NamespaceRegistry`
- **INV-002**: All `NonZeroU64` values in quotas must be >= 1
- **INV-003**: `check_*` methods return `Ok(())` when `requested <= limit`
- **INV-004**: `check_*` methods return `QuotaExceeded` when `requested > limit` AND `overcommit == NoOvercommit`
- **INV-005**: `check_*` methods return `Ok(())` when `requested > limit` AND `overcommit == AllowOvercommit`
- **INV-006**: `check_*` methods return `NamespaceNotFound` when namespace does not exist in registry
- **INV-007**: `check_*` methods return `QuotaNotConfigured` when the specific resource quota is `None` for the namespace
- **INV-008**: A namespace may have any subset of {cpu, memory, disk} configured; missing resources are not enforced
- **INV-009**: `QuotaUsage` values are monotonic: they only increase within a billing cycle
- **INV-010**: Default namespace "default" is created with sensible defaults on `QuotaEnforcer::with_default_namespace()`
- **INV-011**: Overcommit policy is per-namespace, not per-resource
- **INV-012**: `is_overcommit_rejected()` returns `true` for `QuotaExceeded` and `QuotaNotConfigured`, `false` for `NamespaceNotFound`

### 6. Error Taxonomy

```rust
enum QuotaError {
    // Requested resource exceeds configured limit
    QuotaExceeded {
        resource: ResourceKind,
        namespace: String,
        requested: u64,
        available: u64,
    },

    // Namespace does not exist in the registry
    NamespaceNotFound(String),

    // Resource quota not configured for this namespace
    QuotaNotConfigured {
        resource: ResourceKind,
        namespace: String,
    },
}
```

#### 6.1 Error Categories

| Error Variant | Category | Recoverable |
|--------------|----------|-------------|
| `QuotaExceeded` | LimitViolation | Yes (wait/retry) |
| `NamespaceNotFound` | ConfigurationError | Yes (register namespace first) |
| `QuotaNotConfigured` | ConfigurationError | Yes (configure quota first) |

#### 6.2 Error Display Format

- `QuotaExceeded`: "quota exceeded for {resource} in namespace {namespace}: requested {requested}, available {available}"
- `NamespaceNotFound`: "namespace {0} not found"
- `QuotaNotConfigured`: "quota not configured for {resource} in namespace {namespace}"

### 7. Enforcement Protocol

1. **Register**: Add namespace quota to registry via `NamespaceRegistry::register()`
2. **Check CPU**: `QuotaEnforcer::check_cpu(namespace, requested_cores) -> Result<(), QuotaError>`
3. **Check Memory**: `QuotaEnforcer::check_memory(namespace, requested_bytes) -> Result<(), QuotaError>`
4. **Check Disk**: `QuotaEnforcer::check_disk(namespace, requested_bytes) -> Result<(), QuotaError>`

Each check follows this logic:
```
1. Look up namespace in registry
   → Not found: return NamespaceNotFound
2. Look up specific resource quota
   → Not configured: return QuotaNotConfigured
3. Compare requested vs limit
   → Within limit: return Ok(())
   → Over limit AND NoOvercommit: return QuotaExceeded
   → Over limit AND AllowOvercommit: return Ok(()) [overcommit allowed]
```

### 8. Constraints

- Quota enforcement is synchronous; no async or background verification
- No support for quota reservations or "soft" limits separate from hard limits
- Overcommit policy applies to all resources in a namespace uniformly
- Registry modifications (register/remove) are not atomic with respect to checks
- Thread safety: `QuotaEnforcer` and `NamespaceRegistry` require external synchronization for concurrent access

### 9. Relevant Files

- `crates/vo-core/src/resource_quota/mod.rs` (types, errors, builder API)
- `crates/vo-core/src/resource_quota/enforcer.rs` (NamespaceRegistry, QuotaEnforcer)
- `crates/vo-core/src/resource_quota/policy.rs` (OvercommitPolicy)
- `crates/vo-types/src/integer_types.rs` (NonZeroU64 patterns)

### 10. Acceptance Criteria

- [ ] All types (CpuQuota, MemoryQuota, DiskQuota, NamespaceQuota, ResourceKind, OvercommitPolicy) compile and are well-formed
- [ ] All invariants (INV-001 through INV-012) are formally stated and testable
- [ ] Error taxonomy is exhaustive: every failure mode has a corresponding error variant
- [ ] Overcommit policy behavior is consistent across all three resource types
- [ ] Default namespace creation provides sensible defaults (4 cores, 8GB memory, 100GB disk)
- [ ] Error display messages include all relevant context (resource, namespace, requested, available)
- [ ] Contract is self-contained and does not reference nonexistent crates or files
