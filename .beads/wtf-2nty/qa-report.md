# QA Report — wtf-2nty: serve: Load definitions from KV into registry on startup

**Date:** 2026-03-23
**Executor:** QA Enforcer (actual CLI execution)

## Checklist

### 1. `load_definitions_from_kv` exists in serve.rs
**PASS.** Function defined at `serve.rs:129-159`. Signature:
```rust
async fn load_definitions_from_kv(store: &Store) -> anyhow::Result<Vec<(String, WorkflowDefinition)>>
```
Uses `futures::StreamExt` to iterate keys, deserializes with `serde_json::from_slice`, logs `warn` on malformed entries, logs `info` on empty bucket.

### 2. `definitions` field on `OrchestratorConfig`
**PASS.** Field at `state.rs:24`:
```rust
pub definitions: Vec<(String, WorkflowDefinition)>,
```
Defaulted to `Vec::new()` in `Default` impl (line 36). Consumed in `OrchestratorState::new()` at lines 57-59, which iterates and calls `registry.register_definition()`.

### 3. No unwrap/expect in production code
**PASS.** Grep for `\.unwrap\(\)|\.expect\(` in serve.rs returned zero matches. The only error handling uses `context()`, `let Ok(...) else { continue; }`, and `match` — all graceful.

### 4. `cargo test -p wtf-cli -- serve`
**PASS.** 2 tests passed, 0 failed:
- `drain_runtime_signals_shutdown_and_waits_for_four_tasks` ... ok
- `drain_runtime_propagates_worker_error` ... ok

(Note: no unit test for `load_definitions_from_kv` itself — requires live NATS. Integration coverage noted.)

### 5. `cargo test -p wtf-actor --lib -- definitions`
**PASS.** 3 tests passed, 0 failed:
- `new_state_with_pre_seeded_definitions_populates_registry` ... ok
- `new_state_with_multiple_definitions` ... ok
- `new_state_with_empty_definitions_has_empty_registry` ... ok

### 6. Line counts
| File | Lines | Under 300? |
|------|-------|------------|
| `serve.rs` | 230 | PASS |
| `state.rs` | 276 | PASS |

## Verdict

**PASS**

All contract requirements satisfied. Definitions flow: KV scan → `load_definitions_from_kv` → `OrchestratorConfig.definitions` → `OrchestratorState::new()` → `WorkflowRegistry`. Malformed entries skipped with warn log. Empty bucket handled with info log. No panics in production paths. Both crate test suites green. File sizes within limits.
