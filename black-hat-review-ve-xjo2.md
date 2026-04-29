# Black-Hat Security Audit Report — ve-xjo2

**Auditor:** polecat nuka  
**Date:** 2026-04-25  
**Scope:** Full adversarial security review of veloxide codebase  
**Focus:** Injection vectors, privilege escalation, input validation gaps, unsafe code patterns, crypto/data security

---

## Executive Summary

4 CRITICAL, 7 HIGH, 11 MEDIUM, 7 LOW findings. The codebase has solid foundational architecture (no SQL injection, no template injection, no command injection via shell) but has critical operational security gaps: permissive CORS, zero authentication, vault access control bypass, and credential rotation that doesn't actually rotate. The HTTP connector enables SSRF via user-controlled URL construction. Sensitive cryptographic material is never zeroed from memory.

---

## CRITICAL Findings

### C1: Permissive CORS Configuration
- **File:** `crates/vo-api/src/router.rs:116`
- **Code:** `.layer(CorsLayer::permissive())`
- **Impact:** All API endpoints accessible from any origin. CSRF attacks can start/terminate workflows, inject signals, read all data from any website.

### C2: Zero Authentication on All API Routes
- **File:** `crates/vo-api/src/router.rs:48-118`
- **Impact:** Every endpoint is fully open — no auth middleware, no API keys, no JWT, no RBAC. `POST /api/v1/workflows`, `DELETE /api/v1/workflows/{id}`, `POST /api/v1/workflows/{id}/signals` all publicly accessible.
- **Note:** `Principal` and `AccessPolicy` types exist in `vo-types/src/credentials.rs:497-535` and `vo-core/src/vault/vault.rs` but are never wired into the API pipeline.

### C3: `.expect()` on User-Supplied `instance_id` Enables DoS
- **File:** `crates/vo-api/src/handlers/workflow_start.rs:63`
- **Code:** `vo_types::InstanceId::parse(&instance_id_str).expect("generated ULID should be valid")`
- **Impact:** When `req.instance_id` is `Some(ref id)`, user input reaches `.expect()`. Invalid instance_id crashes the handler thread. DoS vector.

### C4: Credential Rotation Generates All-Zeros Key Material
- **File:** `crates/vo-core/src/vault/vault.rs:117-119`
- **Code:** `SecretValue::new(vec![0u8; 32], [0u8; 12], ...)`
- **Impact:** `rotate()` creates new `SecretValue` with zero ciphertext and zero nonce. Rotation is effectively a no-op — original secret is lost, replaced with zeros. The all-zero nonce also violates AES-GCM nonce uniqueness requirement.

---

## HIGH Findings

### H1: SSRF via User-Controlled HTTP Connector URL
- **File:** `crates/vo-worker/src/connector/http.rs:56-66`
- **Code:** `let full_url = format!("{}{}", url, path);` where `url` and `path` come from `effect_intent`
- **Impact:** Attacker controlling `effect_intent` JSON can set `base_url` to `http://169.254.169.254/` (AWS metadata) or any internal service. No URL validation or sanitization.

### H2: Vault Access Control Completely Bypassed
- **File:** `crates/vo-core/src/vault/vault.rs:50,147,172`
- **Code:** `_principal` parameter ignored in `get_secret()`, `revoke_version()`, `revoke_all()`
- **Impact:** Any caller can retrieve secrets, revoke credential versions, or revoke all versions without authorization. The `AccessPolicy`/`AccessChecker` infrastructure exists but is never invoked.

### H3: Missing `zeroize` on Sensitive Cryptographic Material
- **Files:** `crates/vo-storage/src/crypto.rs:75-80,145-147`, `crates/vo-types/src/credentials.rs:307-312`, `crates/vo-types/src/encryption.rs:75-76`
- **Impact:** DEK arrays, `SecretValue` ciphertext, `WrappedDek` bytes, and plaintext from `decrypt_blob` are never zeroed after use. Remain on stack/heap until overwritten. The `zeroize` crate is not a dependency.

### H4: No Rate Limiting on API Endpoints
- **File:** `crates/vo-api/src/router.rs` (absent)
- **Impact:** No inbound rate limiting. Vulnerable to brute-force, DoS via request flooding, resource exhaustion.

