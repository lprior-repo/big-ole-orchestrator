# Test Defects: wtf-jblq

```json
{"bead_id":"wtf-jblq","phase":"test-review","updated_at":"2026-03-22T00:00:00Z"}
```

## Doctrine Violations

### 1. Dave Farley Testing Trophy (Real Execution) — FAILED

**VIOLATION**: Tests use mock/fake SSE servers instead of real execution
- Lines 10-11: "Given: A running SSE server at `http://localhost:8080`..." — this is a mock, not a real SSE server
- Lines 14-17: "Given: A running SSE server that returns a plain JSON `data:` frame" — mock server
- Lines 51-54: "Given: A server that is not listening on the target port" — this IS real, but only for connection refused
- Lines 57-59: "Given: A TCP server that returns HTTP 503..." — fake TCP server, not real HTTP endpoint

**REQUIREMENT**: The Testing Trophy demands "tremendous amounts of integration and end-to-end tests that validate the system actually works." These tests validate the system against fake servers, not real ones.

**FIX NEEDED**: At least 50% of integration tests should target REAL infrastructure (actual HTTP servers, real SSE endpoints).

---

### 2. Dan North BDD (Given-When-Then Naming) — PARTIAL PASS

**ISSUE 1**: Test names reveal implementation, not behavior
- Line 9: `test_watch_namespace_returns_stream_of_instance_views` — "returns_stream" is HOW, not WHAT
- Line 34: `test_backoff_policy_default_values` — exposes implementation of delay calculation
- Line 46: `test_watch_namespace_returns_error_on_empty_base_url` — implementation detail ("returns error")

**SHOULD BE** (behavior-focused):
- "namespace watch yields instance data when SSE is valid"
- "backoff doubles after each failure"
- "empty base URL causes watch to fail with request error"

**ISSUE 2**: White-box contract tests
- Lines 125-128: `test_watch_state_attempt_resets_on_success` — tests internal `WatchState.attempt` field, not external behavior
- Lines 130-133: `test_watch_state_attempt_increments_on_failure` — internal state inspection
- Lines 135-138: `test_watch_state_attempt_saturates_at_u32_max` — internal state

**SHOULD BE**: These should test observable backoff delay timing, not internal counters.

---

### 3. Dave Farley ATDD (Separation of WHAT from HOW) — FAILED

**VIOLATION**: Line 10-11
```
Given: A running SSE server at `http://localhost:8080` with namespace `payments` 
that returns a valid SSE payload with key-prefixed format
```
The phrase "with key-prefixed format" reveals HOW the data is encoded, not WHAT the behavior is.

**VIOLATION**: Line 80
```
Then: Returns the concatenated payload with `workflow_type` and `phase` joined by `,`
```
"joined by comma per SSE spec" exposes SSE implementation detail.

**VIOLATION**: Lines 91-92
```
Then: `instance_id` is `"01ABC"` (rsplit '/' takes last segment, not empty string from trailing slash)
```
This describes the IMPLEMENTATION (`rsplit '/'`), not the contract.

**FIX NEEDED**: All scenario descriptions must describe observable behavior, not implementation details like parsing strategy, data format, or internal algorithms.

---

## Missing Combinatorial Permutations

| Missing Test Case | Description |
|---|---|
| **Multi-failure sequence** | Backoff recovery with 4+ failures (only 2-failure case tested in line 57) |
| **Out-of-order events** | Events arriving with non-sequential `last_event_seq` |
| **Empty instance_id** | SSE payload with `"instance_id": ""` |
| **Rapid event interleaving** | Multiple namespaces, events arriving out of temporal order |
| **Backoff boundary conditions** | `delay_for_attempt(0)` after max attempts, exact cap behavior |

---

## Advanced Paradigms — NOT CONSIDERED

- **Property-based testing**: No `quickcheck` or `proptest` style tests for parsing invariants
- **Fuzzing**: No `cargo-fuzz` or similar for SSE payload parsing
- **Mutation testing**: No mutation coverage analysis

---

## Summary

| Doctrine | Status |
|---|---|
| Testing Trophy (Real Execution) | ❌ REJECT — over-reliance on mocks |
| Dan North BDD | ⚠️ PARTIAL — Given-When-Then present but names expose implementation |
| Dave Farley ATDD | ❌ REJECT — WHAT/HOW separation violated throughout |
| Combinatorial Permutations | ⚠️ PARTIAL — core cases present but missing permutations |
| Advanced Paradigms | ❌ MISSING — no property/fuzz/mutation testing |

---

## Required Actions

1. **Replace mock SSE servers** with at least one real HTTP server running actual SSE in integration tests
2. **Rewrite white-box contract tests** (lines 125-138) to test observable backoff delay timing, not internal state
3. **Rewrite scenario descriptions** to remove implementation details (key-prefixed format, `rsplit`, SSE spec comma joining, etc.)
4. **Add missing permutations**: multi-failure sequence, out-of-order events, empty instance_id, rapid interleaving
5. **Consider adding** property-based tests for SSE parsing invariants
