## Contract: Plugin Hot-Load System

### 1. Purpose

Defines the contract for runtime hot-loading of plugins in the veloxide event-sourced actor system. This contract establishes the types, invariants, lifecycle states, and error taxonomy for the plugin hot-load subsystem, enabling plugins to be loaded, unloaded, and updated without system restart while preserving exact-once semantics.

### 2. Source ADRs

- `docs/adr/v2/ADR-039-v2-hierarchical-lifecycle-state-machine.md` (lifecycle state invariants)
- `docs/adr/v2/ADR-041-v2-managed-connector-runtime-contract.md` (connector prepare/commit pattern)
- `docs/adr/v2/ADR-029-v2-execution-leases-and-fencing.md` (lease semantics for plugin isolation)
- `docs/adr/v2/ADR-035-v2-event-schema-evolution-and-upcasting.md` (version compatibility)
- `docs/adr/v2/ADR-016-v2-atomic-storage-snapshots.md` (atomic plugin registry updates)

### 3. Plugin Types

#### 3.1 PluginId

Unique identifier for a plugin instance.

```
PluginId {
  name: PluginName,
  version: PluginVersion,
  instance_key: InstanceKey,
}
```

#### 3.2 PluginName

Semantic name for a plugin family (e.g., "merge-resolver", "blob-connector").

```
PluginName(SmolfoldString) // alphanumeric, hyphens allowed, max 64 chars
```

#### 3.3 PluginVersion

Semantic version tuple for plugin compatibility checking.

```
PluginVersion {
  major: u32,
  minor: u32,
  patch: u32,
}
```

#### 3.4 PluginDescriptor

Static metadata describing a plugin's capabilities and requirements.

```
PluginDescriptor {
  id: PluginId,
  schema_version: SchemaVersion,
  capabilities: Vec<CapabilityId>,
  dependencies: Vec<PluginVersionConstraint>,
  resource_requirements: ResourceBudget,
  isolation_level: IsolationLevel,
}
```

#### 3.5 PluginInstance

Runtime instance of a loaded plugin.

```
PluginInstance {
  descriptor: PluginDescriptor,
  state: PluginState,
  loaded_at: TimestampMs,
  load_sequence: SequenceNumber,
  fence_token: FenceToken,
}
```

#### 3.6 CapabilityId

Identifier for a capability provided by a plugin.

```
CapabilityId(SmolfoldString) // e.g., "merge-conflict-resolver", "blob-sink"
```

#### 3.7 PluginVersionConstraint

Dependency constraint specifying acceptable version ranges.

```
PluginVersionConstraint {
  name: PluginName,
  range: VersionRange, // e.g., ">=1.0.0 <2.0.0"
}
```

### 4. Plugin Lifecycle States

#### 4.1 PluginState Enum

```
enum PluginState {
  /// Plugin binary is registered but not yet loaded into runtime.
  Registered,

  /// Plugin is being validated and initialized.
  Loading,

  /// Plugin is active and handling requests.
  Active,

  /// Plugin is quiescing (draining in-flight requests).
  Quiescing,

  /// Plugin is unloaded but registry record retained for audit.
  Unloaded,

  /// Plugin load failed and requires manual intervention.
  Failed(PluginFailureContext),
}
```

#### 4.2 PluginTransition Events

```
enum PluginTransition {
  Register(PluginDescriptor),
  Load { expected_version: PluginVersion },
  Activate,
  Quiesce,
  Unload,
  Reload { new_descriptor: PluginDescriptor },
  Fail { error: PluginLoadError },
}
```

#### 4.3 IsolationLevel

```
enum IsolationLevel {
  /// Plugin shares actor runtime, no fencing guarantees.
  SharedRuntime,

  /// Plugin runs in isolated actor with lease fencing.
  IsolatedActor,

  /// Plugin runs in separate process with own memory space.
  Process,
}
```

### 5. Hot-Load Events

#### 5.1 HotLoadEvent

Events that drive plugin lifecycle transitions.

```
enum HotLoadEvent {
  InstallPlugin { descriptor: PluginDescriptor, artifact: PluginArtifact },
  UninstallPlugin { plugin_id: PluginId },
  ActivatePlugin { plugin_id: PluginId },
  DeactivatePlugin { plugin_id: PluginId },
  ReloadPlugin { plugin_id: PluginId, new_descriptor: PluginDescriptor },
  PluginHealthCheck { plugin_id: PluginId },
}
```

#### 5.2 PluginArtifact

Opaque artifact reference for plugin distribution.

```
PluginArtifact {
  artifact_ref: ArtifactRef,
  checksum: BinaryHash,
  schema_version: SchemaVersion,
}
```

### 6. Invariants (PHL-*)

