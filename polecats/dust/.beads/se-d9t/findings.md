# SECURITY REPORT: Veloxide Codebase Adversarial Analysis

**Batch 8 - Security Testing**
**Date**: 2026-04-24
**Reviewer**: Polecat Dust (Black-Hat Security Reviewer)
**Target**: `/home/lewis/src/veloxide/`

---

## EXECUTIVE SUMMARY

The veloxide codebase demonstrates good security engineering practices in several areas: strong input validation via type-safe newtypes, proper use of AES-256-GCM for encryption, and comprehensive injection attack prevention tests. However, several attack surfaces remain that require attention before production deployment.

---

## FINDINGS SUMMARY

| Severity | Count |
|----------|-------|
| Critical | 2 |
| High | 4 |
| Medium | 6 |
| Low | 4 |

---

## CRITICAL FINDINGS

### C-1: Permissive CORS Policy (CRITICAL)

**Location**: `crates/vo-api/src/router.rs:116`

```rust
.layer(CorsLayer::permissive())
```

**Analysis**: The CORS layer allows requests from ANY origin. This enables:
- Cross-site request forgery (CSRF) attacks
- Data exfiltration via malicious websites
- Unauthorized API access from browser-based attacks

**Attack Vector**: Any malicious website can make authenticated requests to the veloxide API if cookies or session tokens are used.

**Recommended Fix**:
```rust
.layer(CorsLayer::new()
    .allow_origin([/* specific allowed origins */])
    .allow_methods(...)
    .allow_headers(...))
```

---

### C-2: No Authentication/Authorization on HTTP Endpoints (CRITICAL)

**Location**: `crates/vo-api/src/router.rs:48-118`

**Analysis**: All API endpoints (`/api/v1/workflows/*`, `/api/v1/watch/*`, `/api/v1/ws/*`) lack authentication middleware. The `AccessChecker` exists in `vo-core/src/vault/access.rs` but is never applied to HTTP routes.

**Attack Vector**:
- Unauthenticated workflow creation, termination, and state manipulation
- Subscription to any workflow SSE/WebSocket stream by knowing the instance ID
- Information disclosure of workflow state, timeline, and events

**Recommended Fix**: Implement authentication middleware that:
1. Validates bearer tokens or API keys
2. Extracts principal from credentials
3. Applies `AccessChecker` per endpoint using the workflow's `AccessPolicy`

---

## HIGH FINDINGS

### H-1: WebSocket Accepts Unsolicited Client Messages (HIGH)

**Location**: `crates/vo-api/src/handlers/ws.rs:223-239`

```rust
Some(Ok(axum::extract::ws::Message::Text(text))) => {
    tracing::debug!(msg = %text, "Received WebSocket message");
}
```

**Analysis**: The WebSocket handler receives client messages and logs them but performs no validation or processing. This could lead to:
- Log injection attacks (malicious content in logs)
- Future SSRF if message handling is extended without proper safeguards

**Recommended Fix**: Validate all incoming WebSocket messages against a schema. Reject unknown message types.

---

### H-2: Unsafe Memory-Mapped File Cache (HIGH)

**Location**: `crates/vo-storage/src/mmap_cache.rs:173`

```rust
let mmap = unsafe { Mmap::map(&file) }.map_err(MmapCacheError::MmapError)?;
```

**Analysis**: The mmap cache creates files based on user-controlled keys with only basic path sanitization:

```rust
fn region_file_path(&self, key: &str) -> PathBuf {
    let safe_name = key.replace(['/', '\\', ':'], "_");
    self.base_path.join(safe_name)
}
```

**Attack Vector**:
- If `base_path` is attacker-controlled or symlinks exist, could read/write arbitrary files
- Time-of-check-time-of-use race condition possible

**Recommended Fix**:
1. Use content-addressable hashing instead of user keys for file names
2. Verify the file is within expected directory using `realpath`
3. Add mandatory access control on cache directory

---

### H-3: Subprocess Execution with Unsafe Raw FD Manipulation (HIGH)

**Location**:
- `crates/vo-executor/src/subprocess.rs:129-160`
- `crates/vo-ipc/src/bus.rs:81-99`

```rust
unsafe {
    command.pre_exec(move || {
        if libc::dup2(fd3_read, 3) == -1 { ... }
        if libc::dup2(fd4_write, 4) == -1 { ... }
    });
}
```

