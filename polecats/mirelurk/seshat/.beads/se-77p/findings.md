# BLACKHAT REVIEW: Veloxide Security Audit - Batch 2

**Bead**: se-77p
**Date**: 2026-04-24
**Reviewer**: Polecat mirelurk (adversarial black-hat)
**Scope**: vo-ipc, vo-actor, vo-api subsystems

---

## Verdict: CRITICAL FINDINGS - SECURITY REVIEW FAILED

**APPROVED WITH CRITICAL FINDINGS REQUIRING IMMEDIATE REMEDIATION**

---

## 1. vo-ipc (Inter-Process Communication) Security Analysis

### 1.1 Pipe Security ✓ PASS

- Uses `O_CLOEXEC` on all pipes (`libc::pipe2` with `O_CLOEXEC`)
- Child process inherits only fd3 (stdin) and fd4 (stdout) - correct
- Parent closes child's ends of pipes properly

### 1.2 Subprocess Execution Security

**FINDING F-5 (LOW): TOCTOU Race on Executable Path Validation**

- `config.rs:71-96` validates program path exists, is file, and is executable
- `canonicalize()` at line 32 resolves symlinks
- **Race condition**: Between validation and actual execution, symlink could be swapped
- **Impact**: If attacker has filesystem access concurrent with validation, could point to malicious binary

**Mitigation**: Use `O_NOFOLLOW` when opening the file, or use file descriptor-based execution

### 1.3 IPC Protocol Security

**FINDING F-6 (INFO): No Encryption on IPC Channel**

- FD3/FD4 pipes are local-only (using `O_CLOEXEC`) but not encrypted
- Any process with local access could read/write to pipes
- **Impact**: Low for most deployments (requires local access)
- **Note**: Not practical to encrypt local IPC; this is standard for Unix

**FINDING F-7 (LOW): No Authentication on IPC Protocol**

- The envelope protocol (`envelope.rs`) has no authentication marker
- Any process that can write to FD3 can send commands
- **Impact**: If an attacker can compromise the engine process, they could impersonate workflows

### 1.4 Payload Size Limits ✓ PASS

- `MAX_PAYLOAD_SIZE = 10_485_760` (10MB) limits DoS
- Both FD3 write and FD4 read enforce this limit

### 1.5 Identity Validation ✓ PASS

- `engine_receive_envelope()` validates `instance_id` and `node_id` match expected values
- Version checking rejects anything != 1
- ID fields validated for alphanumeric characters only

---

## 2. vo-actor (Actor System) Security Analysis

### 2.1 CRITICAL FINDING: No Authorization Model

**FINDING F-8 (CRITICAL): Unauthenticated Actor Messaging**

The `OrchestratorMsg` enum allows any caller to:
- `StartWorkflow` - Start any workflow type
- `GetStatus` - Read status of ANY workflow instance
- `Terminate` - Kill ANY workflow instance
- `ListActive` - List ALL active workflows
- `Compensate` - Trigger compensation on ANY workflow
- `Signal` - Send arbitrary signals to ANY workflow

**Code Location**: `vo-actor/src/lib.rs:62-100`

**Impact**: **CRITICAL** - Anyone with access to the actor system (local or network) can:
1. Enumerate all running workflows
2. Read sensitive workflow state
3. Terminate critical workflows
4. Inject arbitrary signals into workflows

**Recommended Fix**: Implement capability-based authorization:
- Add `Principal` trait with `can_start_workflow()`, `can_read_instance()`, `can_terminate()`, etc.
- Validate caller permissions before processing messages
- Consider using `NamespaceId` for multi-tenant isolation

### 2.2 Message Routing Security

- Actor refs are passed via `Extension<ActorRef<OrchestratorMsg>>` in axum handlers
- No validation that caller is authorized to send to orchestrator
- Same issue as F-8

### 2.3 Ractor Library Trust

- Uses `ractor` crate for actor model
- No custom security controls on ractor internals
- Trust boundary is at the orchestrator message handler

---

## 3. vo-api (HTTP API) Security Analysis

### 3.1 CRITICAL FINDING: No Authentication on HTTP Endpoints

**FINDING F-9 (CRITICAL): Entire API Has No Authentication**

All endpoints in `router.rs:48-118` have NO authentication:
- `POST /api/v1/workflows` - Start workflow
- `GET /api/v1/workflows/{id}` - Get workflow status
- `DELETE /api/v1/workflows/{id}` - Terminate workflow
- `POST /api/v1/workflows/{id}/signals` - Send signal
- `GET /api/v1/search` - Full-text search
- SSE and WebSocket endpoints - Real-time workflow events

**Impact**: **CRITICAL** - Anyone with network access can:
1. Start workflows (potentially resource exhaustion)
2. Enumerate ALL workflow instances and their state
3. Read sensitive workflow data and history
4. Terminate workflows
5. Inject arbitrary signals
6. Subscribe to real-time SSE/WS streams for any workflow

### 3.2 FINDING F-10 (HIGH): Permissive CORS Configuration

