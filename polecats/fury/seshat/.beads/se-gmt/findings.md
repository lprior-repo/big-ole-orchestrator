# BLACKHAT: Adversarial Security Testing - Batch 7 Findings

**Bead**: se-gmt
**Date**: 2026-04-24
**Reviewer**: Polecat fury (adversarial security testing)
**Scope**: Attack surface analysis of veloxide HTTP API (vo-api crate)

---

## Executive Summary

The veloxide API (vo-api) has **critical security deficiencies** in authentication, authorization, and input validation. All endpoints are currently unauthenticated and allow full control over workflow lifecycle operations. This represents a severe attack surface that would allow any client to:

- Terminate any running workflow
- Enumerate and view all workflows
- Send arbitrary signals to any workflow
- Trigger compensation on any workflow
- View complete execution history and timelines

**Verdict**: NOT APPROVED FOR PRODUCTION without security controls.

---

## 1. CRITICAL: Complete Absence of Authentication/Authorization

### Finding S-1 (CRITICAL): No Authentication on Any API Endpoint

All API endpoints in `vo-api/src/router.rs` are unauthenticated:

```rust
// router.rs:48-118 - All routes lack auth middleware
let workflow_routes = Router::new()
    .route("/api/v1/workflows", post(crate::handlers::start_workflow))
    .route("/api/v1/workflows", get(crate::handlers::list_workflows))
    .route("/api/v1/workflows/{id}", get(crate::handlers::get_workflow))
    .route("/api/v1/workflows/{id}", delete(crate::handlers::terminate_workflow))
    // ... all lack authentication
```

**Impact**: Any client can:
- `DELETE /api/v1/workflows/any-namespace/any-instance` - Terminate any workflow
- `GET /api/v1/workflows` - List ALL active workflows
- `GET /api/v1/workflows/{id}/history` - View complete execution history
- `POST /api/v1/workflows/{id}/signals` - Send arbitrary signals
- `POST /api/v1/workflows/{id}/compensate` - Trigger compensation

**Proof**: No `Authorization`, `X-API-Key`, `Bearer token`, or session cookie headers are checked in any handler.

### Finding S-2 (CRITICAL): No Authorization Checks

Even if authentication were added, there are **zero authorization checks**:

- `terminate_workflow` (line 291-357 in workflow.rs) - No check that caller owns the workflow
- `compensate_workflow` (line 148-214 in workflow_lifecycle.rs) - No authorization
- `send_signal` (signal.rs) - No authorization on signal target
- `list_workflows` (line 361-408 in workflow.rs) - Returns ALL workflows to ANY caller

**Attack scenario**: A malicious internal client can enumerate all workflows, view sensitive workflow data, and terminate/compromise any workflow.

---

## 2. HIGH: Permissive CORS Configuration

### Finding S-3 (HIGH): CORS Permissive Mode

```rust
// router.rs:116
.layer(CorsLayer::permissive())
```

**Impact**: Any website can make cross-origin requests to the API, enabling:
- CSRF attacks on state-changing endpoints (POST/DELETE)
- Data exfiltration via JavaScript

**Recommendation**: Use explicit allowed origins whitelist.

---

## 3. HIGH: Missing Rate Limiting

### Finding S-4 (HIGH): No Rate Limiting Layer

No `RateLimitLayer` is applied to any route.

**Impact**:
- DoS via workflow creation flood (`POST /api/v1/workflows`)
- DoS via expensive timeline replay queries
- Brute force enumeration of workflow IDs

