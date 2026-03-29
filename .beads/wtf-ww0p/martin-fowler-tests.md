# Martin Fowler Test Plan: vo-ww0p -- E2E Workflow Completion Test

## Verified Facts (source-read 2026-03-23)

| Fact | Source | Value |
|------|--------|-------|
| V3StartResponse.namespace | `workflow_mappers.rs:62` | **Always `""`** (hardcoded, not from request) |
| URL path param | `routes.rs:22` | Single `:id` captures `namespace/instance_id` |
| URL encoding | `mod.rs:74` split_path_id | `/` in path must be `%2F`-encoded by client |
| paradigm "" | `mod.rs:80` parse_paradigm | `None` -> 400 `invalid_paradigm` |
| valid: false path | `definitions.rs:46-49` | HTTP 200 with `{ valid: false, diagnostics }` |
| AlreadyExists | `workflow_mappers.rs:80-84` | HTTP 409 `{ error: "already_exists" }` |
| KvStores constructor | `kv.rs:38` | `provision_kv_buckets(&js) -> Result<KvStores, VoError>` (no `db` param, no `::new()`) |
| Lint rules used by handler | `definitions.rs:21`, `lib.rs:24` | `vo_linter::lint_workflow_code` re-exports **l005 only** (tokio::spawn check) |
| Test source passes l005 | `l005.rs` | No `tokio::spawn` in source -> clean pass |

## URL Construction Reference

The API routes use a single `:id` path segment (see `routes.rs:22`):
```
GET /api/v1/workflows/:id
GET /api/v1/workflows/:id/journal
```

The `:id` must carry `namespace/instance_id` as a URL-encoded compound value:
- Client encodes: `namespace` + `%2F` + `instance_id`
- Example: `GET /api/v1/workflows/e2e%2F01ARZ3NDEKTSV4RRFFQ69G5FAV`
- Server splits via `split_path_id()` at first `/` (after URL decoding by axum)

## Happy Path Tests
- `e2e_definition_ingestion_returns_valid_for_clean_procedural_source`
- `e2e_start_workflow_returns_201_with_instance_id`
- `e2e_journal_contains_entries_after_workflow_start`
- `e2e_workflow_status_returns_matching_response`
- `e2e_list_workflows_includes_started_instance`

## Error Path Tests
- `e2e_invalid_paradigm_returns_400` -- unknown paradigm string
- `e2e_empty_paradigm_returns_400_invalid_paradigm` -- empty string paradigm
- `e2e_invalid_namespace_returns_400` -- whitespace in namespace
- `e2e_definition_with_empty_workflow_type_returns_400` -- empty workflow_type field
- `e2e_definition_with_malformed_source_returns_400` -- unparseable Rust source
- `e2e_definition_with_lint_errors_returns_200_valid_false` -- parseable but has lint violations
- `e2e_duplicate_instance_id_start_returns_409` -- same instance_id started twice

## Edge Case Tests
- `e2e_journal_empty_before_workflow_start` -- journal for non-existent instance returns 404
- `e2e_status_for_nonexistent_instance_returns_404` -- GET status for unknown instance_id

## Contract Verification Tests
- `test_precondition_nats_unreachable_fails_with_connection_error` -- verify NATS connection failure propagates as `Err`
- `test_precondition_nats_unreachable_returns_error_before_orchestrator_spawn` -- verify failure happens at `connect()`, before stream provisioning or actor spawn
- `test_precondition_stream_provision_required_before_orchestrator` -- verify orchestrator needs provisioned streams
- `test_postcondition_definition_response_shape_matches_dto` -- response deserializes to `DefinitionResponse { valid, diagnostics }`
- `test_postcondition_start_response_shape_matches_dto` -- response deserializes to `V3StartResponse { instance_id, namespace, workflow_type }`
- `test_postcondition_status_response_shape_matches_dto` -- response deserializes to `V3StatusResponse { instance_id, namespace, workflow_type, paradigm, phase, events_applied }`
- `test_postcondition_journal_response_shape_matches_dto` -- response deserializes to `JournalResponse { invocation_id, entries }`
- `test_invariant_journal_seq_strictly_ascending` -- entries are always sorted by ascending seq
- `test_invariant_fresh_streams_per_test` -- resetting streams before each test prevents cross-contamination
- `test_invariant_ephemeral_port_no_conflicts` -- two sequential E2eTestServer instances bind to different ports

