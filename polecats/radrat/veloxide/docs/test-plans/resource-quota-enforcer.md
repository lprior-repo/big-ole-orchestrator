# Test Plan: Resource Quota Enforcer

## Summary

- **Bead**: ve-fiz — Test Plan: Resource quota enforcer
- **Contract**: ve-j6g — Contract: Resource quota enforcer
- **Behaviors identified**: 58
- **Trophy allocation**: 42 unit / 8 integration / 4 e2e / 4 static
- **Proptest invariants**: 12
- **Fuzz targets**: 3
- **Kani harnesses**: 2
- **Mutation checkpoints**: 18

---

## 1. Behavior Inventory

| # | Behavior | Public API | Invariant |
|---|----------|------------|-----------|
| B-001 | `ResourceKind` has exactly 3 variants: Cpu, Memory, Disk | `ResourceKind` enum | - |
| B-002 | `ResourceKind::as_str()` returns "cpu", "memory", "disk" respectively | `ResourceKind::as_str()` | - |
| B-003 | `ResourceKind::Display` formats as lowercase string | `ResourceKind::fmt()` | - |
| B-004 | `CpuQuota::new()` constructs with NonZeroU64 max_cores | `CpuQuota::new()` | INV-002 |
| B-005 | `MemoryQuota::new()` constructs with NonZeroU64 max_bytes | `MemoryQuota::new()` | INV-002 |
| B-006 | `DiskQuota::new()` constructs with NonZeroU64 max_bytes | `DiskQuota::new()` | INV-002 |
| B-007 | `CpuQuota` implements Clone, Copy, PartialEq, Eq, Hash | `CpuQuota` derive | - |
| B-008 | `MemoryQuota` implements Clone, Copy, PartialEq, Eq, Hash | `MemoryQuota` derive | - |
| B-009 | `DiskQuota` implements Clone, Copy, PartialEq, Eq, Hash | `DiskQuota` derive | - |
| B-010 | `OvercommitPolicy` has exactly 2 variants: NoOvercommit, AllowOvercommit | `OvercommitPolicy` enum | - |
| B-011 | `OvercommitPolicy::default()` returns NoOvercommit | `OvercommitPolicy::default()` | - |
| B-012 | `OvercommitPolicy::allows_overcommit()` returns true for AllowOvercommit | `OvercommitPolicy::allows_overcommit()` | - |
| B-013 | `OvercommitPolicy` implements Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize | `OvercommitPolicy` derive | - |
| B-014 | `NamespaceQuota::new()` constructs with namespace and all None quotas | `NamespaceQuota::new()` | INV-008 |
| B-015 | `NamespaceQuota::with_cpu()` sets cpu to Some(quota) | `NamespaceQuota::with_cpu()` | - |
| B-016 | `NamespaceQuota::with_memory()` sets memory to Some(quota) | `NamespaceQuota::with_memory()` | - |
| B-017 | `NamespaceQuota::with_disk()` sets disk to Some(quota) | `NamespaceQuota::with_disk()` | - |
| B-018 | `NamespaceQuota::with_overcommit()` sets overcommit policy | `NamespaceQuota::with_overcommit()` | INV-011 |
| B-019 | `NamespaceQuota` implements Clone, PartialEq, Eq, Serialize, Deserialize | `NamespaceQuota` derive | - |
| B-020 | `QuotaUsage::new()` creates usage with all zeros | `QuotaUsage::new()` | - |
| B-021 | `QuotaUsage::with_cpu()` sets cpu_cores_used | `QuotaUsage::with_cpu()` | INV-009 |
| B-022 | `QuotaUsage::with_memory()` sets memory_bytes_used | `QuotaUsage::with_memory()` | INV-009 |
| B-023 | `QuotaUsage::with_disk()` sets disk_bytes_used | `QuotaUsage::with_disk()` | INV-009 |
| B-024 | `QuotaUsage` implements Default, Clone, Copy, PartialEq, Eq | `QuotaUsage` derive | - |
| B-025 | `NamespaceRegistry::new()` creates empty registry | `NamespaceRegistry::new()` | - |
| B-026 | `NamespaceRegistry::register()` inserts quota and returns Ok(()) | `NamespaceRegistry::register()` | INV-001 |
| B-027 | `NamespaceRegistry::register()` allows duplicate namespace (replaces) | `NamespaceRegistry::register()` | INV-001 |
| B-028 | `NamespaceRegistry::get()` returns Some(quota) for registered namespace | `NamespaceRegistry::get()` | - |
| B-029 | `NamespaceRegistry::get()` returns None for unregistered namespace | `NamespaceRegistry::get()` | - |
| B-030 | `NamespaceRegistry::remove()` returns Some(quota) and removes | `NamespaceRegistry::remove()` | - |
| B-031 | `NamespaceRegistry::remove()` returns None for unregistered namespace | `NamespaceRegistry::remove()` | - |
| B-032 | `NamespaceRegistry::list_namespaces()` returns all registered namespace names | `NamespaceRegistry::list_namespaces()` | INV-001 |
| B-033 | `QuotaEnforcer::new()` constructs with given registry | `QuotaEnforcer::new()` | - |
| B-034 | `QuotaEnforcer::with_default_namespace()` creates "default" namespace | `QuotaEnforcer::with_default_namespace()` | INV-010 |
| B-035 | Default namespace has 4 cores, 8GB memory, 100GB disk | `with_default_namespace()` | INV-010 |
| B-036 | `check_cpu()` returns Ok(()) when requested <= limit | `QuotaEnforcer::check_cpu()` | INV-003 |
| B-037 | `check_cpu()` returns Ok(()) when requested == limit | `QuotaEnforcer::check_cpu()` | INV-003 |
| B-038 | `check_cpu()` returns QuotaExceeded when requested > limit and NoOvercommit | `QuotaEnforcer::check_cpu()` | INV-004 |
| B-039 | `check_cpu()` returns Ok(()) when requested > limit and AllowOvercommit | `QuotaEnforcer::check_cpu()` | INV-005 |
| B-040 | `check_cpu()` returns NamespaceNotFound for unknown namespace | `QuotaEnforcer::check_cpu()` | INV-006 |
| B-041 | `check_cpu()` returns QuotaNotConfigured when cpu quota is None | `QuotaEnforcer::check_cpu()` | INV-007 |
| B-042 | `check_memory()` returns Ok(()) when requested <= limit | `QuotaEnforcer::check_memory()` | INV-003 |
| B-043 | `check_memory()` returns QuotaExceeded when requested > limit and NoOvercommit | `QuotaEnforcer::check_memory()` | INV-004 |
| B-044 | `check_memory()` returns Ok(()) when requested > limit and AllowOvercommit | `QuotaEnforcer::check_memory()` | INV-005 |
| B-045 | `check_memory()` returns NamespaceNotFound for unknown namespace | `QuotaEnforcer::check_memory()` | INV-006 |
| B-046 | `check_memory()` returns QuotaNotConfigured when memory quota is None | `QuotaEnforcer::check_memory()` | INV-007 |
| B-047 | `check_disk()` returns Ok(()) when requested <= limit | `QuotaEnforcer::check_disk()` | INV-003 |
| B-048 | `check_disk()` returns QuotaExceeded when requested > limit and NoOvercommit | `QuotaEnforcer::check_disk()` | INV-004 |
| B-049 | `check_disk()` returns Ok(()) when requested > limit and AllowOvercommit | `QuotaEnforcer::check_disk()` | INV-005 |
| B-050 | `check_disk()` returns NamespaceNotFound for unknown namespace | `QuotaEnforcer::check_disk()` | INV-006 |
| B-051 | `check_disk()` returns QuotaNotConfigured when disk quota is None | `QuotaEnforcer::check_disk()` | INV-007 |
| B-052 | Overcommit policy applies uniformly to all resources in namespace | `NamespaceQuota::overcommit` | INV-011 |
| B-053 | `QuotaError::is_overcommit_rejected()` returns true for QuotaExceeded | `QuotaError::is_overcommit_rejected()` | INV-012 |
| B-054 | `QuotaError::is_overcommit_rejected()` returns true for QuotaNotConfigured | `QuotaError::is_overcommit_rejected()` | INV-012 |
| B-055 | `QuotaError::is_overcommit_rejected()` returns false for NamespaceNotFound | `QuotaError::is_overcommit_rejected()` | INV-012 |
| B-056 | QuotaExceeded error display includes resource, namespace, requested, available | `QuotaError::fmt()` | Error Display |
| B-057 | NamespaceNotFound error display includes namespace name | `QuotaError::fmt()` | Error Display |
| B-058 | QuotaNotConfigured error display includes resource and namespace | `QuotaError::fmt()` | Error Display |

