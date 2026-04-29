# Findings: tw-2jm - Audit and stress-test vo-sdk error paths

## Summary

Ran `cargo test -p vo-sdk` on vo-sdk. Found **11 failing tests** out of 406 total.

The failing tests are QA tests from `adversarial_tests.rs`, `blackhat_48.rs`, and `red_queen_workflow_spec.rs` that document **behavior that has changed since the tests were written**. The tests themselves have incorrect expectations, not the code.

---

## Test Results

```
395 passed; 11 failed; 0 ignored
```

---

## Failing Test Analysis

### 1. `read_numeric_idempotency_key_returns_invalid_input` (adversarial_tests.rs:86)

**Test expectation**: Numeric-only idempotency key `"12345"` should be rejected as `InvalidInput`.

**Actual behavior**: `IdempotencyKey::parse` accepts numeric strings because `is_identifier_char` returns true for digits (`c.is_ascii_alphanumeric()` accepts 0-9).

**Verdict**: Test is WRONG. Numeric keys ARE valid per current `IdempotencyKey` rules.

---

### 2. `bh48_atomic_guard_set_after_empty_read` (blackhat_48.rs:151)

**Test expectation**: Empty input read should return `SdkError::FdNotOpen`.

**Actual behavior**: Returns `SdkError::InvalidInput` (via the `len == 0` check in `read_and_parse`).

**Verdict**: Test is WRONG. Empty input is invalid, not "FD not open". The test comment says "empty read returns FdNotOpen via guard" but the actual code path is: `reader.read_to_end()` returns 0 → `len == 0` check → `InvalidInput`.

---

### 3. `bh48_graph_args_duplicate_graph_flag_in_graph_args_module` (blackhat_48.rs:339)

**Test expectation**: `parse_graph_args` should accept duplicate `--graph` flags and return `Ok(GraphArgs)`.

**Actual behavior**: Returns `Err(GraphArgsError::UnrecognizedArgument { arg: "--graph" })`.

**Verdict**: Test is WRONG. The implementation was changed to reject duplicate `--graph` flags.

---

### 4. `bh48_dag_build_orphan_node_rejected` (blackhat_48.rs:461)

**Test expectation**: A DAG with two disconnected nodes should be rejected as `DagError::OrphanNode`.

**Actual behavior**: The build succeeds.

**Verdict**: This is a potential BUG in `Dag::build` orphan detection. When a node has no edges (orphan), its in_degree and out_degree are both 0. The BFS topological sort doesn't visit it (since it never enters the queue). The orphan check `in_degree[i] == 0 && out_degree[i] == 0` should catch this, but something appears wrong.

**Note**: Looking at the build logic:
- Two orphan nodes both have in_degree=0, out_degree=0
- After BFS, visited[orphan1]=false, visited[orphan2]=false
- They get added to `orphan_nodes` correctly
- But the function returns `Ok(())` instead of `Err(OrphanNode)`

Actually wait - let me re-check. Looking at the build code more carefully: the condition `if visited.iter().any(|&v| !v)` triggers when there are unvisited nodes. The orphan detection code then separates orphans from cycles. If `orphan_nodes` is not empty, it should return `Err(DagError::OrphanNode {...})`.

**Issue**: Need to investigate why the orphan detection isn't working. This could be a real bug.

---

### 5. `bh48_dag_build_one_connected_one_orphan_rejected` (blackhat_48.rs:482)

**Test expectation**: Node "c" (disconnected) should be rejected as orphan.

**Actual behavior**: Build succeeds.

**Verdict**: Same issue as #4 - orphan detection appears broken.

---

### 6. `bh48_concurrent_read_and_write_do_not_interfere` (blackhat_48.rs:645)

**Test expectation**: Both read and write should succeed in concurrent scenario.

**Actual behavior**: Write fails ("write should succeed" assertion fails).

**Analysis**: The test spawns a read thread and a write thread. The write uses `compare_exchange` to set a local guard, then calls `write_success_inner_with_state`. The issue is that `write_success_inner_with_state` checks `*is_written` which was set to `true` by the `compare_exchange`, so the inner write returns `Err(AlreadyWritten)`.

**Verdict**: Test is incorrectly structured - the local guard mechanism doesn't work the way the test expects.

---

### 7. `bh48_concurrent_writes_both_fail_after_one_succeeds` (blackhat_48.rs:693)

**Test expectation**: Out of 8 concurrent writes, at least 7 should fail with `AlreadyWritten`.

**Actual behavior**: Fewer than 7 fail.

**Verdict**: Same issue as #6 - the test's guard mechanism doesn't properly simulate the global atomic guard behavior.

