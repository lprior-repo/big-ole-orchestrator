# Audit Findings: vo-sdk Error Paths (bead tw-1a7)

## Executive Summary

Audit of vo-sdk error handling, edge cases, and panic paths identified **11 failing tests** out of 406.
These represent either bugs in the SDK or incorrect test assertions documenting expected behavior.

---

## Test Results: 395 passed, 11 failed

### Category 1: I/O and Size Boundary Issues

**1. `bh48_write_success_exactly_at_max_output_size`**
- **Expected**: bytes.len() == 10485760 (exactly MAX_OUTPUT_SIZE)
- **Actual**: bytes.len() == 10485764
- **Root Cause**: JSON serialization overhead calculation in test is incorrect. The test constructs a payload by computing `overhead = json_str.len() - 4` which doesn't account properly for the envelope structure.
- **Impact**: Test assertion is wrong, not the SDK. The SDK correctly rejects payloads > 10MB.

**2. `bh48_write_success_just_under_max_output_size`**
- **Same issue as #1**: Test's byte calculation is off by 4 bytes.

---

### Category 2: Guard State Machine Issues

**3. `bh48_atomic_guard_set_after_empty_read`**
- **Expected**: `Err(SdkError::FdNotOpen)` for empty read
- **Actual**: `Err(SdkError::InvalidInput)` for empty read
- **Root Cause**: In `io.rs::read_and_parse()`, empty input (len=0) returns `InvalidInput` before the atomic guard is checked. The guard-check-first semantics expected by the test is not implemented.
- **Impact**: Low - SDK still correctly rejects empty input, just with different error code.

**4. `bh48_concurrent_read_and_write_do_not_interfere`**
- **Expected**: Both read and write should succeed when using separate guards
- **Actual**: Write fails with `AlreadyWritten`
- **Root Cause**: The test uses `write_success_inner_with_state` which takes a local `is_written` boolean, but the test also uses a shared `AtomicBool` guard that isn't actually passed to the write function correctly. The write guard isn't shared across threads.
- **Impact**: Test implementation bug, not SDK bug.

**5. `bh48_concurrent_writes_both_fail_after_one_succeeds`**
- **Expected**: At least 7 of 8 writes should fail with `AlreadyWritten`
- **Actual**: Assertion fails
- **Root Cause**: Same as #4 - the local `is_written` state doesn't properly coordinate between threads.
- **Impact**: Test implementation bug, not SDK bug.

---

### Category 3: DAG Validation Issues

**6. `bh48_dag_build_one_connected_one_orphan_rejected`**
- **Expected**: `Err(DagError::OrphanNode)` when node 'c' has no edges
- **Actual**: `Ok(())` - build succeeds
- **Root Cause**: When a DAG has nodes [a, b, c] and edges [(a, b)], the orphan detection doesn't work correctly. The algorithm processes nodes with in_degree=0 via topological sort, but orphan detection identifies orphans by checking `in_degree[i] == 0 && out_degree[i] == 0` for unvisited nodes. The issue is that orphan detection is checking nodes that were NEVER in the initial queue, but when a node has no incoming edges, it's in the initial queue and thus "visited".
- **Impact**: HIGH - The SDK incorrectly accepts DAGs with orphan nodes.

**7. `bh48_dag_build_orphan_node_rejected`**
- **Same issue as #6**: Two disconnected nodes both have in_degree=0 and out_degree=0. Both are added to the initial queue and marked visited. No orphans detected.
- **Impact**: HIGH - Orphan node detection is fundamentally broken.

---

### Category 4: Behavioral Inconsistencies

**8. `bh48_graph_args_duplicate_graph_flag_in_graph_args_module`**
- **Expected**: `parse_graph_args` in `graph_args.rs` should accept duplicate `--graph` flags (different from `graph.rs` behavior)
- **Actual**: The test EXPECTS `Ok(GraphArgs)` but the code returns `Err(UnrecognizedArgument)`
- **Root Cause**: `graph_args.rs::parse_graph_args` does NOT have the duplicate-check that `graph.rs::parse_graph_args` has. The test imports from `crate::graph` which uses the version with the check. So the test assertion is wrong - it expects the graph_args.rs behavior but uses the graph.rs import.
- **Impact**: Test bug - documents a behavioral difference that doesn't actually exist.

---

### Category 5: Idempotency Key Validation

**9. `read_numeric_idempotency_key_returns_invalid_input`**
- **Expected**: `Err(SdkError::InvalidInput)` for idempotency_key = "12345"
- **Actual**: `Ok(TaskInput { idempotency_key: IdempotencyKey("12345"), ... })`
- **Root Cause**: `IdempotencyKey::parse` uses `is_identifier_char` which accepts digits (0-9). A numeric-only string passes validation because `c.is_ascii_alphanumeric()` returns true for digits.
- **Impact**: Medium - The SDK accepts keys that may not be intended. Whether this is a bug or by design depends on requirements.

---

### Category 6: Serialization Issues

**10. `given_workflow_spec_when_serialized_then_guarantee_class_round_trips`**
- **Expected**: JSON contains `"guarantee_class":"exact-once"`
- **Actual**: The assertion fails
- **Root Cause**: The `GuaranteeClass` enum serializes differently than expected. Need to verify the actual serialization format.
- **Impact**: Medium - Guarantee class may not round-trip correctly through JSON.

---

### Category 7: Serde Deserialization

**11. `rq_workflow_spec_accepts_duplicate_edges_via_serde`**
- **Expected**: `Ok(...)` - duplicate edges should be accepted
- **Actual**: `Err(Error("duplicate edge: a -> b", ...))`
- **Root Cause**: The `WorkflowSpec::deserialize` implementation in `graph.rs` explicitly checks for duplicate edges and rejects them. The test name says "accepts" but the code "rejects".
- **Impact**: The test assertion is wrong. The SDK correctly rejects duplicate edges via serde deserialization.

---

## Summary of Issues by Severity

| Severity | Count | Issues |
|----------|-------|--------|
| HIGH | 2 | #6, #7 - Orphan node detection broken |
| Medium | 3 | #9 - Idempotency key validation, #10 - Guarantee class serialization, #5 - Concurrent write test |
| LOW | 6 | Test bugs and size boundary calculation issues |

---

## Recommendations

1. **Orphan Detection (HIGH)**: The `Dag::build` orphan detection logic needs review. The algorithm should identify nodes that are unreachable from any entry point, not just nodes with zero in/out degree.

2. **Idempotency Key (Medium)**: If numeric-only keys should be rejected, the validation in `IdempotencyKey::parse` needs to add a check like `input.chars().next().map_or(false, |c| c.is_ascii_digit())` to reject keys starting with digits.

3. **Guarantee Class (Medium)**: Verify the serialization format of `GuaranteeClass` enum.

4. **Test Assertions (LOW)**: Several tests have incorrect assertions that document expected behavior that doesn't match the actual implementation. These tests should be updated to match actual behavior, OR the implementation should be changed to match the documented expected behavior.