- **PHL-001**: No two plugins with the same `PluginId` may be Active simultaneously
- **PHL-002**: A plugin must pass all capability checks before transitioning to Active
- **PHL-003**: `loaded_at` and `load_sequence` are monotonically increasing across all load operations
- **PHL-004**: A plugin in `Quiescing` state rejects new requests but completes in-flight requests
- **PHL-005**: `Unloaded` plugins retain audit record with final `load_sequence` for replay correctness
- **PHL-006**: Terminal states (`Failed`, `Unloaded`) reject all transitions except `Register`
- **PHL-007**: Plugin fence token monotonicity is preserved: once a plugin acquires fence token T, no plugin with token < T can be activated for the same capability slot
- **PHL-008**: Schema version compatibility is verified before any plugin activation
- **PHL-009**: Dependencies must be satisfied (all required plugins Active) before a plugin can transition to Active
- **PHL-010**: Hot-load operations are journaled to the plugin audit log before taking effect

### 7. Error Taxonomy

```rust
struct PluginHotLoadError {
    category: PluginErrorCategory,
    detail: PluginErrorDetail,
    context: PluginErrorContext,
}

enum PluginErrorCategory {
    RegistrationFailure,     // Plugin registration failed
    LoadFailure,            // Plugin failed to load
    ActivationFailure,      // Plugin failed to activate
    DependencyFailure,      // Unmet dependencies
    VersionIncompatibility,  // Schema/version mismatch
    ResourceExhaustion,     // Cannot allocate required resources
    QuiesceTimeout,         // Plugin did not quiesce in time
    FenceViolation,         // Fence token invariant violated
    IsolationViolation,      // Plugin violated isolation boundary
}

enum PluginErrorDetail {
    PluginNotFound(PluginId),
    PluginAlreadyLoaded(PluginId),
    SchemaVersionMismatch {
        expected: SchemaVersion,
        actual: PluginVersion,
    },
    CapabilityNotSatisfied {
        plugin_id: PluginId,
        missing: CapabilityId,
    },
    DependencyCycle(Vec<PluginName>),
    UnsatisfiedDependency {
        plugin_id: PluginId,
        missing: PluginVersionConstraint,
    },
    ResourceBudgetExceeded {
        plugin_id: PluginId,
        required: ResourceBudget,
        available: ResourceBudget,
    },
    QuiesceDeadlineExceeded(PluginId),
    FenceRegression {
        plugin_id: PluginId,
        presented_token: FenceToken,
        current_token: FenceToken,
    },
    IsolationBreach {
        plugin_id: PluginId,
        violation_type: IsolationViolationType,
    },
}

enum PluginErrorContext {
    DuringRegistration,
    DuringLoad,
    DuringActivation,
    DuringQuiesce,
    DuringUnload,
    DuringHealthCheck,
}
```

### 8. Hot-Load Protocol

1. **Register**: Validate descriptor schema, check for duplicate PluginId, allocate registry slot
2. **Validate Dependencies**: Verify all dependency constraints are satisfiable by registered plugins
3. **Allocate Resources**: Reserve resource budget for plugin according to descriptor requirements
4. **Load**: Initialize plugin binary, run version compatibility checks, establish fence token
5. **Quiesce Old** (for reload): If replacing existing plugin, quiesce old plugin before activating new
6. **Activate**: Transition to Active, publish capabilities to capability registry
7. **Health Monitor**: Continuous health checks per ADR-041 connector pattern
8. **Audit**: Log all transitions to plugin audit journal with sequence numbers

### 9. Constraints

- Hot-load operations must not violate exact-once semantics for workflow execution
- A plugin in Failed state blocks its capability slot until manual intervention or automatic retry
- Quiesce timeout is bounded (configurable, default 30s); after timeout, force unload is permitted with audit flag
- Plugin reload must be atomic: old plugin fully quiesced before new plugin activates, or rollback
- The plugin registry update and plugin state transition must be atomic (single storage transaction)
- Hot-load must preserve fence token monotonicity across all plugins in the same capability slot
- A plugin may not acquire resources beyond its declared `resource_requirements`

### 10. Relevant Files

- `crates/vo-types/src/` (new plugin module under vo-types)
- `crates/vo-types/src/plugin/mod.rs` (plugin types and state machine)
- `crates/vo-types/src/plugin/errors.rs` (error taxonomy)
- `crates/vo-types/src/plugin/lifecycle.rs` (state transition logic)
- `crates/vo-core/src/plugin_registry.rs` (plugin registry and capability mapping)
- `crates/vo-storage/src/partitions/plugin_registry.rs` (persistent plugin registry storage)
- `docs/adr/v2/ADR-041-v2-managed-connector-runtime-contract.md` (connector pattern reference)

### 11. Acceptance Criteria

- Plugin types compile and cover all plugin lifecycle states (Registered, Loading, Active, Quiescing, Unloaded, Failed)
- Hot-load event vocabulary is exhaustive for all valid plugin lifecycle transitions
- All invariants (PHL-001 through PHL-010) are formally stated and testable
- Error taxonomy covers registration, load, activation, dependency, version, resource, and isolation failure modes
- Fence token monotonicity invariant is expressed consistently with ADR-029
- Schema version compatibility checking is integrated into activation protocol
- The contract is self-contained and references only existing ADR documents and plausible future file paths
