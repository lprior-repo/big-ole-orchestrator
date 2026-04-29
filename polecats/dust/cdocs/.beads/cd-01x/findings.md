# Architectural Drift Detection Report: Wave 3 of 8

**Bead**: cd-01x
**Type**: ARCH-DRIFT: drift detection wave3-8
**Date**: 2026-04-24
**Polecat**: dust (cdocs)

---

## Executive Summary

**STATUS: DRIFT DETECTED**

This wave detected severe architectural drift across the veloxide codebase. The primary drift issues are:
1. **Documentation drift**: References to `vo-engine` which does not exist in workspace
2. **File size drift**: 240+ files exceed the 300-line architectural limit
3. **Primitive obsession**: Extensive use of raw `String`, `i32`, `u64` where newtypes should be used

---

## 1. vo-Engine Reference Drift

### Problem
The architecture spec explicitly states there is **no `vo-engine` crate** in the workspace:
> "There is **no `vo-engine` crate** in the workspace today."

However, multiple lib.rs files contain outdated doc comments referencing `vo-engine`:

| File | Line | Content |
|------|------|---------|
| `vo-actor/src/lib.rs` | 1 | `//! Actor framework for vo-engine.` |
| `vo-types/src/state/mod.rs` | 1 | `//! Domain state types for the vo-engine.` |
| `vo-core/src/lib.rs` | 1 | `//! Core engine implementation for vo-engine.` |
| `vo-common/src/lib.rs` | 1 | `//! Common utilities and types for vo-engine.` |
| `vo-api/src/lib.rs` | 1 | `//! HTTP API for vo-engine.` |
| `vo-linter/src/lib.rs` | 1 | `//! Static analysis and linting tools for vo-engine.` |
| `vo-types/src/events/mod.rs` | 1 | `//! Domain events for the vo-engine.` |
| `vo-frontend/src/lib.rs` | 1 | `//! Frontend UI components for vo-engine.` |

### Impact
- **vo-types/tests/scaffold_compliance.rs** has assertions that enforce these doc comments, meaning the scaffold compliance tests themselves are validating stale architecture

### Remediation
1. Update all lib.rs doc comments to reference actual crate names
2. Update `scaffold_compliance.rs` assertions to match actual crate names
3. File beads for each affected crate

---

## 2. File Size Drift (300-Line Limit Violation)

### Summary Table

| Crate | Max Lines | Total Files | Files >300 |
|-------|-----------|-------------|------------|
| vo-types | 1607 | 170 | 61 |
| vo-storage | 1628 | 116 | 48 |
| vo-api | 573 | 28 | 7 |
| vo-cli | 1075 | 26 | 5 |
| vo-worker | 765 | 32 | 10 |
| vo-frontend | 816 | 42 | 12 |
| vo-linter | 673 | 3 | 2 |
| vo-actor | 2032 | 53 | 21 |
| vo-core | 2121 | 138 | 51 |
| vo-common | 182 | 8 | 1 |
| vo-ipc | 375 | 11 | 4 |
| vo-sdk | 957 | 21 | 8 |
| vo-sdk-macros | 477 | 3 | 2 |

**Total: 240+ files exceeding the 300-line limit**

### Largest Files by Crate

**vo-actor** (21 files >300 lines):
- `probe.rs`: 2032 lines
- `lib.rs`: 1914 lines
- `message_router.rs`: 1202 lines
- `spawn_supervisor.rs`: 1175 lines
- `actor_messages.rs`: 961 lines
- `instance_registry_tests.rs`: 1277 lines
- `lifecycle.rs`: 778 lines

**vo-core** (51 files >300 lines):
- `red_queen_adversarial_tests.rs`: 2121 lines (test file)
- `invalid_business_data_tests.rs`: 1215 lines
- `workload_class.rs`: 901 lines
- `workspace_swap.rs`: 749 lines
- `lease_calc.rs`: 710 lines

**vo-types** (61 files >300 lines):
- `lib.rs`: 1607 lines
- `effects.rs`: (large)
- Multiple test files