**Code Location**: `vo-api/src/router.rs:116`
```rust
.layer(CorsLayer::permissive())
```

**Impact**: Allows cross-origin requests from ANY domain
- Enables CSRF attacks
- Allows any website to make API calls on behalf of users

**Recommended Fix**: Configure CORS with explicit allowed origins

### 3.3 FINDING F-11 (MEDIUM): No Rate Limiting on API

- No rate limiting middleware visible in router
- Could allow DoS via request flooding
- `TimeoutLayer` (30s) helps but doesn't prevent connection exhaustion

### 3.4 Positive Security Controls ✓

- `TimeoutLayer` (30s) prevents slow-client attacks
- `TraceLayer` for observability
- Input validation present in handlers
- ULID generation for instance IDs (non-guessable)

---

## 4. vo-worker (HTTP Connector) Security Analysis

### 4.1 FINDING F-12 (MEDIUM): Unrestricted HTTP Outbound Connector

**Code Location**: `vo-worker/src/connector/http.rs:55-100`

The HTTP connector builds URLs from workflow effect intent:
```rust
let full_url = format!("{}{}", url, path);
```

**Impact**: A malicious workflow definition could:
1. Make HTTP requests to internal services (SSRF)
2. Exhaust worker resources via large requests
3. Bypass network segmentation

**No authentication mechanism** is included in outbound connector:
- No API key injection
- No mTLS configuration visible
- No outbound request validation

---

## 5. Additional Findings from Batch 1 (vo-storage crypto)

Findings from batch 1 (`black-hat-review-ve-1aygp.md`) remain open:

| Finding | Severity | Status |
|---------|----------|--------|
| F-1: No zeroize on DEK/KEK | MEDIUM | Open |
| F-2: No AAD in encryption | MEDIUM | Open |
| F-3: Non-crypto hash for redaction | LOW | Open |
| F-4: Outdated aes-gcm dependency | LOW | Open |

---

## 6. Summary of Findings

### Critical (Must Fix Before Production)
| ID | Finding | Subsystem |
|----|---------|-----------|
| F-8 | No authorization model for actor messages | vo-actor |
| F-9 | No authentication on HTTP API | vo-api |

### High (Should Fix)
| ID | Finding | Subsystem |
|----|---------|-----------|
| F-10 | Permissive CORS | vo-api |
| F-12 | Unrestricted HTTP outbound connector | vo-worker |

### Medium (Defense in Depth)
| ID | Finding | Subsystem |
|----|---------|-----------|
| F-5 | TOCTOU race on executable validation | vo-ipc |
| F-11 | No API rate limiting | vo-api |

### Low / Info
| ID | Finding | Subsystem |
|----|---------|-----------|
| F-6 | No encryption on local IPC | vo-ipc |
| F-7 | No authentication on IPC protocol | vo-ipc |

---

## 7. Security Posture Summary

### Strengths
1. Proper pipe setup with `O_CLOEXEC`
2. Payload size limits prevent memory exhaustion
3. Identity validation on IPC envelopes
4. Timeout protection on HTTP (30s)
5. ULID for non-guessable instance IDs
6. Proper error handling without information leakage

### Critical Gaps
1. **ZERO authentication** on HTTP API - fully open
2. **ZERO authorization** on actor messaging - any caller can do anything
3. No CORS protection
4. No rate limiting
5. Outbound HTTP connector is unrestricted

### Recommendations (Priority Order)
1. **IMMEDIATE**: Add authentication to HTTP API (Bearer token, API key, or OAuth2)
2. **IMMEDIATE**: Add authorization checks to orchestrator message handler
3. **HIGH**: Restrict CORS to specific origins
4. **HIGH**: Add rate limiting middleware
5. **MEDIUM**: Add input validation/scrubbing for HTTP connector URLs
6. **LOW**: Address crypto findings from batch 1

---

## 8. Test Execution Evidence

No code changes made - this is an audit-only task. All findings based on code review.

```
Files Reviewed:
- vo-ipc/src/pipe.rs (15 lines)
- vo-ipc/src/run.rs (255 lines)
- vo-ipc/src/config.rs (205 lines)
- vo-ipc/src/envelope.rs (218 lines)
- vo-ipc/src/error.rs (63 lines)
- vo-actor/src/lib.rs (1914 lines)
- vo-api/src/lib.rs (185 lines)
- vo-api/src/router.rs (130 lines)
- vo-api/src/handlers/workflow_start.rs (172 lines)
- vo-worker/src/lib.rs (765 lines)
- vo-worker/src/connector/http.rs (348 lines)
```

---

## 9. Verdict

**NOT APPROVED FOR PRODUCTION** without addressing F-8 and F-9.

The system has no authentication or authorization - it is completely open. This is acceptable for a development environment behind a firewall, but NOT for production.

The architecture is sound and the security controls that ARE implemented (pipe setup, payload limits, timeouts) are correct. But the absence of authentication on the primary API (HTTP) and authorization on the message bus (actor) are fundamental gaps that must be filled before production deployment.
