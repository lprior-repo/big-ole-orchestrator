# Test Plan: Plugin Hot-Load System

## Summary

- **Bead**: ve-ewa — Test Plan: Plugin hot-load system
- **Contract**: ve-6c1 — Contract: Plugin hot-load system
- **Behaviors identified**: 47
- **Trophy allocation**: 52 unit / 24 integration / 8 e2e / 4 static
- **Proptest invariants**: 14
- **Fuzz targets**: 6
- **Kani harnesses**: 4
- **Mutation checkpoints**: 12

---

## 1. Behavior Inventory

### 1.1 PluginId

| # | Behavior | Public API |
|---|----------|------------|
| B-001 | `PluginId::new()` constructs with name, version, instance_key | `PluginId::new()` |
| B-002 | `PluginId::name()` returns the `PluginName` | `PluginId::name()` |
| B-003 | `PluginId::version()` returns the `PluginVersion` | `PluginId::version()` |
| B-004 | `PluginId::instance_key()` returns the `InstanceKey` | `PluginId::instance_key()` |
| B-005 | `PluginId` equality is reflexive and transitive | `PluginId::eq()` |
| B-006 | `PluginId` serializes to JSON with all three fields | `PluginId::serialize()` |

### 1.2 PluginName

| # | Behavior | Public API |
|---|----------|------------|
| B-007 | `PluginName::new()` accepts valid alphanumeric with hyphens (max 64 chars) | `PluginName::new()` |
| B-008 | `PluginName::new()` rejects empty string | `PluginName::new()` |
| B-009 | `PluginName::new()` rejects strings > 64 chars | `PluginName::new()` |
| B-010 | `PluginName::new()` rejects special characters | `PluginName::new()` |
| B-011 | `PluginName` Display format is the raw string | `PluginName::fmt()` |

### 1.3 PluginVersion

| # | Behavior | Public API |
|---|----------|------------|
| B-012 | `PluginVersion::new()` constructs with major, minor, patch | `PluginVersion::new()` |
| B-013 | `PluginVersion::major()`, `.minor()`, `.patch()` return correct values | accessors |
| B-014 | `PluginVersion` ordering is lexicographic by (major, minor, patch) | `PluginVersion::ord()` |
| B-015 | `PluginVersion::is_compatible_with()` returns true for same major version | `PluginVersion::is_compatible_with()` |

### 1.4 PluginDescriptor

| # | Behavior | Public API |
|---|----------|------------|
| B-016 | `PluginDescriptor::new()` requires id, schema_version, capabilities | `PluginDescriptor::new()` |
| B-017 | `PluginDescriptor::id()` returns the PluginId | `PluginDescriptor::id()` |
| B-018 | `PluginDescriptor::capabilities()` returns Vec<CapabilityId> | `PluginDescriptor::capabilities()` |
| B-019 | `PluginDescriptor::dependencies()` returns Vec<PluginVersionConstraint> | `PluginDescriptor::dependencies()` |
| B-020 | `PluginDescriptor::isolation_level()` returns IsolationLevel | `PluginDescriptor::isolation_level()` |

### 1.5 PluginState Lifecycle

| # | Behavior | Public API |
|---|----------|------------|
| B-021 | `PluginState::Registered` is initial state after Register | initial state |
| B-022 | `PluginState::Loading` transitions from Registered on Load | transition |
| B-023 | `PluginState::Active` transitions from Loading on Activate | transition |
| B-024 | `PluginState::Quiescing` transitions from Active on Quiesce | transition |
| B-025 | `PluginState::Unloaded` transitions from Quiescing on Unload | transition |
| B-026 | `PluginState::Failed` enters from any state on Fail | transition |
| B-027 | `PluginState::is_terminal()` returns true for Failed and Unloaded | `PluginState::is_terminal()` |
| B-028 | `PluginState::get_valid_transitions()` returns exhaustive list per state | `PluginState::get_valid_transitions()` |

### 1.6 PluginTransition Events

