# Security Audit Findings: seshat (BLACKHAT wave3-5)

**Auditor**: polecat/ghoul
**Date**: 2026-04-24
**Scope**: seshat project - Dioxus WASM diagram tool + CLI
**Lines of Code Examined**: ~600+ Rust files across seshat_cli, diagram_tool, diagram_models, canvas_domain, canvas_math

---

## EXECUTIVE SUMMARY

The seshat codebase demonstrates **strong security posture** with multiple defensive layers. Most modules use `#![forbid(unsafe_code)]`, strict clippy linting, and proper error handling. No critical or high-severity vulnerabilities were found.

**VERDICT: PASS WITH MINOR OBSERVATIONS**

---

## SECURITY STRENGTHS

### 1. Memory Safety
- **Most modules use `#![forbid(unsafe_code)]`** - Prevents memory safety vulnerabilities
- All diagram_tool, diagram_models, canvas_domain modules enforce this

### 2. Panic Prevention
- **Server code (`server/ai_documents.rs`) uses strict linting**:
  ```rust
  #![cfg_attr(not(test), deny(clippy::unwrap_used))]
  #![cfg_attr(not(test), deny(clippy::expect_used))]
  #![cfg_attr(not(test), deny(clippy::panic))]
  ```
- This prevents unexpected panics in production

### 3. SQL Injection Prevention
- **All database operations use SQLx with parameterized queries**:
  ```rust
  sqlx::query("INSERT INTO ai_documents (...) VALUES (?1, ?2, ?3, ?4, ?5, ?6)")
      .bind(doc.id())
      .bind(doc.key())
      // ...
  ```
- **LOCATION**: `diagram_tool/src/store_async/ai_documents.rs:85-91`

### 4. Input Size Limits
- **MAX_INPUT_BYTES = 64 MiB** limit in loader prevents DoS via infinite streams
- **LOCATION**: `seshat_cli/src/show/loader.rs:55`

### 5. Two-Pass JSON Parsing
- Validates raw JSON syntax before attempting typed deserialization
- **LOCATION**: `seshat_cli/src/show/loader.rs:92-105`

### 6. Safe Regex Usage
- Uses `regex-lite = "0.1"` instead of full regex (ReDoS-resistant)

### 7. Robust CLI Parsing
- Uses `clap = "4.4"` - well-maintained, secure CLI parsing

---

## MINOR OBSERVATIONS (LOW SEVERITY)

### Observation 1: JSON Error Message Injection (Low Risk)

**FILE**: `diagram_tool/src/server/ai_documents.rs:186`
```rust
AsyncStoreError::ValidationFailed(msg) => {
    format!(r#"{{"error": "{}"}}"#, msg)
}
```

**ISSUE**: If `msg` contains double quotes or braces, it could potentially break JSON structure.

**RISK**: LOW - CLI tool output, not HTML. Only exploitable if JSON is later rendered in web context without escaping.

**RECOMMENDATION**: Escape control characters in error messages or use `serde_json::json!` macro.

---

### Observation 2: CLI File Overwrite (Informational)

**FILE**: `seshat_cli/src/render.rs:66`
```rust
std::fs::write(&cmd.output, svg).map_err(|e| RenderError::IoError(e.to_string()))?;
```

**ISSUE**: The `render` command can overwrite any file user has write access to.

**RISK**: NONE - This is expected CLI behavior. Users deliberately specify output paths.

---

### Observation 3: Test Code Uses Unwraps

**FILES**: Various `*_tests.rs` files

**ISSUE**: Test modules use `.unwrap()` freely due to `#[allow(clippy::unwrap_used)]`

**RISK**: NONE - Test-only code, not compiled into production binaries.

---

## ATTACK SURFACE ANALYSIS

### Network-Facing (if server feature enabled)
- **axum = "0.8.8"` web framework used
- Server functions only compiled for `#[cfg(not(target_arch = "wasm32"))]`
- JSON-over-RPC interface for AI document operations
- SQLx prevents SQL injection

### CLI Attack Surface
- File I/O operations in seshat_cli
- Path traversal: **NOT VULNERABLE** - clap handles path arguments safely
- JSON parsing: **SAFE** - serde_json with size limits

### WASM Attack Surface
- Dioxus 0.7.4 with web/desktop features
- WASM code cannot access filesystem directly
- canvas_domain and canvas_math are pure computation

---

## RECOMMENDATIONS

1. **Consider JSON escaping in error messages** (Observation 1)
2. **Add rate limiting** if server becomes public-facing
3. **Continue enforcing `#![forbid(unsafe_code)]`** in new modules
4. **Keep using regex-lite** for any future regex needs

---

## CONCLUSION

The seshat codebase is **well-secured** with multiple defense layers:
- Memory safety via `forbid(unsafe_code)`
- Panic prevention via strict clippy linting
- SQL injection prevention via SQLx parameterized queries
- DoS prevention via input size limits
- Safe libraries (clap, regex-lite, serde_json)

**No critical or high-severity security issues found.**

---

*This audit focused on adversarial security testing per bead se-fd9 (BLACKHAT: security audit wave3-5)*