### Remediation
Files must be split using the decomposition pattern:
- Split by concern (e.g., separate `timer_supervisor.rs` into `supervisor.rs`, `calc.rs`, `types.rs`)
- Extract test files into dedicated test modules
- Use `mod.rs` aggregation pattern for related functionality

---

## 3. Primitive Obsession Drift

### Problem
The codebase extensively uses raw types where newtypes should be used per Scott Wlaschin DDD principles.

### Evidence (vo-actor)

**Error types using raw String:**
```rust
// lib.rs
TerminateError::NotFound(String),
TerminateError::Failed(String),
StartError::NotFound(String),
StartError::Failed(String),

// heartbeat.rs
Registry(String),  // Should be InstanceId wrapper

// signal_messages.rs
SecretId(pub String),      // Should be SecretId NewType
BinaryHash(pub String),     // Should be BinaryHash NewType
NodeName(pub String),      // Should be NodeName NewType

// message_router.rs
ActorError(String),
ChannelId(String),  // Line 46: pub struct ChannelId(String);
```

**Function parameters using raw primitives:**
```rust
// timers.rs
pub fn compute_fire_at(base_ms: u64, duration_ms: u64) -> Result<u64, TimerError>
pub fn is_timer_expired(fire_at_ms: u64, now_ms: u64) -> bool

// timer_lifecycle.rs
fn create_timer_record(instance_id: InstanceId, fire_at_ms: u64)

// heartbeat.rs
pub async fn register_actor(&self, actor_id: String)
```

### Remediation
1. Create newtype wrappers for domain concepts:
   - `struct InstanceId(String)` → already exists in vo-types
   - `struct ChannelId(String)` → should be in vo-types
   - `struct NodeName(String)` → should be in vo-types
   - `struct SecretId(String)` → should be in vo-types
2. Replace error `String` fields with proper error types
3. Use `TimestampMs` consistently instead of raw `u64`

---

## 4. Other Drift Signals

### 4.1 Test Infrastructure Drift
- `redqueen_timer_drift.rs` appears in multiple veloxide worktrees but not in main
- Test file naming inconsistency (some use `_tests.rs`, others use `tests/` directories)

### 4.2 Workspace Layout Drift
The architecture spec mentions `vo-engine` and `vo-ui` that don't exist, but the actual crates are:
- vo-types, vo-storage, vo-api, vo-cli, vo-worker, vo-frontend, vo-linter, vo-actor, vo-core, vo-common, vo-ipc, vo-sdk

---

## Recommendations

1. **Immediate**: File beads to fix `vo-engine` references in all lib.rs files
2. **Short-term**: Create decomposition plan for files >1000 lines
3. **Medium-term**: Systematic newtype refactoring for domain concepts
4. **Ongoing**: Enforce 300-line limit in CI pre-commit hooks

---

## Files Requiring Immediate Attention

### Critical (>1000 lines, core functionality):
1. `vo-actor/src/probe.rs` - 2032 lines
2. `vo-actor/src/lib.rs` - 1914 lines
3. `vo-actor/src/message_router.rs` - 1202 lines
4. `vo-actor/src/spawn_supervisor.rs` - 1175 lines
5. `vo-core/src/red_queen_adversarial_tests.rs` - 2121 lines
6. `vo-types/src/lib.rs` - 1607 lines
7. `vo-storage/src/*.rs` files

### High (500-1000 lines):
8. `vo-actor/src/actor_messages.rs` - 961 lines
9. `vo-actor/src/lifecycle.rs` - 778 lines
10. `vo-actor/src/timers.rs` - 551 lines
11. `vo-actor/src/heartbeat.rs` - 515 lines
12. `vo-worker/src/pool/pool.rs` - 765 lines
13. `vo-sdk/src/*.rs` - 957 lines max

---

## Next Steps

1. Create bead for each crate fixing vo-engine references
2. Create umbrella bead for file decomposition initiative
3. Create bead for newtype refactoring initiative
4. Recommend CI enforcement of line-count limits
