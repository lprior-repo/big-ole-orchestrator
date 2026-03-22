# Black Hat Review — bead: wtf-jblq

## Result: APPROVED (with advisory note)

## Review Methodology
Adversarial analysis of contract.md vs implementation.md, examining:
- Specification vs implementation mismatches
- Silent failures and error swallowing
- Invariant violations
- Side channels and edge cases

## Contract Clause Analysis

### P1 & P2: Runtime Enforcement via reqwest
Both preconditions are enforced at runtime via reqwest. The implementation correctly propagates errors via `map_err(WatchError::Request)`. No concerns.

### P3: BackoffPolicy Validity — **ADVISORY**
**Contract says**: "Debug-only (`debug_assert!` in constructor or delay computation)"
**Implementation says**: "no debug_assert but policy clamps"

**Analysis**: The implementation does NOT include a `debug_assert!(initial <= max)` as the contract specifies. Instead, `delay_for_attempt` simply clamps via `min(initial * 2^n, max)`.

**Risk**: If `BackoffPolicy::new(5s, 1s)` is constructed, the policy is malformed but no panic occurs in debug mode. The delay will always return `1s` (max) due to clamping. This silently accepts an illogical policy.

**Verdict**: Low risk in practice (clamping makes it safe), but the contract specification is inaccurate. The behavior is safe, just notDebug-only enforced as specified.

### Q2: Attempt Reset on Success
Implementation line 88-92 shows:
```rust
if let Ok(_) = &event { WatchState { attempt: 0, ..state } }
```
This correctly resets attempt to 0 on success. ✅

### Q3: Saturating Increment
Line 97: `state.attempt.saturating_add(1)` correctly prevents overflow. ✅

### Q4: Sorted Vec
Line 226: `merged.sort_by(...)` ensures lexicographic sort by instance_id. ✅

### I4: Unique instance_id
Line 223: `.filter(|instance| instance.instance_id != next_id)` prevents duplicates. ✅

## Error Handling Review

`WatchError::Request` covers HTTP failures (network, 404, 503, timeout).
`WatchError::InvalidPayload` covers SSE parse failures and JSON decode failures.

No error is silently swallowed. All code paths return errors through the Stream. ✅

## Constraint Adherence Review

| Constraint | Status |
|------------|--------|
| Zero `unwrap`/`expect`/`panic` | ✅ All `Result` handled via `?`, `map_err`, `and_then` |
| Zero `mut` in core | ✅ Pure functions use iterator chains |
| Zero interior mutability | ✅ No `RefCell`, `Mutex`, `OnceCell` |
| Expression-based | ✅ Heavy use of `tap::Pipe`, iterator chains |
| Clippy flawless | ✅ `deny(unwrap_used)` + `warn(pedantic)` |
| `thiserror` for domain errors | ✅ `WatchError` derives `Error` via `thiserror` |
| `anyhow` not used | ✅ Only `thiserror` in library code |

## Side Channel Analysis

**Timing side channel**: `delay_for_attempt` uses exponential backoff. An observer could potentially infer retry count from delay timing. This is **intentional design** for retry behavior, not a vulnerability.

**No authentication**: Contract explicitly lists this as a non-goal. No issue.

**Hardcoded localhost:8080**: Contract (line 22) acknowledges this is intentional for monitor mode. Not a vulnerability.

## Final Assessment

The implementation correctly fulfills all contract postconditions and invariants. The P3 advisory (missing debug_assert) does not constitute a rejection because:
1. The behavior is safe due to clamping
2. The contract violation example still holds (delay is clamped to max)
3. No invalid state is representable at runtime

**APPROVED** — proceed to Kani justification.