**Recommendation**: Add rate limiting middleware (e.g., `tower-http`'s `RateLimitLayer`).

---

## 4. MEDIUM: Instance ID Enumeration

### Finding S-5 (MEDIUM): Sequential/ULID Instance IDs Predictable

Instance IDs are ULIDs which are time-sortable and predictable:

```rust
// workflow_start.rs:60
None => Ulid::new().to_string(),
```

An attacker can:
1. Create a workflow to learn the current ULID timestamp
2. Predict subsequent workflow IDs within a time window
3. Enumerate all workflows created within that window

**Recommendation**: Consider using opaque random IDs (not time-ordered) if workflow existence should not be discoverable.

---

## 5. MEDIUM: No Input Validation on workflow_type

### Finding S-6 (MEDIUM): workflow_type Passed Without Validation

```rust
// workflow_start.rs:79
let workflow_type = req.workflow_type.clone();
```

The `workflow_type` string is passed directly to the orchestrator without validation. Malformed workflow_type values could:
- Cause crashes in the worker
- Trigger unexpected behavior in activity handlers

**Recommendation**: Add allowlist validation for known workflow types.

---

## 6. MEDIUM: Signal Name Injection

### Finding S-7 (MEDIUM): User-Controlled signal_name

```rust
// signal.rs:58
OrchestratorMsg::Signal {
    instance_id,
    signal_name: req.signal_name.clone(),  // User controlled
    payload,
    reply: tx,
}
```

The `signal_name` is user-controlled and passed directly. If workflow code uses signal names in security-sensitive paths (e.g., `if signal_name == "admin_override"`), this could enable signal injection attacks.

**Recommendation**: Validate signal_name format with a strict regex or allowlist.

---

## 7. LOW: No Query Parser Injection Prevention in Search

### Finding S-8 (LOW): Search Query Parser Accepts Arbitrary Input

```rust
// query.rs:311
let parsed_query = match QueryParser::new().parse(query_text) {
```

The search query is parsed by a custom parser. Depending on implementation, malformed queries could:
- Cause parser panics (denial of service)
- Potentially leak information via error messages

**Recommendation**: Wrap in panic-catching boundary, sanitize error messages.

---

## 8. LOW: 30-Second HTTP Timeout Too Lenient

### Finding S-9 (LOW): TimeoutLayer(30 seconds)

```rust
// router.rs:115
.layer(TimeoutLayer::new(Duration::from_secs(30)))
```

A 30-second timeout per request is too lenient for cheap operations and could allow slow-client DoS.

**Recommendation**: Per-endpoint timeouts (5s for simple queries, 60s+ for complex replay).

---

## 9. Code Quality: Positive Observations

1. **Type-safe ID parsing**: `InstanceId::parse()` and `NamespaceId::try_new()` provide validation
2. **Structured error responses**: ApiError type provides consistent error format
3. **Actor call timeouts**: 5-second timeout on actor calls prevents indefinite blocking
4. **Proper use of `split_path_id`**: All handlers validate ID format before use
5. **Serde skip_serializing_if**: Optional fields handled properly

---

## 10. Attack Surface Summary

| Endpoint | Risk | Authentication | Authorization |
|----------|------|---------------|---------------|
| `POST /api/v1/workflows` | Start new workflow | NONE | NONE |
| `GET /api/v1/workflows` | List all workflows | NONE | NONE |
| `GET /api/v1/workflows/{id}` | Get workflow status | NONE | NONE |
| `DELETE /api/v1/workflows/{id}` | Terminate workflow | NONE | NONE |
| `GET /api/v1/workflows/{id}/timeline` | View timeline | NONE | NONE |
| `GET /api/v1/workflows/{id}/history` | View history | NONE | NONE |
| `GET /api/v1/workflows/{id}/effect-journal` | View effects | NONE | NONE |
| `POST /api/v1/workflows/{id}/signals` | Send signal | NONE | NONE |
| `POST /api/v1/workflows/{id}/compensate` | Compensate | NONE | NONE |
| `GET /api/v1/search` | Search | NONE | NONE |

---

## 11. Required Mitigations (Priority Order)

1. **P0 (Critical)**: Add authentication middleware (API key, JWT, or mutual TLS)
2. **P0 (Critical)**: Add authorization layer (workflow-level access control)
3. **P1 (High)**: Replace `CorsLayer::permissive()` with explicit allowlist
4. **P1 (High)**: Add rate limiting middleware
5. **P2 (Medium)**: Add workflow_type allowlist validation
6. **P2 (Medium)**: Add signal_name format validation
7. **P3 (Low)**: Consider non-sequential instance IDs for sensitive deployments
8. **P3 (Low)**: Add per-endpoint timeout configuration

---

## 12. Test Evidence

```bash
# All endpoints respond without authentication
curl -X DELETE http://localhost:8080/api/v1/workflows/test/instance123
# Returns 204 NO CONTENT (terminates or not-found, not auth failure)

curl http://localhost:8080/api/v1/workflows  
# Returns full list of workflows with 200 OK
```

---

*Findings compiled: 2026-04-24*
*Next reviewer: Continue batch 8 analysis*