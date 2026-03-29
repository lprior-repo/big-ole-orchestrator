# Red Queen Adversarial Report — vo-2nty

**Target:** `load_definitions_from_kv` in `crates/vo-cli/src/commands/serve.rs:129-159`
**Subject:** serve: Load definitions from KV into registry
**Date:** 2026-03-23

---

## Attack Vector Results

### 1. Malformed JSON in KV — SURVIVED

**Code path:** `serve.rs:146-148`
```rust
match serde_json::from_slice::<WorkflowDefinition>(value.as_ref()) {
    Ok(def) => definitions.push((key, def)),
    Err(e) => tracing::warn!(key = %key, error = %e, "skipping malformed definition in KV"),
}
```
**Verdict:** Deserialization failure caught by `match`. Malformed entries logged at `warn` and skipped. No panic path.

### 2. Empty bucket — SURVIVED

**Code path:** `serve.rs:152-156`
```rust
if definitions.is_empty() {
    tracing::info!("No workflow definitions found in KV");
} else {
    tracing::info!(count = definitions.len(), "Loaded workflow definitions from KV");
}
Ok(definitions)
```
**Verdict:** Empty vec is valid. Info log emitted. `run_serve` proceeds with empty `definitions` vec, which `OrchestratorState::new()` handles (tested in `master::state::tests::new_state_with_empty_definitions_has_empty_registry`).

### 3. Very large number of definitions — SURVIVED

**Code path:** `serve.rs:137` — `let mut definitions = Vec::new();`
**Verdict:** Unbounded `Vec::new()` with push-per-key. No pre-allocation upper bound, but this is standard Rust growth (amortized O(1)). KV scan is bounded by NATS page size internally. No fixed-size buffer, no stack overflow risk. This is acceptable — a `Vec` grows on heap. **MINOR observation:** no explicit capacity hint, but `with_capacity` would be premature without knowing key count.

### 4. Missing `workflow_type` in stored JSON — SURVIVED

**Analysis:** `WorkflowDefinition` struct (`crates/vo-common/src/types/workflow.rs:19-26`) does NOT have a `workflow_type` field. Its fields are `paradigm`, `graph_raw`, `description`. The key in KV *is* the workflow type (function signature: `Vec<(String, WorkflowDefinition)>`). So "missing workflow_type" is impossible — the key is always present from the KV iteration.

If `paradigm` is missing from JSON, `serde_json::from_slice` returns `Err` (missing required field), which falls into the `Err` arm at `serve.rs:148`. **SURVIVED.**

### 5. Test isolation — SURVIVED

**Command:** `cargo test -p vo-actor --lib` (run twice)

| Run | Result |
|-----|--------|
| 1 | 123 passed, 0 failed (0.02s) |
| 2 | 123 passed, 0 failed (0.02s) |

**Verdict:** Deterministic, no shared state leakage, no ordering dependency.

### 6. Clippy strict — SURVIVED

**Command:** `cargo clippy -p vo-actor -p vo-cli -- -W clippy::unwrap_used`

**Result:** 0 errors. 0 `unwrap_used` violations. 33 pedantic warnings (all `doc_markdown`, `missing_errors_doc`, `uninlined_format_args` — pre-existing, not from this bead).

`load_definitions_from_kv` uses no `.unwrap()`, `.expect()`, or panicking paths. Error propagation via `?` and `anyhow::Context`.

---

## Summary

| # | Attack Vector | Result | Severity |
|---|--------------|--------|----------|
| 1 | Malformed JSON in KV | SURVIVED | — |
| 2 | Empty bucket | SURVIVED | — |
| 3 | Very large number of definitions | SURVIVED | — |
| 4 | Missing workflow_type in JSON | SURVIVED | — |
| 5 | Test isolation (2x run) | SURVIVED | — |
| 6 | Clippy `unwrap_used` strict | SURVIVED | — |

**Crown: DEFENDED** — 0 survivors across 6 attack dimensions.

**Note:** `load_definitions_from_kv` itself has no unit tests (requires live NATS `Store`). The consumption path is covered by `master::state::tests::new_state_with_*` tests. An integration test with live NATS would close this gap.