### H5: No HTTP Security Headers
- **File:** `crates/vo-api/src/router.rs:108-118`
- **Impact:** No `X-Content-Type-Options`, `X-Frame-Options`, `Strict-Transport-Security`, or `Content-Security-Policy`.

### H6: Unbounded `serde_json::Value` Input Without Size Limits
- **File:** `crates/vo-api/src/types/v3.rs:16,51`
- **Code:** `pub input: serde_json::Value` and `pub payload: serde_json::Value` with no depth/size constraints
- **Impact:** Deeply nested JSON bomb can cause CPU exhaustion during deserialization. Default axum body limit (2MB) still allows pathological inputs.

### H7: `unsafe impl Send/Sync` on SchedulerQueue
- **File:** `crates/vo-executor/src/scheduler/queue.rs:208-209`
- **Impact:** Manual `Send`+`Sync` overrides compiler auto-traits. If `!Send`/`!Sync` types are stored, data races occur without compiler protection.

---

## MEDIUM Findings

### M1: `SecretValue` Derives `Debug` — Leaks Ciphertext in Logs
- **File:** `crates/vo-types/src/credentials.rs:307`
- **Impact:** Debug output dumps ciphertext and nonce bytes. Any error message or log containing `SecretValue` leaks crypto material.

### M2: `WrappedDek` Derives `Debug` — Leaks Wrapped Key Bytes
- **File:** `crates/vo-types/src/encryption.rs:75`
- **Impact:** Debug output dumps wrapped key bytes. `Display` correctly redacts but `Debug` still leaks.

### M3: IPC Secrets Passed as Plaintext JSON
- **Files:** `crates/vo-ipc/src/bus.rs:206`, `crates/vo-ipc/src/run.rs:186`
- **Impact:** `Fd3Envelope.secrets` (`BTreeMap<String, String>`) serialized as plaintext JSON over FD3/FD4. Intercepted IPC channel exposes all secrets.

### M4: Subprocess Executable Path Not Validated in vo-executor
- **File:** `crates/vo-executor/src/subprocess.rs:22-44,118`
- **Impact:** `SubprocessConfig.executable_path` is a bare `String` without canonicalization or symlink resolution. Unlike `vo-ipc` which validates paths, this is defense-in-depth gap.

### M5: `DedupKey::parse` Allows Arbitrary Character Content
- **File:** `crates/vo-api/src/types/ingress.rs:13-24`
- **Impact:** Length bounded to 1024 chars but allows null bytes, control characters, Unicode. Potential log injection or storage layer issues.

### M6: Storage Layer Deserialization Without Depth Limits
- **Files:** `crates/vo-storage/src/*.rs` (13 locations)
- **Impact:** All `serde_json::from_slice` calls on storage reads without JSON depth limits. Corrupted storage could cause stack overflow. `serde_json` default recursion limit (128) provides some protection.

### M7: IPC Envelope `input` Field Unbounded
- **File:** `crates/vo-ipc/src/envelope.rs:10-18`
- **Impact:** `Fd3Envelope.input` is `serde_json::Value` with no depth/size constraints. 10MB limit on raw payload but nested JSON can still cause excessive CPU.

### M8: `encrypt_blob` Accepts Unbounded Input
- **File:** `crates/vo-storage/src/crypto.rs:156-182`
- **Impact:** No size limit on input slice. Bypassing the 4096-byte validation layer could trigger massive memory allocations.

### M9: `EncryptedBlob.ciphertext` Has No Size Limit
- **File:** `crates/vo-types/src/encryption.rs:104-108`
- **Impact:** IV and tag are validated but ciphertext field accepts arbitrary sizes. Memory exhaustion from deserialized blobs.

### M10: Linter Bypass — `thread_rng()` Not Detected
- **File:** `crates/vo-linter/tests/blackhat_linter_bypass.rs:68-76`
- **Impact:** `vo-linter` flags `rand::random()` and `Uuid::new_v4()` but misses `rand::thread_rng().gen()`. Non-deterministic random in workflows goes undetected.