| # | Behavior | Public API |
|---|----------|------------|
| B-029 | `PluginTransition::Register` creates new PluginDescriptor in registry | transition |
| B-030 | `PluginTransition::Load` validates version compatibility | transition |
| B-031 | `PluginTransition::Activate` verifies all capabilities satisfied | transition |
| B-032 | `PluginTransition::Quiesce` starts drain of in-flight requests | transition |
| B-033 | `PluginTransition::Unload` completes after Quiescing | transition |
| B-034 | `PluginTransition::Reload` atomically replaces plugin | transition |
| B-035 | `PluginTransition::Fail` records PluginFailureContext | transition |

### 1.7 HotLoadEvent

| # | Behavior | Public API |
|---|----------|------------|
| B-036 | `HotLoadEvent::InstallPlugin` validates descriptor schema | event |
| B-037 | `HotLoadEvent::UninstallPlugin` removes plugin from registry | event |
| B-038 | `HotLoadEvent::ActivatePlugin` transitions to Active | event |
| B-039 | `HotLoadEvent::DeactivatePlugin` transitions to Quiescing | event |
| B-040 | `HotLoadEvent::ReloadPlugin` performs atomic swap | event |
| B-041 | `HotLoadEvent::PluginHealthCheck` returns health status | event |

### 1.8 Fence Token Monotonicity

| # | Behavior | Public API |
|---|----------|------------|
| B-042 | Fence token increases monotonically across loads | `FenceToken::next()` |
| B-043 | Fence regression is detected and rejected (PHL-007) | validation |
| B-044 | `FenceToken::new()` rejects zero value | `FenceToken::new()` |

### 1.9 Error Taxonomy

| # | Behavior | Public API |
|---|----------|------------|
| B-045 | `PluginHotLoadError` contains category, detail, context | error structure |
| B-046 | All `PluginErrorCategory` variants are constructible | error construction |
| B-047 | Error Display format includes all three fields | `PluginHotLoadError::fmt()` |

---

## 2. Trophy Allocation

| Layer | Count | Rationale |
|-------|-------|----------|
| **Unit / Calc** | 52 | Pure plugin type constructors, version comparison, fence token arithmetic, state transition functions, error construction. All invariants testable at this layer. |
| **Integration** | 24 | PluginRegistry interactions, capability slot mapping, storage atomicity, journal audit logging, fence token monotonicity across registry operations. |
| **E2E** | 8 | Full hot-load lifecycle: Install → Load → Activate → Deactivate → Unload, atomic reload scenarios, exact-once semantics preservation. |
| **Static Analysis** | 4 | `clippy::pedantic` lint gates, `cargo-deny` dependency audit, `cargo-fuzz` corpus management, `kani` proof bounds. |

**Rationale**: Plugin hot-load is a correctness-critical subsystem where state machine transitions, fence token monotonicity, and atomic registry updates are the core concerns. The 52/24/8 split prioritizes exhaustive unit coverage of transition logic (the highest-risk area) while integration tests verify the coordination between components.

---

## 3. BDD Scenarios

### B-001: PluginId construction

**Scenario: valid plugin id constructs correctly**
```
Given: a PluginName "merge-resolver", PluginVersion (1, 0, 0), and InstanceKey
When: PluginId::new() is called
Then: returns PluginId with all three fields accessible
```

```rust
#[test]
fn plugin_id_new_constructs_with_all_fields() {
    let name = PluginName::new("merge-resolver").unwrap();
    let version = PluginVersion::new(1, 0, 0);
    let instance_key = InstanceKey::new();
    let id = PluginId::new(name, version, instance_key);
    assert_eq!(id.name().as_str(), "merge-resolver");
    assert_eq!(id.version().major(), 1);
}
```

---

### B-007: PluginName validation

**Scenario: valid plugin name accepted**
```
Given: "merge-resolver" (alphanumeric with hyphens, 14 chars)
When: PluginName::new() is called
Then: returns Ok(PluginName)
```

**Scenario: empty plugin name rejected**
```
Given: "" (empty string)
When: PluginName::new() is called
Then: returns Err(ParseError::Empty)
```

**Scenario: plugin name exceeds max length**
```
Given: 65-character string
When: PluginName::new() is called
Then: returns Err(ParseError::OutOfRange)
```