**Analysis**:
- Raw file descriptor manipulation with `unsafe` blocks
- No validation that FD 3 and 4 are not already in use
- If subprocess crashes before FD setup, could inherit unexpected file descriptors

**Recommended Fix**:
1. Use `tokio::process::Command::stdin(Stdio::piped())` with proper async I/O instead of raw FD manipulation
2. Validate FD numbers are within expected range before `dup2`
3. Add `PR_SET_NO_NEW_PRIVS` via `prctl`

---

### H-4: SPSC Queue Lock-Free Data Structure (HIGH)

**Location**: `crates/vo-ipc/src/spsc.rs:68, 85-86`

```rust
let slot = unsafe { &mut *self.buffer.add(self.mask(head)) };
let msg = unsafe { slot.assume_init_read() };
```

**Analysis**:
- Raw pointer manipulation without memory tagging
- `Send + Sync` impls are `unsafe` and rely on correct usage
- If T is not `Send`, undefined behavior

```rust
unsafe impl<T: Send> Send for SpscQueue<T> {}
unsafe impl<T: Send> Sync for SpscQueue<T> {}
```

**Recommended Fix**:
1. Add sanitizers (MSan, Miri) to CI
2. Consider using `crossbeam` or `portable-atomic` instead
3. Document invariants that make the `unsafe` impls sound

---

## MEDIUM FINDINGS

### M-1: No API Rate Limiting on HTTP Endpoints (MEDIUM)

**Location**: `crates/vo-api/src/router.rs`

**Analysis**: Only internal recovery queue has rate limiting (`RecoveryThrottle`). The HTTP API layer lacks:
- Per-IP rate limiting
- Per-user rate limiting
- Request body size limits (beyond 30s timeout)

**Attack Vector**: Denial of service via flooding endpoints.

**Recommended Fix**: Implement tower rate limiting middleware:
```rust
AddExtensionLayer::new(RateLimitLayer::...)
```

---

### M-2: Outdated Cryptographic Dependencies (MEDIUM)

**Location**: `Cargo.toml:118-119`

```
aes-gcm = "0.9"  # Should be 0.10+
aes = "0.7"      # Should be 0.8+
```

**Analysis**:
- `aes-gcm` is 2 versions behind
- No published CVEs but missing hardening improvements
- First party crypto code (not Rust crates) may have similar lag

**Recommended Fix**: Upgrade to latest versions. Monitor for CVE announcements.

---

### M-3: DEK/KEK Not Zeroized After Use (MEDIUM)

**Location**: `vo-storage/src/crypto.rs` (from black-hat review ve-1aygp)

```rust
let mut dek = [0u8; DEK_SIZE_BYTES];
dek.copy_from_slice(&plaintext);
Ok(dek) // DEK bytes persist on caller's stack
```

**Analysis**: Plaintext key material remains on stack after function returns. In core dump or memory inspection scenarios, keys could be recovered.

**Recommended Fix**: Use `zeroize` crate:
```rust
use zeroize::Zeroize;
struct Dek([u8; 32]);
impl Zeroize for Dek { fn zeroize(&mut self) { ... } }
```

---

### M-4: No AAD in AES-GCM Encryption (MEDIUM)

**Location**: `vo-storage/src/crypto.rs` (from black-hat review ve-1aygp)

```rust
let ciphertext = cipher.encrypt(&nonce, data)  // No AAD
```

**Analysis**: Ciphertext is not bound to context (instance ID, DEK ID, content hash). An attacker with write access to storage could transplant ciphertext between instances.

**Recommended Fix**: Add AAD:
```rust
cipher.encrypt(nonce, data, associated_data)
```

---

### M-5: Information Disclosure in Error Messages (MEDIUM)

**Location**: Multiple handlers return detailed errors

**Analysis**: Error responses leak internal details:
```rust
Json(ApiError::new("invalid_namespace", format!("namespace contains illegal characters: {:?}", req.namespace)))
```

**Attack Vector**: Reveals internal type structures, namespace formats, and system internals to attackers.

**Recommended Fix**: Return generic errors to clients, log details server-side:
```rust
// External: "Invalid namespace format"
// Internal: trace!("invalid namespace: {:?}", req.namespace)
```

---

### M-6: DefaultHasher for Redaction Hashing (MEDIUM)

**Location**: `vo-types/src/dual_representation.rs` (from black-hat review ve-1aygp)

**Analysis**: `RedactionKind::Hash` uses 64-bit non-cryptographic hash. Target brute-force feasible for structured data (SSNs, phone numbers).