## Given-When-Then Scenarios

### Scenario 1: Definition ingestion returns valid for clean procedural source
**Given**: a running vo-api server connected to real NATS JetStream
**And**: a procedural workflow source with no l005 violations (no `tokio::spawn`):
```
"impl WorkflowFn for EchoWorkflow {
    async fn execute(&self, ctx: WorkflowContext) -> anyhow::Result<()> {
        let _ = 42;
        Ok(())
    }
}"
```
> **Note (m1)**: The definitions handler calls `vo_linter::lint_workflow_code` which only runs
> l005 (tokio::spawn detection). This source has no closures, no spawn, no async IO, no time
> calls, and no thread ops -- it passes l005 with zero diagnostics.

**When**: `POST /api/v1/definitions/procedural` is called with body `{ "source": "<source>", "workflow_type": "echo" }`
**Then**:
- HTTP 200 is returned
- Response body matches `DefinitionResponse { valid: true, diagnostics: [] }`

### Scenario 2: Start workflow returns 201 with instance_id
**Given**: a running vo-api server connected to real NATS JetStream
**When**: `POST /api/v1/workflows` is called with body:
```json
{
  "namespace": "e2e",
  "workflow_type": "echo",
  "paradigm": "procedural",
  "input": {}
}
```
**Then**:
- HTTP 201 is returned
- Response body matches `V3StartResponse { instance_id: "<non-empty ULID string>", namespace: "", workflow_type: "echo" }`

> **FACT (C1)**: `map_start_result` in `workflow_mappers.rs:62` hardcodes `namespace: "".to_owned()`.
> The response namespace is always empty string, NOT the namespace from the request.
> The assertion must expect `namespace: ""`, not `namespace: "e2e"`.

- `instance_id` is exactly 26 characters (standard ULID length)

### Scenario 3: Journal contains entries after workflow start
**Given**: a running vo-api server connected to real NATS JetStream
**And**: a workflow has been started via `POST /api/v1/workflows` returning `instance_id = "<id>"`
**When**: `GET /api/v1/workflows/e2e%2F<id>/journal` is polled every 200ms for up to 10 seconds
**Then**:
- HTTP 200 is eventually returned
- Response body matches `JournalResponse { invocation_id: "e2e/<id>", entries: [<at least one entry>] }`
- All `entries[].seq` values are strictly ascending (each `seq > previous seq`)
- At least one entry has `entry_type: "Run"` or `entry_type: "Wait"`
- At least one entry has a non-null `timestamp` in RFC 3339 format

> **URL construction (C2)**: The route is `/api/v1/workflows/:id` where `:id` is a single
> axum path segment. To pass `namespace/instance_id`, the client must URL-encode the `/`
> as `%2F`. Example: `/api/v1/workflows/e2e%2F01ARZ3NDEKTSV4RRFFQ69G5FAV/journal`.
> The server's `split_path_id()` decodes this back to `("e2e", "01ARZ3NDEKTSV4RRFFQ69G5FAV")`.

### Scenario 4: Workflow status returns V3StatusResponse with live phase
**Given**: a running vo-api server connected to real NATS JetStream
**And**: a workflow has been started with `instance_id = "<id>"`
**When**: `GET /api/v1/workflows/e2e%2F<id>` is called after a brief settle delay
**Then**:
- HTTP 200 is returned
- Response body matches `V3StatusResponse` with:
  - `instance_id == "<id>"` (exact string match)
  - `namespace == "e2e"` (from `InstanceStatusSnapshot.namespace`, NOT from start response)
  - `workflow_type == "echo"`
  - `paradigm == "procedural"`
  - `phase == "live"`
  - `events_applied >= 1`

