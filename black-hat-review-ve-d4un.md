# BLACKHAT SECURITY AUDIT REPORT: ve-d4un
**Auditor:** polecat/mirelurk
**Date:** 2026-04-25
**Focus:** Injection vectors, privilege escalation, input validation gaps, unsafe code patterns

---

## CRITICAL FINDINGS

### 1. SPSC Queue - Blanket Unsafe impl Send/Sync
**File:** `vo-ipc/src/spsc.rs:8-16`
```rust
pub struct SpscQueue<T> {
    buffer: *mut MaybeUninit<T>,  // RAW POINTER
    cap: usize,
    head: AtomicUsize,
    tail: AtomicUsize,
}
unsafe impl<T: Send> Send for SpscQueue<T> {}  // DANGEROUS!
unsafe impl<T: Send> Sync for SpscQueue<T> {}
```
**Severity:** CRITICAL
**Risk:** Any `Send` type (even with interior mutability) can be sent across threads, enabling data races.
**Fix:** Require `T: Send + Sync` or use safe alternative like `Arc<Mutex<Vec<T>>>`.

---

### 2. Credential Vault Authorization Bypass
**File:** `vo-core/src/vault/vault.rs:47-74`
```rust
pub fn get_secret(&self, id: &CredentialId, _principal: &Principal) -> Result<SecretValue> {
    // _principal is IGNORED - no authorization check!
    Ok(active.secret_value.clone())
}
```
**Severity:** CRITICAL
**Risk:** Any caller can retrieve any secret if they know the CredentialId.
**Fix:** Implement actual authorization check using AccessChecker.

---

### 3. ExecProbe Arbitrary Command Execution
**File:** `vo-actor/src/probe.rs:469-539`
```rust
let mut cmd = Command::new(&self.command);
cmd.args(&self.args);  // No validation!
```
**Severity:** CRITICAL (depends on config exposure)
**Risk:** If probe config is user-controlled, arbitrary command execution is possible.
**Fix:** Whitelist allowed executables only.

---

## HIGH FINDINGS

### 4. Unsafe FD Handling in SDK
**File:** `vo-sdk/src/io.rs:47-51`
```rust
if !is_fd_valid(4) { return Err(...); }
let mut fd4 = unsafe { std::fs::File::from_raw_fd(4) };  // Ownership taken after check
```
**Severity:** HIGH
**Risk:** TOCTOU race between check and use; privilege escalation via FD reuse.
**Fix:** Restructure to validate before taking ownership, use OwnedFd.

---

### 5. Unvalidated URL Construction - SSRF Risk
**File:** `vo-worker/src/connector/http.rs:56-66`
```rust
let path = request_data["path"].as_str().unwrap_or("/");
let full_url = format!("{}{}", url, path);  // No validation!
```
**Severity:** HIGH
**Risk:** Path traversal (`/../admin/delete`), SSRF to internal services.
**Fix:** Validate path doesn't contain `..` or URL schemes.

---

### 6. Signal to Process Group TOCTOU Race
**File:** `vo-ipc/src/run.rs:239-246`
```rust
unsafe { libc::kill(-kill_pgid, libc::SIGKILL); }  // Entire process group
```
**Severity:** HIGH
**Risk:** PID reuse between acquire and kill could kill wrong process group.
**Fix:** Verify process still exists before sending signal.

---

### 7. IPC Identity Missing Cryptographic Verification
**File:** `vo-ipc/src/envelope.rs:200-218`
```rust
if envelope.instance_id != expected_instance_id { return Err(...); }
```
**Severity:** HIGH
**Risk:** Forged envelopes with valid IDs bypass authentication.
**Fix:** Add HMAC or similar cryptographic identity verification.

---

## MEDIUM FINDINGS

### 8. Weak Path Traversal Defense
**File:** `vo-storage/src/mmap_cache.rs:296`
```rust
let safe_name = key.replace(['/', '\\', ':'], "_");  // Doesn't block ".."
```
**Severity:** MEDIUM
**Fix:** Canonicalize path and verify it stays within base_path.

### 9. Silent Error Suppression (.ok() chains)
**File:** `vo-sdk/src/read.rs:34-47`
```rust
std::str::from_utf8(buf).ok()
    .and_then(|s| serde_json::from_str::<...>(s).ok())
```
**Severity:** MEDIUM
**Fix:** Use proper error handling that preserves diagnostic info.

### 10. UTF-8 Lossy Conversion
**File:** `vo-ipc/src/config.rs:99-104`
```rust
String::from_utf8_lossy(payload)  // Silently replaces invalid bytes!
```
**Severity:** MEDIUM
**Fix:** Use strict validation, reject invalid UTF-8.

### 11. Pre-Exec Race Condition
**File:** `vo-executor/src/subprocess.rs:125-147`
**Severity:** MEDIUM - Race window between fork and exec.

### 12. Lock Token Verification Potentially Bypassed
**File:** `vo-worker/src/retry.rs`
**Severity:** MEDIUM - hold_token may not be consistently verified.

---

## SUMMARY

| Severity | Count | Top Issues |
|----------|-------|------------|
| CRITICAL | 3 | SPSC unsafe impl, Vault bypass, ExecProbe injection |
| HIGH | 4 | FD handling, SSRF, TOCTOU signal, IPC auth |
| MEDIUM | 5 | Path traversal, error suppression, UTF-8 lossy |

**Immediate Actions Required:**
1. Fix Credential Vault authorization bypass (CRITICAL)
2. Fix SPSC queue blanket impl (CRITICAL)
3. Add command validation to ExecProbe (CRITICAL)
4. Validate URL construction in HTTP connector (HIGH)