### M11: No TLS Configuration; Default Plain HTTP
- **Files:** `crates/vo-cli/src/cli.rs:130,145,165,230`, `crates/vo-worker/src/connector/http.rs:14-19`
- **Impact:** All API communication plaintext. Default `http://localhost:3000`. HTTP connector creates default `reqwest::Client` with no TLS cert validation.

---

## LOW Findings

### L1: Hand-Rolled SPSC Queue with 13 Unsafe Operations
- **File:** `crates/vo-ipc/src/spsc.rs` (lines 9-147)
- **Impact:** Raw pointer arithmetic without bounds checks. `mask()` invariant depends on power-of-2 capacity. Needs Miri/ThreadSanitizer verification.

### L2: Triplicated IPC Pipe/Fork/FD Management
- **Files:** `crates/vo-ipc/src/run.rs`, `crates/vo-ipc/src/bus.rs`, `crates/vo-executor/src/subprocess.rs`
- **Impact:** Three copies of pipe/fork/fd management with `from_raw_fd` + `pre_exec` unsafe surface. Should consolidate into single shared module.

### L3: `.unwrap()` on Option in IPC Drain Path
- **File:** `crates/vo-ipc/src/bus.rs:177,183`
- **Impact:** Double-drain panics. API does not prevent double-call.

### L4: `.expect()` in Default Runtime Implementation
- **File:** `crates/vo-executor/src/runtime.rs:81`
- **Impact:** Tokio runtime creation failure panics. Resource exhaustion DoS.

### L5: `.expect()` on Semaphore Acquire
- **File:** `crates/vo-executor/src/scheduler/mod.rs:79`
- **Impact:** Closed semaphore panics. Currently dead_code but will crash if activated.

### L6: Hardcoded Test KEK `[0x42u8; 32]`
- **File:** `crates/vo-storage/src/key_partition/fjall_dek_store.rs:295-297`
- **Impact:** Test-only, but should be documented to prevent copying to production.

### L7: Error Messages Leak Internal Crypto State
- **Files:** `crates/vo-storage/src/crypto.rs:16-38`, `crates/vo-core/src/vault/error.rs:55-58`
- **Impact:** Error messages reveal DEK existence, key shredding status, master key version numbers.

---

## Positive Findings (Security Strengths)

1. **No command injection via shell** — All `Command::new()` uses fixed binary paths, no `/bin/sh -c` with user input
2. **No SQL injection** — No SQL database; Fjall KV store with programmatic key construction
3. **No template injection** — No template rendering engines
4. **No hardcoded production secrets** — All credential references in test files only
5. **IPC subprocess isolation** — `env_clear()`, `setpgid(0,0)`, `O_CLOEXEC` on pipes
6. **SDK FD3 read is bounded** — 10MB limit, UTF-8 validation, atomic guard against double-read
7. **SPSC queue memory ordering correct** — Acquire/Release semantics for lock-free access
8. **IPC path validation** — `vo-ipc/src/config.rs` canonicalizes and validates executable paths

---

## Recommendations (Priority Order)

1. **Add authentication middleware** — API key, JWT, or mTLS on all routes
2. **Replace `CorsLayer::permissive()`** with explicit allowed origins
3. **Fix `workflow_start.rs:63`** — Replace `.expect()` with proper `map_err()` returning 400
4. **Fix vault `rotate()`** — Generate actual new key material via `crypto::generate_dek()`, re-encrypt secret
5. **Enforce vault access control** — Wire `_principal` into `AccessChecker`
6. **Add URL validation to HttpConnector** — Block internal IPs, enforce HTTPS, validate paths
7. **Add `zeroize` dependency** — Zero DEK arrays, `SecretValue`, `WrappedDek` on drop
8. **Remove `Debug` derives** from `SecretValue` and `WrappedDek` — Implement custom redacting `Debug`
9. **Add rate limiting middleware** — `tower-governor` or similar
10. **Add security headers** — `X-Content-Type-Options`, `X-Frame-Options`, `Strict-Transport-Security`
11. **Add JSON depth limits** — `serde_json::from_str` with `recursion_limit` on all API input
12. **Consolidate IPC pipe management** — Single shared module instead of 3 copies
