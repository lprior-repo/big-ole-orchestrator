# Findings: tw-uitj - Replace RwLock expect with Result

## Issue
In-memory repository returns panic on poisoned lock. Should return Result with PoisonedLock error.

## Files Modified
- `/home/lewis/src/hardline/crates/core/src/domain/agent_registry/mod.rs` - Added `PoisonedLock` error variant
- `/home/lewis/src/hardline/crates/core/src/domain/agent_registry/repository/in_memory.rs` - Replaced 9 `expect("lock poisoned")` calls

## Changes Made

### 1. Added PoisonedLock Error Variant (mod.rs:92)
```rust
/// Lock poisoned
#[error("lock poisoned")]
PoisonedLock,
```

### 2. Replaced 9 expect() Calls (in_memory.rs)
All `.expect("lock poisoned")` calls replaced with `.map_err(|_| AgentRegistryError::PoisonedLock)?`:

| Line | Method | Lock Type |
|------|--------|-----------|
| 34 | save() | write |
| 44 | find_by_id() | read |
| 49 | find_by_name() | read |
| 54 | list_all() | read |
| 62 | list_by_status() | read |
| 74 | list_by_workspace() | read |
| 83 | find_stale_agents() | read |
| 92 | delete() | write |
| 102 | update_heartbeat() | write |

## Pattern Used
```rust
// Before:
let agents = self.agents.write().expect("lock poisoned");

// After:
let agents = self.agents.write().map_err(|_| AgentRegistryError::PoisonedLock)?;
```

## Verification
- Grep confirmed no remaining "lock poisoned" strings in in_memory.rs
- Build was attempted but timed out (dependencies compiling)
- Code structure follows existing error handling patterns in the codebase