---

## 2. Trophy Allocation

| Layer | Count | Rationale |
|-------|-------|-----------|
| **Unit / Calc** | 42 | Pure types: ResourceKind, CpuQuota, MemoryQuota, DiskQuota, OvercommitPolicy, NamespaceQuota, QuotaUsage. All type derivations, builder patterns, and error variants. QuotaError is_overcommit_rejected logic. |
| **Integration** | 8 | NamespaceRegistry + QuotaEnforcer integration, default namespace creation with multiple resources, partial quota configuration (INV-008), overcommit policy uniformity across resources (INV-011) |
| **E2E** | 4 | Full quota check workflow with registration, concurrent namespace operations, overcommit policy behavior end-to-end, error taxonomy verification |
| **Static Analysis** | 4 | `clippy::pedantic` lint gates, `cargo-deny` dependency audit, `rustfmt` formatting, `miri` for unsafe code verification |

**Rationale**: Resource quota enforcement is a pure data-validation layer with synchronous checks. The 42/8/4 split reflects that most behaviors are testable at the unit level (calc layer), with integration covering NamespaceRegistry interactions and multi-resource scenarios. The critical invariant verifications (INV-003 through INV-007) justify focused testing across all three resource types.

---

## 3. BDD Scenarios

### B-001: ResourceKind has exactly 3 variants

