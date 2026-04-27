# Findings: cd-llu ARCH-DRIFT drift detection wave3-10

## Bead Status
- **Bead ID:** cd-llu (claimed and in_progress by brahmin)
- **Title:** ARCH-DRIFT: drift detection wave3-10
- **Type:** Task (architectural drift detection audit)

## Drift Detection Analysis

### Files Over 300 Lines (Architectural Violations)

| File | Lines | Status |
|------|-------|--------|
| vo-actor/src/lib.rs | 1914 | VIOLATION |
| vo-actor/src/probe.rs | 2032 | VIOLATION |
| vo-storage/src/append.rs | 1628 | VIOLATION |
| vo-actor/src/spawn_supervisor.rs | 1175 | VIOLATION |
| vo-actor/src/instance_registry.rs | ~900 | VIOLATION |
| vo-types/src/command_history.rs | 1778 | VIOLATION (test file excluded) |
| vo-core/src/replay/red_queen_adversarial_tests.rs | 2121 | Test (acceptable) |

**Source files violating 300-line limit:**
1. `crates/vo-actor/src/lib.rs` - 1914 lines (ACTOR FRAMEWORK)
2. `crates/vo-actor/src/probe.rs` - 2032 lines (HEALTH PROBES)
3. `crates/vo-storage/src/append.rs` - 1628 lines (STORAGE)
4. `crates/vo-actor/src/spawn_supervisor.rs` - 1175 lines (SPAWN)

### DDD Violations Observed

**Primitive Obsession Issues:**
- `vo-actor/src/lib.rs:68` - `workflow_type: String` - should be NewType
- `vo-actor/src/lib.rs:81` - `reason: String` - should be NewType
- `vo-actor/src/probe.rs:57` - `url: String` - should be Url type
- `vo-actor/src/probe.rs:70` - `command: String` - should be Command type
- `vo-actor/src/probe.rs:91` - `args: Vec<String>` - should not be raw Vec

**Missing State Modeling:**
- `WorkflowParadigm` enum exists but workflows don't appear to use explicit state machines
- `InstancePhaseView` enum (Replay/Live) suggests state drift in instance modeling

### Key Observations

1. **vo-actor/src/lib.rs** is a monolith (1914 lines) that should be split:
   - Consider: separate `orchestrator.rs`, `messages.rs`, `errors.rs`, `workflow.rs`
   - The enum definitions should be in a `types.rs` module

2. **vo-actor/src/probe.rs** (2032 lines) violates single responsibility:
   - `HttpProbeConfig`, `TcpProbeConfig`, `ExecProbeConfig` should be separate files
   - Probe logic should be in separate module per probe type

3. **vo-actor/src/spawn_supervisor.rs** (1175 lines) should be split:
   - `spawn.rs` - spawning logic
   - `supervisor.rs` - supervision logic

### Conclusion

**STATUS: DRIFT DETECTED**

Multiple architectural drift violations found in veloxide codebase:
- 4 major source files exceed 300-line limit
- Primitive obsession with String types throughout
- Missing module-level separation in actor framework

### Recommended Actions

1. Split `vo-actor/src/lib.rs` into separate modules
2. Extract probe types into separate files
3. Replace primitive `String` usage with domain newtypes
4. Schedule refactoring for future bead

---

*Investigated by: brahmin (polecat/worker)*
*Date: 2026-04-24*
*Session: 1d92bb45-116b-4fa6-8983-4690cbde5f26*
*Codebase: /home/lewis/src/veloxide*