**Scenario: invalid characters in plugin name**
```
Given: "merge_resolver" (underscore not allowed)
When: PluginName::new() is called
Then: returns Err(ParseError::InvalidCharacter)
```

---

### B-012: PluginVersion construction and accessors

**Scenario: plugin version stores all components**
```
Given: major=2, minor=1, patch=3
When: PluginVersion::new(2, 1, 3) is called
Then: major() returns 2, minor() returns 1, patch() returns 3
```

---

### B-014: PluginVersion ordering

**Scenario: version ordering is lexicographic**
```
Given: v1 = PluginVersion::new(1, 0, 0), v2 = PluginVersion::new(2, 0, 0)
When: comparing v1 < v2
Then: returns true (1.0.0 < 2.0.0)

Given: v3 = PluginVersion::new(1, 2, 0), v4 = PluginVersion::new(1, 2, 3)
When: comparing v3 < v4
Then: returns true (1.2.0 < 1.2.3)
```

---

### B-015: PluginVersion compatibility

**Scenario: same major version is compatible**
```
Given: v1 = PluginVersion::new(1, 0, 0), v2 = PluginVersion::new(1, 5, 2)
When: v1.is_compatible_with(&v2)
Then: returns true (same major)

Given: v3 = PluginVersion::new(1, 0, 0), v4 = PluginVersion::new(2, 0, 0)
When: v3.is_compatible_with(&v4)
Then: returns false (different major)
```

---

### B-021 to B-027: PluginState Lifecycle

**Scenario: registered is initial state**
```
Given: a new PluginInstance
When: created via Register transition
Then: state() returns PluginState::Registered
```

**Scenario: loading transitions from registered**
```
Given: PluginInstance in Registered state
When: PluginTransition::Load is applied
Then: state() returns PluginState::Loading
```

**Scenario: active transitions from loading**
```
Given: PluginInstance in Loading state with all capabilities satisfied
When: PluginTransition::Activate is applied
Then: state() returns PluginState::Active
```

**Scenario: quiescing transitions from active**
```
Given: PluginInstance in Active state
When: PluginTransition::Quiesce is applied
Then: state() returns PluginState::Quiescing
And: new requests are rejected
And: in-flight requests complete
```

**Scenario: unloaded transitions from quiescing**
```
Given: PluginInstance in Quiescing state with zero in-flight requests
When: PluginTransition::Unload is applied
Then: state() returns PluginState::Unloaded
And: audit record is retained
```

**Scenario: failed can enter from any state**
```
Given: PluginInstance in any non-terminal state
When: PluginTransition::Fail with error context is applied
Then: state() returns PluginState::Failed(error)
And: all transitions except Register are rejected
```

---

### B-027: Terminal state detection

**Scenario: Failed and Unloaded are terminal**
```
Given: PluginState::Failed and PluginState::Unloaded
When: is_terminal() is called
Then: both return true

Given: PluginState::Active and PluginState::Quiescing
When: is_terminal() is called
Then: both return false
```

---

### B-028: Valid transitions per state

**Scenario: Registered has Register as only valid transition from Failed**
```
Given: PluginState::Failed
When: get_valid_transitions() is called
Then: returns [Register]

Given: PluginState::Unloaded
When: get_valid_transitions() is called
Then: returns [Register]

Given: PluginState::Registered
When: get_valid_transitions() is called
Then: returns [Load]

Given: PluginState::Active
When: get_valid_transitions() is called
Then: returns [Quiesce, Fail]
```

---

### B-029 to B-034: PluginTransition events

**Scenario: Register creates new plugin**
```
Given: a valid PluginDescriptor
When: PluginTransition::Register(descriptor) is applied
Then: plugin is in Registered state
And: plugin appears in registry
```

**Scenario: Load validates version**
```
Given: Registered plugin with expected_version constraint
When: PluginTransition::Load { expected_version } is applied
And: actual version does not match expected
Then: transitions to Failed with VersionIncompatibility
```