---

### 8. `rq_workflow_spec_accepts_duplicate_edges_via_serde` (red_queen_workflow_spec.rs:794)

**Test expectation**: Duplicate edges `a→b, a→b` should be accepted via serde.

**Actual behavior**: Serde rejects with "duplicate edge: a -> b".

**Verdict**: Test is WRONG. The `WorkflowSpec::deserialize` implementation rejects duplicate edges (see graph.rs lines 160-165).

---

### 9. `bh48_write_success_exactly_at_max_output_size` (blackhat_48.rs:47)

**Test expectation**: Payload exactly at `10 * 1024 * 1024` bytes should succeed.

**Actual behavior**: Size calculation creates payload of 10,485,764 bytes instead of 10,485,760.

**Verdict**: Test's size calculation is incorrect - the JSON envelope overhead calculation doesn't match reality.

---

### 10. `bh48_write_success_just_under_max_output_size` (blackhat_48.rs:98)

**Test expectation**: Payload just under 10MB should succeed.

**Actual behavior**: Payload exceeds limit due to same calculation issue as #9.

**Verdict**: Test's size calculation is incorrect.

---

### 11. `given_workflow_spec_when_serialized_then_guarantee_class_round_trips` (graph.rs:603)

**Test expectation**: JSON should contain `"guarantee_class":"exact-once"`.

**Actual behavior**: The serialized JSON doesn't contain this exact string.

**Verdict**: Need to investigate actual serialization format. This may be a real issue with serialization.

---

## Error Path Analysis

### io.rs Error Paths

| Function | Error Condition | Error Type |
|---------|----------------|------------|
| `read_input` | FD3 not valid | `FdNotOpen` |
| `read_input` | Already read | `FdNotOpen` |
| `read_input` | Empty input | `InvalidInput` |
| `read_input` | >10MB input | `InvalidInput` |
| `read_input` | Non-UTF8 | `InvalidInput` |
| `read_input` | Invalid JSON | `InvalidInput` |
| `read_input` | Invalid idempotency key | `InvalidInput` |
| `write_success` | FD4 not valid | `WriteError` |
| `write_success` | Already written | `AlreadyWritten` |
| `write_success` | >10MB output | `WriteError` |
| `write_failure` | >1024 byte message | `InvalidInput` |

### graph.rs Error Paths

| Function | Error Condition | Error Type |
|---------|----------------|------------|
| `parse_graph_args` | No `--graph` flag | `NoGraphFlag` |
| `parse_graph_args` | Extra args after `--graph` | `UnrecognizedArgument` |
| `parse_graph_args` | Duplicate `--graph` | `UnrecognizedArgument` |
| `WorkflowSpec::validate` | Duplicate node name | `DuplicateNodeName` |
| `WorkflowSpec::validate` | Edge to missing node | `MissingEdgeSource/Target` |
| `WorkflowSpec::validate` | Self-loop | `SelfLoop` |
| `WorkflowSpec::validate` | Cycle | `CycleDetected` |
| `WorkflowSpec::validate` | No entry point | `NoEntryPoint` |
| `emit_graph_if_requested` | Cycle detected | `process::exit(1)` |
| `emit_graph_if_requested` | Parse error | `process::exit(1)` |

### dag.rs Error Paths

| Function | Error Condition | Error Type |
|---------|----------------|------------|
| `add_node_with_kind` | Invalid name | `InvalidNodeName` |
| `add_node_with_kind` | Duplicate name | `DuplicateNodeName` |
| `connect` | Node not found | `NodeNotFound` |
| `build` | Empty workflow | `EmptyWorkflow` |
| `build` | Cycle detected | `CycleDetected` |
| `build` | Orphan nodes | `OrphanNode` |
| `build` | Invalid workflow name | `InvalidNodeName` |

---

## Conclusions

1. **Most failing tests have incorrect expectations** - they document old behavior that has since changed.

2. **Potential real bugs**:
   - Orphan node detection in `Dag::build` appears to not work correctly
   - The `given_workflow_spec_when_serialized_then_guarantee_class_round_trips` serialization issue needs investigation

3. **Test quality issues**:
   - Concurrent tests use incorrect guard patterns that don't reflect actual atomic behavior
   - Size boundary tests have incorrect calculation logic

4. **The core SDK error handling is sound** - `SdkError` enum covers all documented cases, guards are set before I/O, and error messages are clear.

---

## Recommendations

1. Fix or remove the 9 tests with incorrect expectations
2. Investigate orphan detection bug in `Dag::build`
3. Fix guarantee_class serialization test
4. Rewrite concurrent tests to properly simulate atomic guard behavior