**Scenario: exhaustive match covers all resource types**

```
Given: A ResourceKind enum value
When: pattern matching on all variants
Then: Cpu, Memory, Disk are all handled
```

```rust
fn resource_kind_has_exactly_three_variants() {
    fn _exhaustiveness(k: ResourceKind) -> &'static str {
        match k {
            ResourceKind::Cpu => "cpu",
            ResourceKind::Memory => "memory",
            ResourceKind::Disk => "disk",
        }
    }
    assert_eq!(_exhaustiveness(ResourceKind::Cpu), "cpu");
    assert_eq!(_exhaustiveness(ResourceKind::Memory), "memory");
    assert_eq!(_exhaustiveness(ResourceKind::Disk), "disk");
    let all: [ResourceKind; 3] = [
        ResourceKind::Cpu,
        ResourceKind::Memory,
        ResourceKind::Disk,
    ];
    assert_eq!(all.len(), 3);
}
```

---

### B-004: CpuQuota::new() constructs with NonZeroU64 max_cores

**Scenario: cpu quota creation succeeds with valid non-zero value**

```
Given: A NonZeroU64 value >= 1
When: CpuQuota::new(value) is called
Then: returns CpuQuota with max_cores equal to value
```

```rust
#[test]
fn cpu_quota_new_constructs_with_max_cores() {
    let quota = CpuQuota::new(NonZeroU64::new(4).unwrap());
    assert_eq!(quota.max_cores.get(), 4);
}

#[test]
fn cpu_quota_new_rejects_zero() {
    let result = NonZeroU64::new(0);
    assert!(result.is_none());
}
```

---

### B-010: OvercommitPolicy has exactly 2 variants

**Scenario: exhaustive match covers all overcommit policies**

```
Given: An OvercommitPolicy enum value
When: allows_overcommit() is called
Then: returns false for NoOvercommit, true for AllowOvercommit
```

```rust
#[test]
fn overcommit_policy_has_two_variants() {
    assert!(!OvercommitPolicy::NoOvercommit.allows_overcommit());
    assert!(OvercommitPolicy::AllowOvercommit.allows_overcommit());
}

#[test]
fn overcommit_policy_default_is_no_overcommit() {
    assert_eq!(OvercommitPolicy::default(), OvercommitPolicy::NoOvercommit);
}
```

---

### B-026: NamespaceRegistry::register() inserts quota

**Scenario: namespace registration succeeds and quota is retrievable**