> **Note (m2)**: The `phase == "live"` assertion has a timing dependency. The workflow
> actor transitions from replay phase to live phase after processing initial events.
> If the status query races ahead of the actor state machine, phase may still be `"replay"`.
> Mitigation: poll status with retry (same pattern as journal polling) or insert a settle
> delay. The orchestrator's `InstancePhaseView::Live` is set after `transition_to_live` completes.

### Scenario 5: List workflows includes the started instance
**Given**: a running vo-api server connected to real NATS JetStream
**And**: a workflow has been started with `instance_id = "<id>"`
**When**: `GET /api/v1/workflows` is called
**Then**:
- HTTP 200 is returned
- Response body is a JSON array
- The array contains at least one element
- At least one element has `instance_id == "<id>"`

### Scenario 6: Invalid paradigm (unknown string) returns 400
**Given**: a running vo-api server connected to real NATS JetStream
**When**: `POST /api/v1/workflows` is called with body:
```json
{
  "namespace": "e2e",
  "workflow_type": "bad",
  "paradigm": "quantum_computing",
  "input": {}
}
```
**Then**:
- HTTP 400 is returned
- Response body matches `ApiError { error: "invalid_paradigm", message: "bad paradigm" }`

### Scenario 7: Empty paradigm returns 400 invalid_paradigm (C3)
**Given**: a running vo-api server connected to real NATS JetStream
**When**: `POST /api/v1/workflows` is called with body:
```json
{
  "namespace": "e2e",
  "workflow_type": "echo",
  "paradigm": "",
  "input": {}
}
```
**Then**:
- HTTP 400 is returned
- Response body matches `ApiError { error: "invalid_paradigm", message: "bad paradigm" }`

> **FACT (C3)**: `parse_paradigm("")` returns `None` (confirmed in workflow.rs:170 test).
> `validate_start_req` maps `None` to 400 with `invalid_paradigm`. This is a separate
> error path from unknown paradigm strings -- both hit the same error code.

### Scenario 8: Invalid namespace returns 400
**Given**: a running vo-api server connected to real NATS JetStream
**When**: `POST /api/v1/workflows` is called with body:
```json
{
  "namespace": "has spaces!",
  "workflow_type": "bad",
  "paradigm": "procedural",
  "input": {}
}
```
**Then**:
- HTTP 400 is returned
- Response body matches `ApiError { error: "invalid_namespace", message: "bad namespace" }`

> **Note**: `NamespaceId::try_new` rejects strings containing whitespace (space is
> `c.is_whitespace()` in `validate_nats_component`). But empty string `""` is accepted
> (no illegal chars found).

### Scenario 9: Definition with empty workflow_type returns 400
**Given**: a running vo-api server connected to real NATS JetStream
**When**: `POST /api/v1/definitions/procedural` is called with body:
```json
{
  "source": "fn valid() {}",
  "workflow_type": ""
}
```
**Then**:
- HTTP 400 is returned
- Response body matches `ApiError { error: "invalid_request", message: "workflow_type must be non-empty" }`

### Scenario 10: Definition with malformed source returns 400
**Given**: a running vo-api server connected to real NATS JetStream
**When**: `POST /api/v1/definitions/procedural` is called with body:
```json
{
  "source": "!!!not valid rust syntax",
  "workflow_type": "bad-workflow"
}
```
**Then**:
- HTTP 400 is returned
- Response body matches `ApiError { error: "parse_error" }`

### Scenario 11: Definition with lint errors returns 200 { valid: false } (M1)
**Given**: a running vo-api server connected to real NATS JetStream
**When**: `POST /api/v1/definitions/procedural` is called with body:
```json
{
  "source": "impl WorkflowFn for BadWorkflow { async fn execute(&self, _ctx: WorkflowContext) -> anyhow::Result<()> { tokio::spawn(async {}); Ok(()) } }",
  "workflow_type": "lint-violator"
}
```
**Then**:
- HTTP 200 is returned
- Response body matches `DefinitionResponse { valid: false, diagnostics: [<non-empty array>] }`
- At least one diagnostic has `code: "L005"` and `severity: "error"`