**Recommended Fix**: Use SHA-256 or BLAKE3 (already in dependency tree).

---

## LOW FINDINGS

### L-1: 30-Second HTTP Timeout Can Be Exploited for DoS (LOW)

**Location**: `crates/vo-api/src/router.rs:115`

```rust
.layer(TimeoutLayer::new(Duration::from_secs(30)))
```

**Analysis**: Long timeout allows slow-read attacks on connections. Combined with no rate limiting, an attacker can hold connections open indefinitely.

**Recommended Fix**: Reduce timeout, implement connection limiting.

---

### L-2: Broadcast Channel Capacity Limits Can Cause Silent Event Loss (LOW)

**Location**:
- `crates/vo-api/src/handlers/sse.rs:20` - `SSE_BROADCAST_CAPACITY: usize = 1000`
- `crates/vo-api/src/handlers/ws.rs:12` - `WS_BROADCAST_CAPACITY: usize = 1000`

**Analysis**: If client cannot keep up, events are silently dropped. No indication to client that events were lost (beyond `Lagged` error on WS).

**Recommended Fix**:
1. Document this behavior clearly
2. Consider adding sequence numbers so clients can detect gaps
3. Provide a "resync" endpoint to retrieve missed events

---

### L-3: IPC Pipe Creation Race Condition (LOW)

**Location**: `crates/vo-executor/src/subprocess.rs:119-120`

```rust
let (fd3_read, fd3_write) = create_pipe()?;
let (fd4_read, fd4_write) = create_pipe()?;
```

**Analysis**: Between pipe creation and `pre_exec`, signal handlers could fire causing unpredictable behavior.

**Recommended Fix**: Block signals during this critical section.

---

### L-4: Missing Security Headers (LOW)

**Location**: HTTP responses from `vo-api`

**Analysis**: No security headers set:
- `X-Content-Type-Options: nosniff`
- `X-Frame-Options: DENY`
- `Content-Security-Policy`
- `Strict-Transport-Security`

**Recommended Fix**: Add `tower-http` security headers middleware.

---

## POSITIVE SECURITY OBSERVATIONS

1. **Strong Input Validation**: Type-safe `WorkflowName`, `SignalName`, `InstanceId` newtypes with regex enforcement prevent injection attacks
2. **Comprehensive Injection Tests**: `security_input_validation_tests.rs` covers SQL injection, XSS, template injection, null bytes
3. **Credential Vault Design**: RBAC with proper `AccessPolicy` type system
4. **AES-256-GCM**: Correct algorithm choice with CSPRNG IVs
5. **KEK/DEK Hierarchy**: Proper key separation
6. **env_clear()**: Subprocess clears environment (good)
7. **FD_CLOEXEC**: Pipes properly marked for close-on-exec

---

## RECOMMENDED MITIGATIONS (PRIORITY ORDER)

1. **P0**: Add authentication middleware to all HTTP endpoints
2. **P0**: Replace `CorsLayer::permissive()` with restrictive CORS
3. **P1**: Add API rate limiting
4. **P1**: Implement DEK/KEK zeroization
5. **P1**: Add AAD to encryption operations
6. **P2**: Upgrade `aes-gcm` to 0.10+
7. **P2**: Sanitize error messages for external responses
8. **P2**: Add security headers
9. **P3**: Replace `DefaultHasher` with BLAKE3 for redaction

---

## TEST COVERAGE ASSESSMENT

| Area | Coverage |
|------|----------|
| SQL/NoSQL Injection | Good (dedicated tests) |
| XSS | Good (input validation) |
| Path Traversal | Partial (key sanitization exists but could be stronger) |
| Authentication | None on HTTP endpoints |
| Authorization | Exists in types but not enforced on API |
| Rate Limiting | No HTTP API rate limiting |
| Cryptographic Operations | Good (38 tests pass) |
| Memory Safety | Extensive `unsafe` blocks need sanitizers |

---

## CONCLUSION

**Verdict**: **CONDITIONAL APPROVAL**

The veloxide codebase demonstrates solid security engineering fundamentals with strong input validation, proper cryptography, and comprehensive injection attack testing. However, the absence of authentication on HTTP endpoints and permissive CORS configuration constitute critical vulnerabilities that must be addressed before production deployment.

The codebase is suitable for staging/development but **NOT production-ready** until:
1. Authentication is implemented
2. CORS is restricted
3. Rate limiting is added
4. Key zeroization is implemented