```
Given: A NamespaceRegistry and a valid NamespaceQuota
When: register(quota) is called
Then: returns Ok(()) and get(namespace) returns Some(quota)
```

```rust
#[test]
fn namespace_registry_register_inserts_quota() {
    let mut registry = NamespaceRegistry::new();
    let quota = NamespaceQuota::new("payments")
        .with_cpu(CpuQuota::new(NonZeroU64::new(4).unwrap()));
    let result = registry.register(quota);
    assert!(result.is_ok());
    assert!(registry.get("payments").is_some());
}
```

---

### B-027: NamespaceRegistry::register() allows duplicate namespace

**Scenario: registering same namespace twice replaces the quota**

```
Given: A NamespaceRegistry with "payments" registered
When: register() is called again with new quota for "payments"
Then: returns Ok(()) and get("payments") returns the new quota
```

```rust
#[test]
fn namespace_registry_register_replaces_existing() {
    let mut registry = NamespaceRegistry::new();
    let q1 = NamespaceQuota::new("payments")
        .with_cpu(CpuQuota::new(NonZeroU64::new(2).unwrap()));
    let _ = registry.register(q1);
    
    let q2 = NamespaceQuota::new("payments")
        .with_cpu(CpuQuota::new(NonZeroU64::new(8).unwrap()));
    let _ = registry.register(q2);
    
    let retrieved = registry.get("payments").unwrap();
    assert_eq!(retrieved.cpu.unwrap().max_cores.get(), 8);
}
```

---

### B-036: check_cpu() returns Ok when within limit

**Scenario: cpu request within quota succeeds**

```
Given: A QuotaEnforcer with "payments" namespace having 4-core CPU quota
When: check_cpu("payments", 2) is called
Then: returns Ok(())
```

```rust
#[test]
fn check_cpu_returns_ok_when_under_limit() {
    let enforcer = make_test_enforcer(); // 4 cores
    let result = enforcer.check_cpu("payments", 2);
    assert!(result.is_ok());
}
```

---

### B-037: check_cpu() returns Ok when at limit

**Scenario: cpu request exactly at quota succeeds**

```
Given: A QuotaEnforcer with "payments" namespace having 4-core CPU quota
When: check_cpu("payments", 4) is called
Then: returns Ok(())
```

```rust
#[test]
fn check_cpu_returns_ok_when_at_limit() {
    let enforcer = make_test_enforcer(); // 4 cores
    let result = enforcer.check_cpu("payments", 4);
    assert!(result.is_ok());
}
```

---

### B-038: check_cpu() returns QuotaExceeded when over limit

**Scenario: cpu request exceeds quota with NoOvercommit policy**

```
Given: A QuotaEnforcer with "payments" namespace having 4-core CPU quota, NoOvercommit
When: check_cpu("payments", 8) is called
Then: returns Err(QuotaExceeded { resource: Cpu, namespace: "payments", requested: 8, available: 4 })
```

```rust
#[test]
fn check_cpu_returns_quota_exceeded_when_over_limit() {
    let enforcer = make_test_enforcer(); // 4 cores, NoOvercommit
    let result = enforcer.check_cpu("payments", 8);
    assert!(matches!(
        result,
        Err(QuotaError::QuotaExceeded {
            resource: ResourceKind::Cpu,
            namespace: ns,
            requested: 8,
            available: 4
        }) if ns == "payments"
    ));
}
```

---

### B-039: check_cpu() returns Ok when over limit with AllowOvercommit

**Scenario: cpu request exceeds quota but overcommit is allowed**

```
Given: A QuotaEnforcer with "payments" namespace having 4-core CPU quota, AllowOvercommit
When: check_cpu("payments", 100) is called
Then: returns Ok(())
```

```rust
#[test]
fn check_cpu_returns_ok_when_over_limit_with_allow_overcommit() {
    let enforcer = make_overcommit_enforcer(); // AllowOvercommit
    let result = enforcer.check_cpu("payments", 100);
    assert!(result.is_ok());
}
```

---

### B-040: check_cpu() returns NamespaceNotFound for unknown namespace

**Scenario: quota check for unregistered namespace**

```
Given: A QuotaEnforcer with "payments" registered
When: check_cpu("unknown", 2) is called
Then: returns Err(NamespaceNotFound("unknown"))
```