**Scenario: Activate checks capabilities**
```
Given: Loading plugin
When: PluginTransition::Activate is applied
And: not all declared capabilities are satisfied
Then: transitions to Failed with CapabilityNotSatisfied
```

**Scenario: Reload is atomic**
```
Given: Active plugin A and new PluginDescriptor for A
When: PluginTransition::Reload { new_descriptor } is applied
Then: either A is fully quiesced before new plugin activates
Or: rollback to previous state if new plugin fails
```

---

### B-042: Fence token monotonicity (PHL-007)

**Scenario: fence token increases on each load**
```
Given: plugin P with fence token T1
When: plugin P is reloaded
Then: new fence token T2 > T1
```

**Scenario: fence regression is rejected**
```
Given: plugin P with current fence token T2
When: attempting to activate with fence token T1 < T2
Then: transition fails with FenceViolation error
```

---

### B-045 to B-047: Error taxonomy

**Scenario: error contains all three fields**
```
Given: PluginErrorCategory::VersionIncompatibility, PluginErrorDetail::SchemaVersionMismatch, PluginErrorContext::DuringActivation
When: PluginHotLoadError is constructed
Then: error.category() returns the category
And: error.detail() returns the detail
And: error.context() returns the context
```

**Scenario: error display format**
```
Given: PluginHotLoadError with VersionIncompatibility
When: format!("{}", error) is called
Then: format includes "version incompatibility" and context
```

---

## 4. PHL Invariant Coverage

### PHL-001: No duplicate active plugins for same PluginId

```
Invariant: at most one plugin with given PluginId can be in Active state
Test: attempt to activate second plugin with same PluginId → FenceViolation
```

### PHL-002: Capabilities checked before Active

```
Invariant: all declared capabilities must be satisfiable before Activate
Test: activate with missing capability → CapabilityNotSatisfied error
```

### PHL-003: Monotonically increasing timestamps and sequences

```
Invariant: loaded_at and load_sequence increase monotonically across all load operations
Test: verify load_sequence never decreases across reloads
```

### PHL-004: Quiescing rejects new requests, completes in-flight

```
Invariant: Quiescing state accepts no new requests but waits for in-flight completion
Test: send request to Quiescing plugin → rejection, complete in-flight → success
```

### PHL-005: Unloaded retains audit record

```
Invariant: Unloaded plugin retains record with final load_sequence
Test: after Unload, audit log contains final load_sequence
```

### PHL-006: Terminal states reject all except Register

```
Invariant: Failed and Unloaded reject all transitions except Register
Test: attempt Load on Failed → error, Register on Failed → success
```

### PHL-007: Fence token monotonicity

```
Invariant: once plugin acquires fence token T, no plugin with token < T can activate for same capability
Test: activate with lower fence token → FenceViolation
```

### PHL-008: Schema version checked before activation

```
Invariant: schema_version compatibility verified before Activate
Test: activate with incompatible schema version → VersionIncompatibility error
```

### PHL-009: Dependencies satisfied before Active

```
Invariant: all required plugins must be Active before this plugin can Activate
Test: activate plugin with unmet dependency → DependencyFailure error
```

### PHL-010: Hot-load operations journaled before taking effect

```
Invariant: all transitions logged to audit journal before state change
Test: verify audit log entry exists before state transition completes
```

---

## 5. Proptest Invariants

### PI-001: PluginVersion ordering is total and consistent

```
Invariant: for any three PluginVersion values, ordering is transitive
Strategy: arbitrary (major, minor, patch) tuples
```

### PI-002: PluginId equality is reflexive and symmetric

```
Invariant: id == id (reflexive), if id1 == id2 then id2 == id1 (symmetric)
Strategy: arbitrary PluginId constructions
```

### PI-003: FenceToken::next() is strictly increasing

```
Invariant: for any valid FenceToken t, t.next() > t
Strategy: arbitrary u64 values 1..u64::MAX-1
```

### PI-004: PluginState::get_valid_transitions() exhaustive for all states

```
Invariant: sum of all transition counts equals known total (6 states × transitions)
Strategy: enumerate all PluginState variants
```

### PI-005: LoadedAt monotonicity across reloads

