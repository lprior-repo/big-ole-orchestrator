# ADR-009 Review Findings: Multi-Task Binary & --execute-node Routing

**Bead:** ve-ewnx
**Reviewer:** brahmin
**Date:** 2026-04-24
**Status:** Compilation errors block full verification

---

## Executive Summary

The ADR-009 multi-task binary model is partially implemented. `--graph` discovery works correctly, but **`--execute-node` routing is not implemented** - the `vo_sdk::start()` function described in ADR-011 does not exist. Additionally, compilation errors in `vo-api` and `vo-linter` block the full build.

---

## 1. Compilation Status

### 1.1 vo-linter (FIXED)
**File:** `crates/vo-linter/src/rules/random.rs`
**Issue:** Missing import for `Rule` trait on line 16.
**Fix Applied:**
```rust
// Added: use crate::rules::Rule;
use crate::diagnostic::{Diagnostic, LintCode};
use crate::rules::Rule;  // <-- ADDED
```

### 1.2 vo-api (BLOCKING)
**Files:** `crates/vo-api/src/handlers/workflow_start.rs`, `workflow_status.rs`
**Issue:** Handlers reference `OrchestratorMsg::StartWorkflow`, `OrchestratorMsg::GetStatus`, `OrchestratorMsg::ListActive` which do not exist in the enum.
**Actual enum (vo-actor/src/lib.rs:66):**
```rust
pub enum OrchestratorMsg {
    Signal {
        instance_id: InstanceId,
        signal_name: String,
        payload: Bytes,
        reply: ractor::port::RpcReplyPort<Result<(), SignalError>>,
    },
}
```
**Impact:** vo-api cannot compile. The API layer expects workflow lifecycle messages that don't exist.

---

## 2. ADR-009 Verification: `--graph` Support ✅

**Status:** IMPLEMENTED and working.

**Implementation location:** `crates/vo-sdk/src/graph_args.rs`

| Function | Status |
|----------|--------|
| `parse_graph_args()` | ✅ Works - parses `--graph` CLI flag |
| `emit_graph_if_requested()` | ✅ Works - serializes WorkflowSpec to JSON, exits with code 0 |
| `WorkflowSpec` | ✅ Implements full DAG validation (cycle detection, edge validation) |

**Example usage:**
```rust
let spec = wf.build().unwrap();
emit_graph_if_requested(&std::env::args().collect::<Vec<_>>(), &spec);
```

---

## 3. ADR-009/ADR-011 Verification: `--execute-node` Support ❌

**Status:** NOT IMPLEMENTED.

### What ADR-009/ADR-011 Describe

The ADR specifies that when the Engine invokes:
```
./binary --execute-node charge_stripe
```

The `vo_sdk::start()` function should:
1. Parse the `--execute-node <name>` argument
2. Look up the task function by name
3. Read input from FD3
4. Execute the task
5. Write result to FD4

### Current State

**Missing:** `vo_sdk::start()` function does not exist in `crates/vo-sdk/src/lib.rs`.

**Current macro behavior (`vo-sdk-macros/src/task.rs`):**
The `#[task_macro]` generates a simple `main()` that directly calls the annotated function:
```rust
// Generated code for: #[task] fn my_task() {}
fn main() { my_task(); }

// Generated code for: #[task] async fn my_task() {}
fn main() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("...");
    rt.block_on(async { my_task().await; })
}
```

**Problems:**
1. No CLI argument parsing for `--execute-node <name>`
2. No task dispatch based on node name
3. No FD3 input reading / FD4 output writing
4. The macro only generates single-task entrypoints, not multi-task dispatch

### Required Implementation (per ADR-009/ADR-011)

```rust
// vo_sdk should provide:
pub fn start(workflow: &WorkflowSpec, args: &[String]) {
    // 1. Parse --execute-node <name>
    // 2. Find matching node in workflow
    // 3. Read input from FD3
    // 4. Execute node function
    // 5. Write result to FD4
}
```

---

## 4. Architecture Gap Analysis

### 4.1 Dispatch Gap (Critical)

The architecture-spec.md section 8.10 (`vel-y7g`) identifies this exact problem:
> "macro bead now also carries ADR-009 dispatch logic and may imply nonexistent crate/layout assumptions."

**Required action:** Rewrite and split - separate macro crate existence from generated `--graph` / `--execute-node` dispatch behavior.

### 4.2 SDK Scaffold Gap (ADR-011 Section 8.11 `vel-edo`)

> "in-progress, but still framed as SDK scaffold instead of freeze-set-aligned protocol coverage."
> "Required action: rewrite in place or supersede"
> "Replacement rule: split read/write helpers, single-write guard, graph emission helpers, and execute-node dispatch helpers; keep ownership in real vo-sdk crate."

---

## 5. Test Evidence

### 5.1 vo-worker test references `--execute-node`
**File:** `crates/vo-worker/tests/qa_worker.rs:97`
```rust
command: "vo-binary --execute-node start".into(),
```
This confirms the expected CLI contract, but no implementation exists to handle it.

### 5.2 vo-executor error types exist
**File:** `crates/vo-executor/src/errors.rs`
`ExecuteNodeError` enum has variants for step_not_found, timeout, etc., but these are for the executor's internal state machine, not for CLI dispatch.

---

## 6. Findings Summary

| Component | Status | Notes |
|-----------|--------|-------|
| `--graph` discovery | ✅ Works | Implemented in vo-sdk |
| `--execute-node` routing | ❌ Missing | vo_sdk::start() doesn't exist |
| vo-linter compilation | ✅ Fixed | Added missing import |
| vo-api compilation | ❌ Broken | OrchestratorMsg incomplete |
| Full build | ❌ Blocked | vo-api errors prevent compilation |

---

## 7. Recommended Actions

### Critical (blocks compilation)
1. **vo-api/orchestrator gap**: Either implement missing `OrchestratorMsg` variants (`StartWorkflow`, `GetStatus`, `ListActive`) or remove the dependent handler code.

### High (ADR-009 implementation)
2. **Implement `vo_sdk::start()`**: Create the CLI dispatch function that parses `--execute-node <name>` and routes to the correct task function.
3. **FD3/FD4 I/O integration**: Wire up the input reading and output writing in the SDK.
4. **Macro enhancement**: Update `#[task_macro]` to generate multi-task dispatch code instead of single-task main().

### Medium (architectural)
5. **Address `vel-y7g`**: Separate macro crate from ADR-009 dispatch logic.
6. **Address `vel-edo`**: Split vo-sdk into read/write helpers, graph emission, and execute-node dispatch modules.

---

## 8. Evidence of Review

- Read ADR-009 (v2 multi-task binary)
- Read ADR-011 (async current-thread runtime)  
- Read vo-sdk lib.rs, graph_args.rs, dag.rs
- Read vo-sdk-macros lib.rs, task.rs
- Read vo-executor lib.rs, execution.rs, subprocess.rs
- Read vo-actor OrchestratorMsg enum
- Read vo-linter random.rs (fixed import error)
- Attempted cargo build, analyzed errors
- Verified `--graph` CLI handling exists
- Verified `--execute-node` CLI handling is missing
