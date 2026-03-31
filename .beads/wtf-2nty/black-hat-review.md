# Black Hat Review — wtf-2nty

**Date:** 2026-03-23
**Scope:** `crates/wtf-actor/src/master/state.rs`, `crates/wtf-cli/src/commands/serve.rs`

---

## STATUS: APPROVED

---

## 1. Hallucinated APIs

### 1a. Does `WorkflowDefinition` exist in `wtf_common`?

**PASS.** `WorkflowDefinition` is defined at `crates/wtf-common/src/types/workflow.rs:19` and re-exported through the chain:
- `wtf_common` -> `wtf_actor::master::registry` (pub use at `registry.rs:4`) -> `wtf_actor::master` (pub use at `mod.rs:5`)

The import `use wtf_actor::master::{MasterOrchestrator, OrchestratorConfig, WorkflowDefinition}` at `serve.rs:13` resolves correctly.

### 1b. Does `Store::keys()` match usage?

**PASS.** Verified against `async_nats` 0.46.0 docs:
- `Store::keys()` -> `Result<Keys, HistoryError>` (returns `Keys` struct)
- `Keys` impls `Stream<Item = Result<String, Error<WatcherErrorKind>>>`
- Code at `serve.rs:132-135` unwraps with `.context()` — correct
- Code at `serve.rs:139` uses `keys.next().await` via `futures::StreamExt` — correct (yields `Option<Result<String, E>>`)
- Code at `serve.rs:140` destructures `let Ok(key) = key_result else { continue; }` — correct

### 1c. Does `Store::get()` match usage?

**PASS.** Verified against `async_nats` 0.46.0 docs:
- `Store::get(key) -> Result<Option<Bytes>, EntryError>`
- Code at `serve.rs:143`: `let Ok(Some(value)) = store.get(&key).await else { continue; }` — correct

---

## 2. Silent Failures

### 2a. Key iteration errors silently swallowed

**WARNING (non-blocking).** At `serve.rs:140`, when `key_result` is `Err`, the code silently `continue`s without logging. This could mask NATS connectivity issues mid-scan. Similarly at `serve.rs:143`, a `store.get()` error is silently skipped.

**Risk:** Low. The doc comment at line 124-128 explicitly documents "Malformed entries are logged at `warn` level and skipped." However, key-level errors are *not* malformed entries — they're stream failures that could indicate connection problems. A `tracing::warn!` on the error path would improve observability.

**Verdict:** Not a blocking issue for approval. Recommend adding `tracing::warn!(error = ?key_result, "failed to iterate key")` at line 141.

### 2b. No other silent failures detected

All error paths in `run_serve()` use `.context()` and propagate via `?`. `drain_runtime()` propagates all task errors. `provision_storage()` propagates both stream and KV errors.

---

## 3. Contract Violations

### 3a. Is load called before orchestrator spawn?

**PASS.** At `serve.rs:56-58`, `load_definitions_from_kv(&kv.definitions)` is called *before* `MasterOrchestrator::spawn()` at line 74. The loaded `definitions` vec is passed into `OrchestratorConfig.definitions` at line 71, which seeds the registry in `OrchestratorState::new()` at `state.rs:55-59`.

### 3b. OrchestratorConfig fields all populated

**PASS.** All `Option<>` fields (`event_store`, `state_store`, `task_queue`, `snapshot_db`) are populated at `serve.rs:60-72` before spawn. No `None` values leak through.

---

## 4. Dead Code / Unused Imports

### `state.rs`

**PASS.** All imports are used:
- `WorkflowDefinition`, `WorkflowRegistry` — used in struct fields and `new()`
- `InstanceArguments`, `InstanceMsg`, `InstanceSeed` — used in `build_instance_args()` and `active` field
- `ActorRef` — used in `active` and `get()` return type
- `HashMap` — used in `active` field
- `Arc` — not directly used in this file (removed from audit)

Wait — re-checking: `Arc` at line 5 is imported but not directly used in `state.rs`. The `Arc` wrapping happens in `OrchestratorConfig` fields but those are `Option<Arc<dyn EventStore>>` etc., which are defined by the types themselves, not by `Arc` being used in this file's code.

**Actually:** `Arc` is NOT used in `state.rs`. The `OrchestratorConfig` struct fields reference `Arc<dyn EventStore>` etc., but `state.rs` never calls `Arc::new()` or `.clone()` on an `Arc` — it only stores the `Option<Arc<...>>` values. The `Arc` type is brought in through the field types but the import `use std::sync::Arc;` is **unused**.

**WAIT — re-verify:** `state.rs:108-110` does `self.config.event_store.clone()` etc. The `.clone()` on `Option<Arc<dyn EventStore>>` works because `Arc` impls `Clone`, but the import `Arc` is not needed at the use-site since `clone()` is a method. However, Rust needs the `Arc` name to be in scope for the type annotations in `OrchestratorConfig`... Actually no, `OrchestratorConfig` is defined in the *same file* and the `Arc` IS used in the struct definition at lines 18-22.

**Final verdict:** `Arc` IS used at `state.rs:18-22` in `OrchestratorConfig` field types. **PASS.**

### `serve.rs`

**PASS.** All imports are used:
- `PathBuf` — `ServeConfig.data_dir`
- `Arc` — wrapping stores at lines 60-62
- `anyhow::Context` — `.context()` calls
- `Store` — `load_definitions_from_kv` param type
- `StreamExt` — `keys.next().await`
- `Actor` — `MasterOrchestrator::spawn()` (trait bound)
- `watch`, `JoinHandle` — shutdown signaling and task types
- All `wtf_*` imports — actively used

---

## Summary

| Check | Result |
|-------|--------|
| Hallucinated APIs | PASS — all verified against source and docs |
| Silent failures | 1 warning (non-blocking): key iteration errors silently skipped at serve.rs:140-141 |
| Contract violations | PASS — load before spawn enforced |
| Dead code | PASS — all imports used |

**Recommendations (non-blocking):**
1. Add `tracing::warn!` for key iteration errors at `serve.rs:141` to distinguish stream failures from malformed entries
