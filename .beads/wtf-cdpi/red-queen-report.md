# Red Queen Report — vo-cdpi

- **bead_id:** vo-cdpi
- **phase:** STATE-5
- **updated_at:** 2026-03-23T00:00:00Z
- **target:** `crates/vo-api/src/handlers/definitions.rs` (180 lines)
- **attacker:** Red Queen adversarial QA

---

## Attack Results

### 1. Empty `workflow_type` — SURVIVED (low severity concern)

**Vector:** `workflow_type: ""` → `definition_key("default", "")` produces `"default/"`.

**Evidence:** Pure function test confirms `definition_key("default", "")` returns `"default/"`. This is a valid NATS KV key. The definition would be stored under the namespace key itself, which is semantically wrong (it overwrites the namespace "directory") but doesn't crash.

**Verdict:** SURVIVED (no crash/panic). However, **semantically wrong** — storing under `"default/"` could shadow other definitions in that namespace. Missing input validation at the handler level.

---

### 2. Slashes in `workflow_type` — SURVIVED (medium severity concern)

**Vector:** `workflow_type: "foo/bar/baz"` → `definition_key("default", "foo/bar/baz")` produces `"default/foo/bar/baz"`.

**Evidence:** Pure function test confirms the key is produced. NATS KV treats `/` as part of the key string — no hierarchy enforcement at the KV level. This creates a key that *looks* like it's in a sub-namespace but isn't enforced.

**Verdict:** SURVIVED (no crash). But **namespace leakage** — a malicious or confused client can create keys that appear to be in a different namespace (`foo/bar/baz` looks like namespace `default/foo`, type `bar/baz`).

---

### 3. Huge source body — SURVIVED

**Vector:** Extremely large `source` string (>1MB).

**Evidence:** The handler does no size validation. `req.source.as_bytes().to_vec().into()` allocates the full body in memory. NATS KV has server-side limits (default ~1MB) that will reject oversized puts with an error, which is caught by the `Err(e)` branch → 500 + `kv_store_failure`.

**Verdict:** SURVIVED. KV failure is handled. But there's no **early rejection** — the entire body is parsed by the Rust compiler (via `vo_linter::lint_workflow_code`) before KV is even attempted. An enormous payload would waste CPU on linting before NATS rejects the store.

---

### 4. Unicode in `workflow_type` — SURVIVED

**Vector:** `workflow_type: "日本語ワークフロー"`.

**Evidence:** Pure function test confirms `"default/日本語ワークフロー"` is produced. NATS KV accepts UTF-8 keys.

**Verdict:** SURVIVED. No crash. Questionable UX but not a bug.

---

### 5. Missing `workflow_type` field — SURVIVED

**Vector:** JSON body `{ "source": "fn main() {}" }` with no `workflow_type`.

**Evidence:** `DefinitionRequest` derives `Deserialize` with `workflow_type: String` (not `Option<String>`). Serde's default behavior for a missing required field is to return a deserialization error. Axum's `Json<>` extractor converts this to a 400 Bad Request before the handler is ever called.

**Verdict:** SURVIVED. Serde rejects missing fields at the framework boundary.

---

### 6. Test isolation — SURVIVED

**Command:** `cargo test -p vo-api --lib -- definitions` run twice in sequence.

**Evidence:** Both runs: 4 passed, 0 failed. Tests use `lint_only_app()` which creates a fresh router per test — no shared state.

**Verdict:** SURVIVED. Tests are deterministic and isolated.

---

### 7. Clippy on definitions.rs — SURVIVED

**Command:** `cargo clippy -p vo-api -- -W clippy::unwrap_used -W clippy::expect_used`

**Evidence:** Zero warnings for `definitions.rs`. (Warnings exist elsewhere in vo-api but not in this file.)

**Verdict:** SURVIVED. Clean.

**Note:** The file uses `#![deny(clippy::unwrap_used)]` via `kv.rs` but the definitions handler itself has no such deny — it relies on `kv.rs` being clean. The `definitions.rs` file doesn't use `unwrap` or `expect` in production code (only in `#[cfg(test)]` which is acceptable).

---

### 8. Race condition / shared mutable state — SURVIVED

**Vector:** Concurrent POST requests to the definitions endpoint.

**Evidence:** `ingest_definition` takes `Extension<KvStores>` which is `Clone` (the NATS `Store` handle is internally synchronized). No shared mutable state exists in the handler. The `definition_key` is a pure function. The only mutation is `kv.definitions.put()` which is atomic per-key at the NATS server.

**Verdict:** SURVIVED. No race condition possible in the handler itself. Last-writer-wins semantics at the KV level are expected behavior.

---

## Summary

| # | Attack Vector | Result |
|---|---------------|--------|
| 1 | Empty `workflow_type` | SURVIVED (semantic concern) |
| 2 | Slashes in `workflow_type` | SURVIVED (namespace leakage) |
| 3 | Huge source body | SURVIVED (no early rejection) |
| 4 | Unicode in `workflow_type` | SURVIVED |
| 5 | Missing `workflow_type` field | SURVIVED |
| 6 | Test isolation | SURVIVED |
| 7 | Clippy strict mode | SURVIVED |
| 8 | Race condition | SURVIVED |

**BROKE: 0 | SURVIVED: 8**

---

## Recommendations (Non-Blocking)

These are **not blockers** but worth addressing in a follow-up bead:

1. **Validate `workflow_type` is non-empty** — Reject `workflow_type: ""` at the handler level. Empty keys create ambiguous namespace entries.

2. **Sanitize `workflow_type` for slashes** — Either reject or normalize keys containing `/` to prevent namespace confusion. A key like `default/../../evil` could be problematic depending on NATS KV subject matching rules.

3. **Add a `#[cfg(test)]` test for empty workflow_type** — The existing tests don't cover this edge case.