> **FACT (M1)**: The definitions handler at `definitions.rs:21` calls `vo_linter::lint_workflow_code`
> (l005 re-export). If parsing succeeds but l005 fires an error-severity diagnostic (e.g.
> `tokio::spawn`), the handler returns HTTP 200 with `valid: false` and the diagnostic details.
> The `valid` flag is `dtos.iter().all(|d| d.severity != "error")` -- so any error-severity
> diagnostic makes `valid == false`. The definition is NOT stored in KV when `valid == false`
> (see `definitions.rs:34-48`: KV write only happens inside `if valid` block).

### Scenario 12: Duplicate instance_id start returns 409 (M2)
**Given**: a running vo-api server connected to real NATS JetStream
**And**: a workflow has been started with `instance_id = "<id>"` returned from the first start
**When**: `POST /api/v1/workflows` is called again with body:
```json
{
  "namespace": "e2e",
  "workflow_type": "echo",
  "paradigm": "procedural",
  "input": {},
  "instance_id": "<same id as first start>"
}
```
**Then**:
- HTTP 409 is returned
- Response body matches `ApiError { error: "already_exists", message: "<instance_id>" }`

> **FACT (M2)**: The orchestrator's `start.rs:48` returns `StartError::AlreadyExists(id)` when
> the instance_id is already registered. `map_start_error` in `workflow_mappers.rs:80-84` maps
> this to HTTP 409 `{ error: "already_exists" }`. Confirmed by actor test at
> `spawn_workflow_test.rs:185-186`.

### Scenario 13: Journal for non-existent instance returns 404
**Given**: a running vo-api server connected to real NATS JetStream
**When**: `GET /api/v1/workflows/e2e%2Fnonexistent-id-12345/journal` is called
**Then**:
- HTTP 404 is returned
- Response body matches `ApiError { error: "not_found" }`

### Scenario 14: Status for non-existent instance returns 404
**Given**: a running vo-api server connected to real NATS JetStream
**When**: `GET /api/v1/workflows/e2e%2Fnonexistent-id-12345` is called
**Then**:
- HTTP 404 is returned
- Response body matches `ApiError { error: "not_found" }`

### Scenario 15: NATS unreachable fails at connect (M3 -- precondition)
**Given**: no NATS server is running (or NATS_URL points to a dead host)
**When**: `E2eTestServer::new()` is called
**Then**:
- `E2eTestServer::new()` returns `Err` containing a connection error description
- The error occurs at `connect(&config)` -- before stream provisioning, KV bucket creation, or orchestrator spawn
- No orchestrator actor is created (failure is early in the boot sequence)

> **Precondition (M3)**: NATS reachability is a hard precondition. The boot sequence is:
> `connect()` -> `provision_streams()` -> `provision_kv_buckets()` -> orchestrator spawn.
> Failure at `connect()` means none of the subsequent steps execute. The test should
> verify the error message contains "connection" or "nats" to confirm it failed at the
> right stage, not at a later provisioning step.

### Scenario 16: Fresh streams per test prevent cross-contamination
**Given**: test A starts a workflow that writes events to NATS streams
**When**: test B boots a fresh `E2eTestServer` (which resets and re-provisions streams)
**Then**:
- test B's journal query returns 404 for test A's instance_id
- test B's list query returns an empty array (no leftover instances)

### Scenario 17: Sequential execution via global lock
**Given**: `global_test_lock()` uses `OnceLock<Arc<Mutex<()>>>`
**When**: test A acquires the lock and holds it for the duration of `E2eTestServer` lifetime
**Then**:
- test B blocks on `global_test_lock().lock_owned().await` until test A drops its `OwnedMutexGuard`
- No two tests ever share a NATS connection, orchestrator, or HTTP server simultaneously

