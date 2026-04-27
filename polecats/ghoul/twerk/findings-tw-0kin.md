# Findings: tw-0kin - QA Audit of twerk-cli task/queue/trigger handlers

## Summary
Ran code review and manual QA audit against newly added twerk-cli task/queue/trigger handlers.
Build is BROKEN - cannot run test suite or manual QA due to pre-existing compilation errors in dispatch.rs.

## Pre-existing Build Errors (UNRELATED to task/queue/trigger handlers)

**File**: `crates/twerk-cli/src/cli/dispatch.rs`
**Error 1**: E0027 - Pattern does not mention field `password` at line 210
```rust
UserCommand::Create { username } => {  // MISSING: password field
    handlers::user::user_create(ep_str, &username, json_mode).await?;
}
```
**Error 2**: E0061 - Function takes 4 arguments but 3 were supplied
- `user_create` expects 4 args but only 3 passed at line 211

## Code Review Findings for task.rs, queue.rs, trigger.rs

### Issue 1: queue_delete ignores API response body (MEDIUM)
**File**: `queue.rs:100-126`
**Problem**: `queue_delete` always returns hardcoded JSON regardless of server response:
```rust
Ok(format!(r#"{{"deleted":true,"name":"{}"}}"#, name))
```
If server returns different body (e.g., `{"success": true}`), it's ignored.
**Recommendation**: Parse actual server response instead of fabricating JSON.

### Issue 2: Redundant success check in trigger_delete (LOW)
**File**: `trigger.rs:311`
**Problem**: `if status == reqwest::StatusCode::NO_CONTENT || status.is_success()` - NO_CONTENT (204) is already a success status, so `|| status.is_success()` is redundant.
**Dead Code**: The subsequent `Err(CliError::HttpStatus...)` at line 322 is unreachable via this branch.

### Issue 3: Inconsistent error handling patterns (LOW)
**File**: `trigger.rs` vs `queue.rs`
**Problem**:
- `trigger_list` (line 45-59): Returns `ApiError` for non-success status
- `queue_list` (line 24-29): Returns `HttpStatus` for non-success status
- `trigger_get` (line 105-112): Returns `ApiError` for NOT_FOUND
- `queue_get` (line 69-71): Returns `NotFound` for NOT_FOUND

Inconsistent error types make it harder for callers to handle errors uniformly.

### Issue 4: Missing validation for required fields (MEDIUM)
**File**: `queue.rs:10-15`
**Problem**: `QueueInfo` struct has non-optional `name: String` without `#[serde(default)]`:
```rust
pub struct QueueInfo {
    pub name: String,  // If API returns no name, deserialization fails
    pub size: i32,
    pub subscribers: i32,
    pub unacked: i32,
}
```
If API response is `{}` or missing `name`, `serde_json::from_str` returns error.
Compare to `TaskResponse` which uses `Option<String>` for all fields - more defensive.

### Issue 5: trigger_create only handles CREATED (201) explicitly (LOW)
**File**: `trigger.rs:191-200`
**Problem**: If API returns 200 OK (instead of 201 Created), the response still gets printed but no explicit handling. Code then falls through to line 202 and returns `Ok(body)` anyway, so not a bug but inconsistent with how other handlers check for specific success codes.

### Issue 6: No timeout on HTTP requests (MEDIUM)
**File**: `task.rs`, `queue.rs`, `trigger.rs`
**Problem**: All handlers use `reqwest::get()` or `client.get().send()` without explicit timeouts. If the API is unresponsive, the CLI will hang indefinitely.
**Example** (task.rs:61):
```rust
let response = reqwest::get(&url).await.map_err(CliError::Http)?;
```
**Recommendation**: Use `reqwest::Client` with configured timeout:
```rust
let client = reqwest::Client::builder()
    .timeout(Duration::from_secs(10))
    .build()?;
```

## Test Coverage Assessment

### Existing Tests
- `trigger_contract_regression_test.rs`: Good coverage for trigger handlers (RFC3339 timestamps, 204 handling)
- `bdd_behavioral_contract_test.rs`: Only tests CLI constants, errors, commands enum - NOT handlers
- `e2e_cli_test.rs`: Only tests --help, --version, --json flag behavior

### Missing Tests
1. **NO tests for task handlers** (`task_get`, `task_log`)
2. **NO tests for queue handlers** (`queue_list`, `queue_get`, `queue_delete`)
3. **No integration tests** for error paths (network failure, malformed JSON)
4. **No tests for timeout behavior**

## Bead Reference
Parent bead: `tw-avcw` (cli: Implement task, queue, and trigger command handlers)
This QA audit found that the implementation compiles (except dispatch.rs pre-existing bug) but has inconsistent error handling and missing defensive coding practices.

## Verification
- Build: BROKEN (pre-existing dispatch.rs error)
- Tests: CANNOT RUN (build fails)
- Manual QA: CANNOT RUN (binary doesn't compile)

## Recommendation
1. Fix dispatch.rs compilation errors first
2. Add unit tests for task and queue handlers
3. Standardize error handling across all handlers
4. Add HTTP timeout configuration
5. Add defensive deserialization for QueueInfo