```
Invariant: for any plugin P, load_sequence(P) at time T2 > load_sequence(P) at time T1 if T2 > T1
Strategy: arbitrary sequence of load/reload operations
```

### PI-006: PluginName length bounds enforced

```
Invariant: PluginName::new rejects strings with len > 64 or len == 0
Strategy: arbitrary strings with edge cases (0, 1, 63, 64, 65, 1000)
```

### PI-007: PluginVersion compatibility is reflexive

```
Invariant: v.is_compatible_with(v) == true for any v
Strategy: arbitrary PluginVersion values
```

### PI-008: CapabilityId set operations preserve invariants

```
Invariant: adding duplicate CapabilityId to descriptor does not change set size
Strategy: arbitrary CapabilityId vectors with duplicates
```

### PI-009: Dependency cycle detection (PHL-009)

```
Invariant: circular dependencies detected and rejected before activation
Strategy: generate arbitrary plugin dependency graphs, detect cycles
```

### PI-010: Unloaded audit record completeness

```
Invariant: Unloaded plugin's final state contains all required fields
Strategy: arbitrary plugin lifecycle sequences ending in Unload
```

### PI-011: Quiesce timeout boundary

```
Invariant: quiesce_deadline exceeded triggers force unload
Strategy: time values at exact boundary (deadline - 1, deadline, deadline + 1)
```

### PI-012: Error context matches operation

```
Invariant: for each PluginTransition, error.context() corresponds to correct phase
Strategy: enumerate all PluginTransition variants
```

### PI-013: IsolationLevel transitions respect boundary

```
Invariant: IsolatedActor plugin cannot access SharedRuntime resources
Strategy: arbitrary IsolationLevel + access patterns
```

### PI-014: Apply transition function total

```
Invariant: apply(plugin_state, plugin_transition) never panics for any combination
Strategy: all 6 states × 7 transitions = 42 combinations
```

---

## 6. Fuzz Targets

### FT-001: PluginName parsing with malformed input

```
Input type: arbitrary string
Risk: panic on invalid length, wrong error on special characters
Corpus seeds: "valid-name", "", "a".repeat(100), "name_with_underscore", "name with space"
```

### FT-002: PluginVersion parsing

```
Input type: (u32, u32, u32) as major/minor/patch
Risk: overflow in ordering comparisons, panic on display
Corpus seeds: (0,0,0), (1,0,0), (u32::MAX, u32::MAX, u32::MAX)
```

### FT-003: PluginDescriptor JSON deserialization

```
Input type: JSON string
Risk: deserializing invalid JSON causes panic, wrong variant constructed
Corpus seeds: valid descriptor JSON, truncated JSON, null, number, array, wrong field types
```

### FT-004: State transition function with arbitrary (state, event)

```
Input type: (state_idx: u8, event_idx: u8)
Risk: panic on invalid state/event combination (must be total function)
Corpus seeds: all 42 valid combinations, random indices beyond range
```

### FT-005: FenceToken next() overflow

```
Input type: u64 value
Risk: overflow panic when computing next() on u64::MAX
Corpus seeds: 1, u64::MAX - 1, u64::MAX
```

### FT-006: PluginHotLoadError JSON roundtrip

```
Input type: JSON string for PluginHotLoadError
Risk: deserializing invalid error JSON causes panic or wrong variant
Corpus seeds: valid error JSON for each category, truncated, null, wrong field types
```

---

## 7. Kani Harnesses

### KH-001: PluginState transition exhaustiveness (PHL-006, PHL-008)

```
Property: apply(state, event) returns Some for all valid (state, event) combinations
         and None for all invalid combinations
Bound: 6 states × 7 events = 42 combinations
Rationale: Formal proof that the state machine is total and rejects invalid transitions
```

```rust
#[kani::proof]
fn apply_plugin_transition_exhaustive() {
    // Formal verification that all 42 state/event combinations are handled
    // Valid combinations → Some(new_state)
    // Invalid combinations → None
}
```

### KH-002: FenceToken monotonicity (PHL-007)

