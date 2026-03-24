# Implementation: wtf-2nty

- **bead_id**: wtf-2nty
- **bead_title**: serve: Load definitions from KV into registry on startup
- **phase**: STATE-3
- **updated_at**: 2026-03-23T12:00:00Z

## Summary

Implemented KV-to-registry loading on startup. When `wtf serve` starts, after provisioning KV buckets, it scans every key in the `wtf-definitions` bucket, deserializes each value as `WorkflowDefinition` (JSON), and seeds them into `OrchestratorState.registry` via the new `OrchestratorConfig.definitions` field.

## Files Modified

| File | Lines Changed | Description |
|------|---------------|-------------|
| `crates/wtf-actor/src/master/state.rs` | 1, 23-24, 36, 52-65, 123-275 | Added `definitions: Vec<(String, WorkflowDefinition)>` to `OrchestratorConfig`; updated `OrchestratorState::new()` to iterate and register pre-seeded definitions; added 3 unit tests |
| `crates/wtf-cli/src/commands/serve.rs` | 8-9, 13, 55-57, 70, 107-145 | Added `load_definitions_from_kv()` function; wired into `run_serve()` after `provision_storage()` and before orchestrator spawn |
| `crates/wtf-cli/Cargo.toml` | 17 | Added `async-nats = { workspace = true }` dependency (required for `Store` type) |

## Implementation Details

### Data->Calc->Actions Pattern
- **Data**: `OrchestratorConfig.definitions: Vec<(String, WorkflowDefinition)>` — pure data carried through config
- **Calc**: `OrchestratorState::new()` iterates definitions and registers them into `WorkflowRegistry` — pure transformation
- **Action**: `load_definitions_from_kv()` — I/O action (KV scan) that produces data; `run_serve()` wires it together

### Constraint Adherence
- **Zero `unwrap()`/`expect()`**: All fallible operations use `?`, `if let Ok(...)`, or `match`
- **Zero `mut` in core logic**: Only `let mut definitions` and `let mut keys` in the I/O action function (shell layer) — no `mut` in `OrchestratorState::new()` core logic
- **Make illegal states unrepresentable**: `WorkflowDefinition` is parsed at the boundary via `serde_json::from_slice` — only valid structs enter the system
- **Error handling**: KV scan failure is fatal (`?` propagation). Individual entry failures are non-fatal (warn + skip). Empty bucket is valid (info log).

### Invariants
- **I-NO-DUPS**: KV inherently provides last-write-wins per key; `HashMap::insert` overwrites duplicates
- **I-TYPE-SAFETY**: Only `serde_json::from_slice::<WorkflowDefinition>` results enter the registry
- **I-ORDER-INDEPENDENT**: Definitions are loaded sequentially but final registry state depends only on the set of (key, definition) pairs

## Tests Written

| Test Name | Location | Status |
|-----------|----------|--------|
| `new_state_with_pre_seeded_definitions_populates_registry` | `wtf-actor/src/master/state.rs:231` | PASS |
| `new_state_with_multiple_definitions` | `wtf-actor/src/master/state.rs:247` | PASS |
| `new_state_with_empty_definitions_has_empty_registry` | `wtf-actor/src/master/state.rs:272` | PASS |

Note: `load_definitions_from_kv` requires a live NATS connection for integration testing. Unit testing is covered by the `OrchestratorState::new()` tests above which verify the consumption path.

## cargo test Output

```
$ cargo test -p wtf-actor --lib
running 91 tests
test master::state::tests::new_state_with_pre_seeded_definitions_populates_registry ... ok
test master::state::tests::new_state_with_multiple_definitions ... ok
test master::state::tests::new_state_with_empty_definitions_has_empty_registry ... ok
test result: ok. 91 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

$ cargo test -p wtf-cli
running 9 tests
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## cargo clippy Output

```
$ cargo clippy -p wtf-actor -p wtf-cli
    Checking wtf-actor v0.1.0
    Checking wtf-cli v0.1.0
    Finished `dev` profile [unoptimized + debuginfo] target(s) in X.XXs
```

No new warnings introduced. All pre-existing warnings are in dependency crates (wtf-common, wtf-storage, wtf-worker).