### Scenario 18: Ephemeral port binding is unique per test
**Given**: test A binds to `127.0.0.1:0` and receives port P1
**When**: test B binds to `127.0.0.1:0` (after test A releases its guard)
**Then**:
- test B receives a different port P2
- `P1 != P2` (though technically possible for OS to reuse, extremely unlikely within sequential execution)
- Both ports are valid (server accepts connections)

## Boot Sequence Corrections (M4)

The `E2eTestServer::new()` boot sequence must use the ACTUAL `KvStores` constructor:

```rust
// WRONG (from original contract):
// let kv = KvStores::new(js.clone(), db.clone())?;

// CORRECT (verified from kv.rs:38):
let kv = provision_kv_buckets(&js).await?;
```

`provision_kv_buckets` takes only `&Context` and returns `Result<KvStores, VoError>`.
There is no `db` parameter. The `sled::Db` is only needed for `OrchestratorConfig.snapshot_db`.

## Test Execution Matrix

| # | Test Name | HTTP Method | Endpoint | Expected Status | Key Assertions |
|---|-----------|-------------|----------|----------------|----------------|
| 1 | `e2e_definition_ingestion_returns_valid_for_clean_procedural_source` | POST | `/api/v1/definitions/procedural` | 200 | `valid: true`, empty diagnostics |
| 2 | `e2e_start_workflow_returns_201_with_instance_id` | POST | `/api/v1/workflows` | 201 | non-empty `instance_id`, **`namespace: ""`**, `workflow_type: "echo"` |
| 3 | `e2e_journal_contains_entries_after_workflow_start` | GET | `/api/v1/workflows/e2e%2F{id}/journal` | 200 | `entries.len() > 0`, seq ascending |
| 4 | `e2e_workflow_status_returns_matching_response` | GET | `/api/v1/workflows/e2e%2F{id}` | 200 | matching `instance_id`, `namespace: "e2e"` (from snapshot), `phase: "live"` |
| 5 | `e2e_list_workflows_includes_started_instance` | GET | `/api/v1/workflows` | 200 | array contains started `instance_id` |
| 6 | `e2e_invalid_paradigm_returns_400` | POST | `/api/v1/workflows` | 400 | `error: "invalid_paradigm"` |
| 7 | `e2e_empty_paradigm_returns_400_invalid_paradigm` | POST | `/api/v1/workflows` | 400 | `error: "invalid_paradigm"` |
| 8 | `e2e_invalid_namespace_returns_400` | POST | `/api/v1/workflows` | 400 | `error: "invalid_namespace"` |
| 9 | `e2e_definition_with_empty_workflow_type_returns_400` | POST | `/api/v1/definitions/procedural` | 400 | `error: "invalid_request"` |
| 10 | `e2e_definition_with_malformed_source_returns_400` | POST | `/api/v1/definitions/procedural` | 400 | `error: "parse_error"` |
| 11 | `e2e_definition_with_lint_errors_returns_200_valid_false` | POST | `/api/v1/definitions/procedural` | 200 | `valid: false`, `diagnostics` non-empty |
| 12 | `e2e_duplicate_instance_id_start_returns_409` | POST | `/api/v1/workflows` | 409 | `error: "already_exists"` |
| 13 | `e2e_journal_for_nonexistent_instance_returns_404` | GET | `/api/v1/workflows/e2e%2Fnonexistent/journal` | 404 | `error: "not_found"` |
| 14 | `e2e_status_for_nonexistent_instance_returns_404` | GET | `/api/v1/workflows/e2e%2Fnonexistent` | 404 | `error: "not_found"` |

## Run Commands