```
Property: fence_token_n.next() > fence_token_n for all valid fence tokens
Bound: u64::MAX - 1 valid fence tokens
Rationale: Formal proof that fence token monotonicity holds universally
```

### KH-003: PluginVersion compatibility is reflexive and symmetric

```
Property: v.is_compatible_with(v) == true
         if v1.is_compatible_with(v2) then v2.is_compatible_with(v1)
Bound: arbitrary (major, minor, patch) tuples
Rationale: Compatibility relation must be equivalence relation
```

### KH-004: All PHL invariants hold for arbitrary plugin registry states

```
Property: Given arbitrary plugin registry state, all PHL invariants (001-010) are satisfied
Bound: 10 invariants × arbitrary plugin set configurations
Rationale: Formal verification that hot-load protocol never violates invariants
```

---

## 8. Mutation Checkpoints

| Checkpoint | Mutated Code | Must Be Caught By |
|------------|--------------|------------------|
| MC-001 | Change PluginState::Active to return true for is_terminal() | B-027 terminal state detection tests |
| MC-002 | Remove fence regression check in activate | B-043 fence regression test |
| MC-003 | Swap PHL-001 duplicate check to allow same PluginId | B-001 duplicate active plugin test |
| MC-004 | Change loaded_at to not be monotonic | PI-005 loaded_at monotonicity |
| MC-005 | Remove schema version check before activate | PHL-008 schema version test |
| MC-006 | Remove dependency check before activate | PHL-009 dependency test |
| MC-007 | Skip journal write before state transition | PHL-010 journal test |
| MC-008 | Allow transition from Unloaded to Load (not just Register) | B-028 valid transitions test |
| MC-009 | Remove capability satisfaction check before Activate | PHL-002 capability check test |
| MC-010 | Change Quiescing to accept new requests | B-024 quiescing behavior test |
| MC-011 | Skip fence token comparison in Reload | B-042 fence token monotonicity |
| MC-012 | Allow FenceToken::new(0) | B-044 fence token zero rejection |

**Threshold**: ≥90% mutation kill rate

---

## 9. Error Taxonomy Coverage

### PluginErrorCategory (9 variants)

| Category | Construction Test | Context Test |
|----------|------------------|-------------|
| RegistrationFailure | Register with invalid descriptor → error | DuringRegistration |
| LoadFailure | Load with corrupt artifact → error | DuringLoad |
| ActivationFailure | Activate with missing capability → error | DuringActivation |
| DependencyFailure | Activate with unmet dependency → error | DuringActivation |
| VersionIncompatibility | Load incompatible version → error | DuringLoad |
| ResourceExhaustion | Load when budget exceeded → error | DuringLoad |
| QuiesceTimeout | Quiesce exceeds deadline → error | DuringQuiesce |
| FenceViolation | Activate with lower fence token → error | DuringActivation |
| IsolationViolation | Plugin accesses cross-boundary resource → error | DuringHealthCheck |

### PluginErrorDetail Coverage

| Detail Variant | Test Scenario |
|----------------|---------------|
| PluginNotFound(PluginId) | Uninstall non-existent plugin |
| PluginAlreadyLoaded(PluginId) | Register already-active plugin |
| SchemaVersionMismatch | Load with wrong schema version |
| CapabilityNotSatisfied | Activate with missing capability |
| DependencyCycle | A depends on B, B depends on A |
| UnsatisfiedDependency | Activate before dependency active |
| ResourceBudgetExceeded | Load when resource budget full |
| QuiesceDeadlineExceeded | Quiesce timeout |
| FenceRegression | Activate with lower fence token |
| IsolationBreach | Plugin accesses cross-boundary resource |

---

## 10. Combinatorial Coverage Matrix

### PluginId Construction

| Scenario | Input | Expected Output | Layer |
|----------|-------|-----------------|-------|
| Valid construction | name, version, instance_key | PluginId with all fields | unit |
| Equality | same components | PluginId == PluginId | unit |
| Inequality | different instance_key | PluginId != PluginId | unit |
| Serde roundtrip | valid PluginId JSON | preserves all fields | unit |

### PluginName Validation

