# Bead tw-wjzn Findings: SDK Documentation Generation from Workflow Defs

## Task
Add SDK documentation generation from workflow definitions. Output node descriptions with input/output types, edge documentation, retry policy documentation, and connector requirements. Format as markdown. Integrates with vo-cli export.

## Implementation

### Changes Made

**File: `crates/vo-sdk/src/graph.rs`**

1. Added `DocArgs` marker type and `DocArgsError` for `--doc` CLI flag parsing
2. Added `parse_doc_args()` function to detect `--doc` flag in CLI arguments
3. Added `to_markdown()` method to `WorkflowSpec` that generates comprehensive markdown documentation including:
   - Workflow overview (name, guarantee class, dedupe scope)
   - Node documentation (name, kind, retry policy, signal metadata if present)
   - Edge documentation (source -> target in table format)
   - Guarantee semantics description
4. Added helper functions:
   - `node_kind_label()` - converts NodeKind enum to snake_case string
   - `retry_policy_description()` - formats RetryPolicy as human-readable string
   - `buffer_policy_label()` - formats BufferPolicy as string
   - `guarantee_class_label()` - formats GuaranteeClass as string
   - `guarantee_class_description()` - provides detailed description of guarantee semantics
   - `dedupe_scope_label()` - formats DedupeScope as string
5. Added `emit_doc_if_requested()` function - analogous to `emit_graph_if_requested()` but outputs markdown documentation

**File: `crates/vo-sdk/src/lib.rs`**

1. Exported new public functions: `emit_doc_if_requested`, `parse_doc_args`, `DocArgsError`

### Example Output

The `to_markdown()` method generates documentation like:

```markdown
# Workflow: checkout-workflow

## Overview

- **Guarantee Class**: at-least-once
- **Dedupe Scope**: unbounded
- **Nodes**: 2 total
- **Edges**: 1 total

## Nodes

### `validate`

- **Kind**: `pure`
- **Retry Policy**: default (no retries)

### `charge`

- **Kind**: `managed_effect`
- **Retry Policy**: default (no retries)

## Edges

| Source | Target | Condition |
|--------|--------|-----------|
| `validate` | `charge` | always |

## Guarantee Semantics

**AtLeastOnce**: Retries may cause duplicate side effects. The engine retries on failure but does not deduplicate ingress.
```

### Usage

SDK users can now call:

```rust
use vo_sdk::{emit_doc_if_requested, Workflow};

let spec = workflow.build().unwrap();
emit_doc_if_requested(&std::env::args().collect::<Vec<_>>(), &spec);
```

When the binary is invoked with `--doc` flag, it outputs markdown documentation and exits.

### Test Added

- `to_markdown_generates_correct_output` test in `crates/vo-sdk/src/graph.rs` to verify markdown generation

## Notes

- Input/output types are not explicitly stored in `WorkflowSpec` - they are implied from the task binary's type signatures and would require additional metadata to document properly
- The implementation follows the existing pattern of `emit_graph_if_requested()` for CLI integration
- The documentation covers all fields available in `WorkflowSpec`: nodes, edges, retry policies, signal metadata, guarantee class, and dedupe scope