```bash
# Full E2E suite (requires NATS)
cargo test -p vo-api --test e2e_workflow_test -- --test-threads=1 --nocapture

# Individual tests
cargo test -p vo-api --test e2e_workflow_test e2e_definition_ingestion -- --test-threads=1 --nocapture
cargo test -p vo-api --test e2e_workflow_test e2e_start_workflow -- --test-threads=1 --nocapture
cargo test -p vo-api --test e2e_workflow_test e2e_journal -- --test-threads=1 --nocapture
cargo test -p vo-api --test e2e_workflow_test e2e_workflow_status -- --test-threads=1 --nocapture
cargo test -p vo-api --test e2e_workflow_test e2e_list_workflows -- --test-threads=1 --nocapture
cargo test -p vo-api --test e2e_workflow_test e2e_invalid_paradigm -- --test-threads=1 --nocapture
cargo test -p vo-api --test e2e_workflow_test e2e_empty_paradigm -- --test-threads=1 --nocapture
cargo test -p vo-api --test e2e_workflow_test e2e_invalid_namespace -- --test-threads=1 --nocapture
cargo test -p vo-api --test e2e_workflow_test e2e_definition_empty_type -- --test-threads=1 --nocapture
cargo test -p vo-api --test e2e_workflow_test e2e_definition_malformed -- --test-threads=1 --nocapture
cargo test -p vo-api --test e2e_workflow_test e2e_definition_lint_errors -- --test-threads=1 --nocapture
cargo test -p vo-api --test e2e_workflow_test e2e_duplicate_instance_id -- --test-threads=1 --nocapture
cargo test -p vo-api --test e2e_workflow_test e2e_journal_404 -- --test-threads=1 --nocapture
cargo test -p vo-api --test e2e_workflow_test e2e_status_404 -- --test-threads=1 --nocapture

# Clippy validation
cargo clippy -p vo-api --tests -- -D warnings
```

## Defect Resolution Summary

| ID | Severity | Issue | Resolution |
|----|----------|-------|------------|
| C1 | CRITICAL | `namespace` assertion expected `"e2e"` but server returns `""` | Scenario 2 assertion updated: expect `namespace: ""` (verified `workflow_mappers.rs:62` hardcodes empty string) |
| C2 | CRITICAL | URL construction undocumented | Added "URL Construction Reference" section explaining `%2F`-encoding of compound `namespace/instance_id` path param |
| C3 | CRITICAL | Missing test for empty paradigm | Added Scenario 7: `paradigm: ""` returns 400 `invalid_paradigm` (verified `parse_paradigm("")` returns `None`) |
| M1 | MAJOR | Missing test for lint-error-but-parseable source | Added Scenario 11: `tokio::spawn` in source returns 200 `{ valid: false }` (verified `definitions.rs:46-49`) |
| M2 | MAJOR | Missing test for duplicate instance_id | Added Scenario 12: second start with same `instance_id` returns 409 `already_exists` (verified `workflow_mappers.rs:80-84`) |
| M3 | MAJOR | NATS unreachable precondition too vague | Expanded Scenario 15: documents exact failure point (`connect()`), verifies no later steps execute |
| M4 | MAJOR | `KvStores::new(js, db)` is wrong | Added "Boot Sequence Corrections": correct constructor is `provision_kv_buckets(&js)` with no `db` param (verified `kv.rs:38`) |
| m1 | MINOR | Test source linter compliance unverified | Verified: source passes l005 (the only rule run by the handler). Added note about l005-only scope. |
| m2 | MINOR | Timing dependency on phase assertion undocumented | Added note to Scenario 4: `phase == "live"` may race with actor state machine. Recommend retry polling. |

## Exit Criteria
- Every failure mode from the error taxonomy has at least one test covering it
- Every precondition has a test that verifies the system fails gracefully when violated
- Every postcondition has a happy-path test that verifies the expected outcome
- Every invariant (I-1 through I-12) has at least one test verifying it
- All test names describe behavior unambiguously without referencing implementation details
- No `unwrap()` or `expect()` in test harness code (only `?` with `Box<dyn Error>`)
- `cargo clippy -p vo-api --tests -- -D warnings` passes with zero warnings
- All assertions match verified source behavior (no assumptions without proof)