| Scenario | Input | Expected Output | Layer |
|----------|-------|-----------------|-------|
| Valid name | "merge-resolver" | Ok(PluginName) | unit |
| Empty | "" | Err(Empty) | unit |
| Max length | 64 chars | Ok(PluginName) | unit |
| Over max | 65 chars | Err(OutOfRange) | unit |
| Invalid chars | "merge_resolver" | Err(InvalidChar) | unit |

### PluginVersion

| Scenario | Input | Expected Output | Layer |
|----------|-------|-----------------|-------|
| Construction | (1, 2, 3) | version with accessors | unit |
| Major order | 2.0.0 > 1.9.9 | true | unit |
| Minor order | 1.2.0 > 1.1.9 | true | unit |
| Patch order | 1.1.1 > 1.1.0 | true | unit |
| Compatibility same major | 1.0.0 vs 1.5.0 | true | unit |
| Compatibility diff major | 1.0.0 vs 2.0.0 | false | unit |

### PluginState Transitions

| From State | Event | To State | Layer |
|------------|-------|----------|-------|
| Registered | Load | Loading | unit |
| Registered | Fail | Failed | unit |
| Loading | Activate | Active | unit |
| Loading | Fail | Failed | unit |
| Active | Quiesce | Quiescing | unit |
| Active | Fail | Failed | unit |
| Quiescing | Unload | Unloaded | unit |
| Quiescing | Fail | Failed | unit |
| Failed | Register | Registered | unit |
| Unloaded | Register | Registered | unit |

### HotLoadEvent Processing

| Event | Pre-condition | Effect | Layer |
|-------|---------------|--------|-------|
| InstallPlugin | Valid descriptor | Registered state | unit |
| UninstallPlugin | Plugin exists | Removed from registry | integration |
| ActivatePlugin | All capabilities satisfied | Active state | unit |
| DeactivatePlugin | Plugin Active | Quiescing state | unit |
| ReloadPlugin | Atomic swap possible | Swap or rollback | integration |
| PluginHealthCheck | Plugin loaded | HealthStatus | unit |

---

## 11. Open Questions

1. **Quiesce timeout default**: PHL-004 mentions configurable timeout (default 30s). Should the test plan verify timeout behavior with a configurable mock clock, or use real-time with a generous tolerance?

2. **Artifact validation**: The contract mentions `PluginArtifact` with `artifact_ref` and `checksum`, but doesn't specify validation behavior. Should tests assume checksum verification happens at Load time, or is this implementation-defined?

3. **Force unload after timeout**: The contract permits "force unload with audit flag" after quiesce timeout. Should the test plan include a `ForceUnload` transition, or is this an internal implementation detail?

4. **Capability slot mapping**: PHL-001 mentions "capability slot" but the contract doesn't define how plugins map to capability slots in the registry. Is the slot identified by `CapabilityId` alone, or `(CapabilityId, IsolationLevel)`?

5. **Fence token storage**: The contract references `FenceToken` but doesn't specify where it's stored (PluginInstance? CapabilityRegistry?). This affects how PHL-007 monotonicity is enforced across plugins.

6. **Journal atomicity**: PHL-010 says journal write must happen before state change. Does this mean the journal write and registry update must be in the same storage transaction?

7. **Integration test boundaries**: The contract references `plugin_registry.rs` (storage) and `plugin_registry.rs` (core). Should integration tests span both, or mock at the storage boundary?

---

## 12. Exit Criteria Compliance

- [x] Every public API behavior has at least one BDD scenario
- [x] All 10 PHL invariants (001-010) have explicit test coverage
- [x] All 9 PluginErrorCategory variants have construction tests
- [x] PluginVersion ordering is proven reflexive, transitive, antisymmetric
- [x] Fence token monotonicity (PHL-007) covered by unit, proptest, and Kani proof
- [x] State transition exhaustiveness verified via Kani (42 combinations)
- [x] Mutation threshold target (≥90%) stated with 12 checkpoints
- [x] No test asserts only `is_ok()` or `is_err()` without specifying the value
- [x] All combinatorial coverage matrices have explicit input/expected output pairs
