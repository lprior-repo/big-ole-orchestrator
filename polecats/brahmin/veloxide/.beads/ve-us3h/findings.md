# Findings: ve-us3h - ARCH-DRIFT vo-actor/src/lib.rs 1932→2879 lines

## Issue
File: `vo-actor/src/lib.rs`
- Current: **2879 lines**
- Limit: **300 lines**
- Violation: **9.6x the limit**

## Root Cause
The `lib.rs` file contains 17 inline `#[cfg(test)]` test modules spanning lines 96-2879 (2783 lines).
The non-test production code is only lines 1-95 (~95 lines).

## Test Modules in lib.rs (lines 96-2879)

| Module | Start Line | Purpose |
|--------|-----------|---------|
| signal_error_tests | 97 | SignalError construction tests |
| terminate_error_tests | 124 | TerminateError construction tests |
| constructor_tests_instance_actor_message | 316 | InstanceActorMessage constructors |
| constructor_tests_control_actor_message | 453 | ControlActorMessage constructors |
| debug_format_instance_actor_message | 516 | Debug format for InstanceActorMessage |
| debug_format_control_actor_message | 608 | Debug format for ControlActorMessage |
| clone_instance_actor_message | 658 | Clone tests for InstanceActorMessage |
| clone_control_actor_message | 843 | Clone tests for ControlActorMessage |
| partial_eq_instance_actor_message | 890 | PartialEq tests for InstanceActorMessage |
| partial_eq_control_actor_message | 953 | PartialEq tests for ControlActorMessage |
| eq_properties_instance_actor_message | 995 | Eq property tests for InstanceActorMessage |
| eq_properties_control_actor_message | 1060 | Eq property tests for ControlActorMessage |
| send_sync_bounds | 1104 | Send+Sync bounds verification |
| ractor_message_trait | 1137 | Ractor Message trait impl tests |
| reserved_permit_budget_tests | 1246 | Permit budget tests |
| control_actor_tests | 1861 | Main ControlActor behavior tests |
| accept_resume_tests | 2434 | AcceptAndResume atomic tests |

## Production Code (lines 1-95)
- Module doc comment
- Use statements (bytes, vo_types)
- NamespaceId type alias
- heartbeat module declaration
- 15 module declarations (async_message_router, fairness, instance_registry, etc.)
- TerminateError enum
- WorkflowParadigm enum
- InstancePhaseView enum
- OrchestratorMsg enum
- SignalError enum
- InstanceSnapshot struct

## Recommended Fix
Extract all test modules into a new file `vo-actor/src/lib_tests.rs`:

1. Create `lib_tests.rs` with all 17 test modules
2. Replace test modules in `lib.rs` with: `#[cfg(test)] pub mod lib_tests;`
3. Result: `lib.rs` → ~100 lines (under 300-line limit)

## Worktree Issue
**CRITICAL**: The worktree at `/home/lewis/gt/polecats/brahmin/veloxide/` does NOT contain the source code.
Source is at `/home/lewis/gt/crates/vo-actor/src/lib.rs` (main repo).
This appears to be a worktree configuration issue — the brahmin worktree is a subdirectory of the main repo but doesn't have the actual crate files checked out.

## Status
**NO CODE CHANGES** — Worktree doesn't contain source code. Cannot refactor.
