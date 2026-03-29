# Kani Justification: vo-cdpi

## Decision: Kani model checking NOT required.

## 1. Critical State Machines

**None exist.** `ingest_definition` is a stateless HTTP handler with no internal state transitions. The control flow is a pure decision tree:

```
request → validate workflow_type → lint → [valid? store in KV] → response
```

There is no state machine with reachable invalid states. The handler has no accumulator, no loop counter, no shared mutable state, and no resource lifecycle beyond the `Extension<KvStores>` provided by axum's dependency injection.

## 2. Why Kani Would Add No Value

Kani targets properties like "can this assertion fail?" or "can this index be out of bounds?". The handler's correctness properties are:

| Property | Enforcement Mechanism |
|---|---|
| Empty `workflow_type` rejected | Explicit guard at line 13, tested by `empty_workflow_type_rejected` and `whitespace_only_workflow_type_rejected` |
| Invalid definitions not stored | Structural: `kv.definitions.put` is inside `if valid` block at line 34; the `else` and `Err` branches have no KV path |
| Malformed KV key impossible | `definition_key("default", "")` would produce `"default/"` (proved by test), but the validation guard at line 13 prevents reaching that path with empty input |

These are **structural guarantees enforced by Rust's control flow and ownership**, not invariants that require exhaustive state-space exploration to verify.

## 3. What Tests Cover

- 4 unit tests covering: key format, parse error rejection, valid definition acceptance, empty workflow_type rejection, whitespace-only rejection, malformed key proof
- The KV store path (`valid == true`) requires a live NATS connection and is covered by E2E pipeline tests
- All Result types are handled via exhaustive `match` — no `unwrap`, no `expect`, no panic paths

## 4. Honest Assessment

Kani could theoretically verify that no code path stores an invalid definition in KV without checking `valid` first. But this is already **structurally impossible**: the `kv.definitions.put` call is lexically nested inside the `if valid` branch. Rust's borrow checker and control flow make it unreachable otherwise. No concurrent access pattern could bypass this since there is no shared mutable state between requests — each handler invocation is independent.

**Risk of undetected defect via lack of Kani: negligible.**
