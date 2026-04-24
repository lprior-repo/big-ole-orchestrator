# QA Audit Findings: tw-0kin

## Bead Summary
- **Bead**: tw-0kin
- **Title**: qa: audit CLI task/queue/trigger handlers
- **Type**: task (QA audit)
- **Status**: in_progress → completed

## Scope
Audit of the newly added twerk-cli task/queue/trigger handlers from bead tw-avcw.

## Code Audited
- `/home/lewis/gt/twerk/polecats/maestro/twerk/crates/twerk-cli/src/handlers/task.rs`
- `/home/lewis/gt/twerk/polecats/maestro/twerk/crates/twerk-cli/src/handlers/queue.rs`
- `/home/lewis/gt/twerk/polecats/maestro/twerk/crates/twerk-cli/src/handlers/trigger.rs`
- Related: `commands.rs`, `dispatch.rs`, `error.rs`
- Tests: `trigger_contract_regression_test.rs`, `e2e_cli_test.rs`

## Build & Test Status
- **Build**: PASSED - `cargo build -p twerk-cli` succeeds
- **Tests**: PASSED - 140 tests passed across 7 test suites

## Findings

### 1. INCONSISTENT API PATH PREFIX (Medium)

**Issue**: Task and Queue handlers use paths without `/api/v1` prefix, but Trigger handlers use `/api/v1/triggers`.

| Handler | Path Pattern |
|---------|--------------|
| Task | `/tasks/{id}`, `/tasks/{id}/log` |
| Queue | `/queues`, `/queues/{name}` |
| Trigger | `/api/v1/triggers`, `/api/v1/triggers/{id}` |

**Impact**: If the twerk API server implements consistent routing with `/api/v1` prefix for all v1 endpoints, the task and queue handlers will fail with 404 errors.

**Recommendation**: Verify if this is intentional (triggers on a different API version) or if task/queue handlers should also use `/api/v1` prefix.

### 2. Trigger Handler - Redundant NO_CONTENT Check (Low)

**File**: `trigger.rs:311-319`

```rust
if status == reqwest::StatusCode::NO_CONTENT || status.is_success() {
```

This check is unreachable in practice because:
- If status is NO_CONTENT, it's also `is_success()`, so the second condition would catch it anyway
- If status is BAD_REQUEST (line 302), we return early before reaching this check

However, the current code is not incorrect - it's just redundant.

### 3. queue_delete - Fabricated Response Body (Low)

**File**: `queue.rs:125`

```rust
Ok(format!(r#"{{"deleted":true,"name":"{}"}}"#, name))
```

The `queue_delete` function returns a fabricated JSON body regardless of the actual server response. This is inconsistent with `trigger_delete` which returns the actual (empty) body on NO_CONTENT.

If the server returns a different response body (e.g., an error message), the fabricated body will be returned instead.

### 4. Task Handler - Missing Error Response Parsing (Low)

**File**: `task.rs`

Unlike `trigger.rs` which parses `TriggerErrorResponse` for error handling, `task.rs` and `queue.rs` use simpler HTTP status code checks without parsing potential API error response bodies.

This means if the server returns a structured error response on failure, it will be ignored and only the HTTP status will be reported.

### 5. Missing Test Coverage for task/queue Handlers (Medium)

**Finding**: The `trigger_contract_regression_test.rs` provides good coverage for trigger handlers (RFC3339 timestamp validation, NO_CONTENT handling, etc.), but there are no equivalent contract tests for task and queue handlers.

**Recommendation**: Add contract tests similar to `trigger_contract_regression_test.rs` for:
- `task_get` - verify task response parsing
- `task_log` - verify log page parsing
- `queue_list` - verify queue list formatting
- `queue_get` - verify queue info parsing
- `queue_delete` - verify NO_CONTENT handling

## Test Coverage Summary
| Handler | Contract Tests | E2E Tests |
|---------|----------------|-----------|
| task | ❌ None | ❌ None |
| queue | ❌ None | ❌ None |
| trigger | ✅ Full (210 lines) | ✅ Basic |

## Recommendations (Priority Order)
1. **HIGH**: Verify API path prefix consistency - determine if task/queue should use `/api/v1` prefix
2. **MEDIUM**: Add contract tests for task and queue handlers (similar to `trigger_contract_regression_test.rs`)
3. **LOW**: Consider parsing error responses in task/queue handlers for better error messages
4. **LOW**: Remove fabricated response body in `queue_delete` or document as intentional

## Conclusion
The handlers are generally well-implemented with good error handling. The main concerns are:
1. API path inconsistency between trigger and task/queue handlers
2. Missing test coverage for task/queue handlers compared to trigger handlers

All existing tests pass (140/140), and the code compiles cleanly.

## Discovered Issues (for follow-up beads)
None that warrant immediate follow-up - all issues found are low/medium priority improvements rather than bugs.