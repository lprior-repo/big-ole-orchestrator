# Security Audit Findings: se-3i5 (BLACKHAT wave3-13)

## Issue Details
- **ID**: se-3i5
- **Title**: BLACKHAT: security audit wave3-13
- **Type**: Security Audit (Adversarial Testing)
- **Assignee**: seshat/polecats/dust
- **Status**: in_progress

## Executive Summary
The veloxide API has **critical security vulnerabilities** primarily centered around **missing authentication and authorization** on all endpoints. The API is a workflow orchestration engine with event sourcing, and exposes sensitive operations without access control.

## Critical Findings

### 1. No Authentication on Any API Endpoint (CRITICAL)
**Severity**: Critical
**Location**: All handlers in `vo-api/src/handlers/`

All API endpoints are publicly accessible with no authentication mechanism:
- `POST /api/v1/workflows` - Start workflows (anyone can create workflows)
- `GET /api/v1/workflows/:id` - Get workflow status
- `DELETE /api/v1/workflows/:id` - Terminate workflows
- `GET /api/v1/workflows` - List all active workflows
- `GET /api/v1/workflows/:id/timeline` - Get workflow timeline
- `GET /api/v1/workflows/:id/history` - Get workflow history
- `GET /api/v1/watch/:instance_id` - SSE subscription for live events

**Impact**: Anyone on the network can:
- Start arbitrary workflows
- Query status of all workflows
- Terminate running workflows
- Subscribe to SSE streams for any workflow
- Access complete execution history including inputs/outputs

### 2. No Rate Limiting (HIGH)
**Severity**: High
**Location**: API server configuration

The API has no rate limiting, making it vulnerable to:
- DoS attacks
- Resource exhaustion via workflow creation
- Brute force enumeration of workflow IDs

### 3. Information Disclosure via Detailed Error Messages (MEDIUM)
**Severity**: Medium
**Location**: `vo-api/src/handlers/workflow.rs`

Error messages reveal internal system details:
```rust
"instance {namespace}/{instance_id_str} not found"
"engine at capacity: {running}/{max} instances running"
```

**Impact**: Attackers can enumerate system state and capacity.

### 4. No Input Sanitization on Namespace/Instance IDs (MEDIUM)
**Severity**: Medium
**Location**: `split_path_id()` in helpers.rs

The `split_path_id` function parses user input but doesn't sanitize:
```rust
pub fn split_path_id(id: &str) -> Option<(NamespaceId, InstanceId)> {
    let parts: Vec<&str> = id.split('/').collect();
    // No validation of special characters or length limits
}
```

**Impact**: Potential for injection attacks if downstream handlers don't validate.

### 5. SSE Broadcast Channel Subscription (MEDIUM)
**Severity**: Medium
**Location**: `vo-api/src/handlers/sse.rs`

The SSE endpoint allows subscribing to any workflow's live events without authentication:
```rust
pub async fn watch_workflow(...) {
    let receiver = state.broadcaster.subscribe();
    // No check if user is authorized to view this workflow
}
```

**Impact**: Unauthorized real-time monitoring of workflow execution.

### 6. No Authorization Checks (CRITICAL)
**Severity**: Critical
**Location**: All handlers

Even if authentication were added, there's no authorization layer:
- No namespace-level permissions
- No workflow-level access control
- No role-based access control (RBAC)

## Attack Surface Analysis

### Public Endpoints
| Endpoint | Method | Risk |
|----------|--------|------|
| `/api/v1/workflows` | POST | Workflow creation abuse |
| `/api/v1/workflows` | GET | Enumerate all workflows |
| `/api/v1/workflows/:id` | GET | Query workflow details |
| `/api/v1/workflows/:id` | DELETE | Terminate workflows |
| `/api/v1/workflows/:id/timeline` | GET | Expose execution history |
| `/api/v1/workflows/:id/history` | GET | Expose step details |
| `/api/v1/watch/:id` | GET | Real-time surveillance |
| `/api/v1/search` | GET | Query workflow data |

### Data Exposure
- Complete workflow execution history
- Step inputs and outputs
- Error messages and stack traces
- System capacity and performance metrics

## Recommendations

### Immediate (P0)
1. **Add API authentication** - JWT, API keys, or OAuth2
2. **Add rate limiting** - Per-IP and per-user limits
3. **Add authorization layer** - Namespace and workflow permissions

### Short-term (P1)
1. Sanitize all user inputs in `split_path_id`
2. Reduce error message verbosity in production
3. Add audit logging for all state-changing operations
4. Implement workflow-level access control

### Long-term (P2)
1. Add RBAC with roles: admin, operator, viewer
2. Add IP allowlisting for sensitive endpoints
3. Add anomaly detection for unusual API usage
4. Security audit of storage layer (`vo-storage`)

## Testing Performed
- Code review of all handler files
- Analysis of authentication/authorization patterns
- Review of error handling and information disclosure
- SSE subscription mechanism analysis

## Conclusion
The veloxide API is **not production-ready from a security perspective**. The absence of authentication and authorization is a critical vulnerability that would allow any user to perform any operation on any workflow. This must be addressed before deploying to any environment with untrusted users.

---
*Audit performed by: seshat/polecats/dust*
*Date: 2026-04-24*
*Wave: BLACKHAT 3-13*