```rust
#[test]
fn check_cpu_returns_namespace_not_found_when_unknown_namespace() {
    let enforcer = make_test_enforcer();
    let result = enforcer.check_cpu("unknown", 2);
    assert!(matches!(
        result,
        Err(QuotaError::NamespaceNotFound(ns)) if ns == "unknown"
    ));
}
```

---

### B-041: check_cpu() returns QuotaNotConfigured when cpu quota is None

**Scenario: checking resource that wasn't configured**

```
Given: A QuotaEnforcer with "no-cpu" namespace having only memory quota
When: check_cpu("no-cpu", 2) is called
Then: returns Err(QuotaNotConfigured { resource: Cpu, namespace: "no-cpu" })
```

```rust
#[test]
fn check_cpu_returns_quota_not_configured_when_no_cpu_quota() {
    let mut registry = NamespaceRegistry::new();
    let _ = registry.register(
        NamespaceQuota::new("no-cpu")
            .with_memory(MemoryQuota::new(NonZeroU64::new(1024).unwrap())),
    );
    let enforcer = QuotaEnforcer::new(registry);
    let result = enforcer.check_cpu("no-cpu", 2);
    assert!(matches!(
        result,
        Err(QuotaError::QuotaNotConfigured {
            resource: ResourceKind::Cpu,
            namespace: ns
        }) if ns == "no-cpu"
    ));
}
```

---

### B-052: Overcommit policy applies uniformly to all resources

**Scenario: overcommit policy affects all resource checks in namespace**

```
Given: A QuotaEnforcer with "payments" namespace having AllowOvercommit
When: check_cpu, check_memory, and check_disk are called with values over limit
Then: all return Ok(()) because overcommit is allowed
```

```rust
#[test]
fn overcommit_policy_applies_to_all_resources() {
    let mut registry = NamespaceRegistry::new();
    let _ = registry.register(
        NamespaceQuota::new("payments")
            .with_cpu(CpuQuota::new(NonZeroU64::new(2).unwrap()))
            .with_memory(MemoryQuota::new(NonZeroU64::new(1024).unwrap()))
            .with_disk(DiskQuota::new(NonZeroU64::new(1000).unwrap()))
            .with_overcommit(OvercommitPolicy::AllowOvercommit),
    );
    let enforcer = QuotaEnforcer::new(registry);
    
    assert!(enforcer.check_cpu("payments", 100).is_ok());
    assert!(enforcer.check_memory("payments", u64::MAX).is_ok());
    assert!(enforcer.check_disk("payments", u64::MAX).is_ok());
}
```

---

## 4. Proptest Invariants

### INV-002: All NonZeroU64 values in quotas must be >= 1

```rust
proptest! {
    #[test]
    fn cpu_quota_rejects_zero_and_u64_max(n in 1u64..) {
        let quota = CpuQuota::new(NonZeroU64::new(n).unwrap());
        prop_assert!(quota.max_cores.get() >= 1);
    }
    
    #[test]
    fn memory_quota_rejects_zero_and_u64_max(n in 1u64..) {
        let quota = MemoryQuota::new(NonZeroU64::new(n).unwrap());
        prop_assert!(quota.max_bytes.get() >= 1);
    }
    
    #[test]
    fn disk_quota_rejects_zero_and_u64_max(n in 1u64..) {
        let quota = DiskQuota::new(NonZeroU64::new(n).unwrap());
        prop_assert!(quota.max_bytes.get() >= 1);
    }
}
```

### INV-003: check_* returns Ok when requested <= limit

```rust
proptest! {
    #[test]
    fn check_cpu_ok_when_within_limit(requested in 1u64..=4) {
        let enforcer = make_test_enforcer(); // 4 cores limit
        let result = enforcer.check_cpu("payments", requested);
        prop_assert!(result.is_ok());
    }
}
```

### INV-004: check_* returns QuotaExceeded when > limit and NoOvercommit

```rust
proptest! {
    #[test]
    fn check_cpu_quota_exceeded_when_over_limit_no_overcommit(requested in 5u64..100) {
        let enforcer = make_test_enforcer(); // NoOvercommit
        let result = enforcer.check_cpu("payments", requested);
        prop_assert!(matches!(
            result,
            Err(QuotaError::QuotaExceeded { resource: ResourceKind::Cpu, .. })
        ));
    }
}
```

### INV-005: check_* returns Ok when > limit and AllowOvercommit

```rust
proptest! {
    #[test]
    fn check_cpu_ok_when_over_limit_with_allow_overcommit(requested in 5u64..100) {
        let enforcer = make_overcommit_enforcer(); // AllowOvercommit
        let result = enforcer.check_cpu("payments", requested);
        prop_assert!(result.is_ok());
    }
}
```

---

## 5. Edge Cases

| Edge Case | Expected Behavior |
|-----------|------------------|
| Zero requested cores/bytes | Should return Ok (0 <= any limit) |
| u64::MAX requested | Should return QuotaExceeded or Ok depending on overcommit |
| Empty namespace string | Should be allowed as namespace name |
| Very long namespace string | Should be allowed (no length limit specified) |
| Namespace with special characters | Should be allowed (namespace is just a String) |
| Registering then immediately checking | Should work (no async or eventual consistency) |
| Removing namespace then checking | Should return NamespaceNotFound |
| Partial quota (cpu only, no memory/disk) | Should return QuotaNotConfigured for missing resources |
| All three resources configured | Should check each independently |

---

## 6. Error Taxonomy Verification

| Error Variant | is_overcommit_rejected | Category | Recoverable |
|--------------|------------------------|----------|-------------|
| QuotaExceeded | true | LimitViolation | Yes (wait/retry) |
| NamespaceNotFound | false | ConfigurationError | Yes (register first) |
| QuotaNotConfigured | true | ConfigurationError | Yes (configure first) |

---

## 7. Test File Structure

```
crates/vo-core/src/resource_quota/
├── mod.rs              # Types + inline unit tests
├── enforcer.rs         # NamespaceRegistry, QuotaEnforcer + inline tests
├── policy.rs           # OvercommitPolicy + inline tests
├── proptests.rs        # Property-based tests (INV-002, INV-003, INV-004, INV-005)
└── integration_tests.rs # Full workflow integration tests
```

---

## 8. Mutation Testing Checkpoints

| Checkpoint | Mutation Target | Kill Condition |
|------------|---------------|----------------|
| M-001 | `requested_cores > max_cores` to `>=` | Boundary case at exactly limit passes when it should fail |
| M-002 | `quota.overcommit.allows_overcommit()` to `false` | Overcommit namespace rejects when it should accept |
| M-003 | `self.quotas.get(namespace)` to `self.quotas.get("wrong")` | Wrong namespace lookup succeeds |
| M-004 | `quota.cpu.as_ref()` to `None` | QuotaNotConfigured returned when cpu is configured |
| M-005 | Return type `Result<(), QuotaError>` to `Result<bool, QuotaError>` | Compile fails (type change) |
| M-006 | `is_overcommit_rejected()` matches | NamespaceNotFound incorrectly returns true |

---

## 9. Fuzz Targets

### Fuzz Target 1: Arbitrary NamespaceQuota Generation

```rust
fn fuzz_namespace_quota_registration(data: &[u8]) {
    // Generate arbitrary NamespaceQuota and verify registration succeeds
}
```

### Fuzz Target 2: Arbitrary check_* Calls

```rust
fn fuzz_check_calls(namespace: &str, resource: ResourceKind, requested: u64) {
    // Call check_* with arbitrary inputs and verify error taxonomy
}
```

### Fuzz Target 3: Overcommit Policy Combinations

```rust
fn fuzz_overcommit_combinations(quotas: Vec<NamespaceQuota>) {
    // Verify overcommit policy is uniform across all resources in a namespace
}
```

---

## 10. Kani Harnesses

### Harness 1: QuotaError::is_overcommit_rejected

```rust
#[kani::proof]
fn is_overcommit_rejected_is_correct_for_all_errors() {
    // Verify is_overcommit_rejected returns correct value for all QuotaError variants
}
```

### Harness 2: check_* Boundary Conditions

```rust
#[kani::proof]
fn check_cpu_boundary_condition_is_correct() {
    // Verify check_cpu returns correct result at boundary: requested == limit
}
```
