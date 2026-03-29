# Test Defects Report

**Bead:** vo-qgum
**Reviewer:** test-reviewer (glm-5-turbo)
**Verdict:** REJECTED
**Date:** 2026-03-23

---

## Summary

The test plan is structurally strong — 17 Given-When-Then scenarios with real values, zero mocks, and solid error-path coverage. However, **6 defects** require resolution before implementation proceeds. The most critical are the testability contradiction in `register_defaults` unit tests (D1/D4) and the inherently flaky timing assertions (D5).

---

## Defects

### D1 [CRITICAL] — `register_defaults` contract tests are classified as unit tests but cannot be unit tests

**Location:** `martin-fowler-tests.md` lines 43-48 (Unit Tests section)

**Problem:** `Worker::new` requires `js: async_nats::jetstream::Context` (worker.rs:102), which requires a live NATS connection. The plan places `test_postcondition_register_defaults_adds_echo_and_sleep_handlers` and `test_postcondition_register_defaults_preserves_existing_handlers` in `builtin.rs #[cfg(test)] mod tests`, but these tests **cannot execute** without NATS connectivity. The existing codebase explicitly acknowledges this pattern (worker.rs:311): *"Worker::new / register / run require a live NATS Context. Those paths are covered by integration tests."*

**Fix:** Either:
- (a) Move all `register_defaults` verification tests to the `worker_integration_tests.rs` E2E section (where NATS is available), verifying handler registration through actual task dispatch (e.g., enqueue an echo task and verify it's processed), OR
- (b) Refactor `Worker` to expose a `handlers()` accessor or make `handlers` `pub(crate)` for test inspection, OR
- (c) Extract handler registration into a trait/strategy that can be tested without a full `Worker` instance.

### D2 [HIGH] — `worker.handlers` is private; plan assumes direct field access

**Location:** `martin-fowler-tests.md` line 44: *"inspect worker.handlers keys"*

**Problem:** The `handlers` field is private (worker.rs:92, no `pub` modifier). The plan instructs the implementer to "inspect `worker.handlers` keys" but provides no mechanism for doing so. The existing integration tests never access internal state — they verify behavior through the run loop.

**Fix:** Specify the access strategy. Options:
- Add `pub(crate) fn has_handler(&self, name: &str) -> bool` to `Worker`
- Make `handlers` `pub(crate)` with a `#[cfg(test)]` gate
- Remove direct inspection and verify through dispatch behavior in integration tests

### D3 [HIGH] — Timing assertions in Scenarios 4 and 5 are inherently flaky

**Location:** `martin-fowler-tests.md` lines 96, 103

**Problem:**
- Scenario 4: "at least 10ms have elapsed since the handler was called" — lower-bound time assertions are flaky under CI load, container scheduling, and CPU throttling.
- Scenario 5: "elapsed time is less than 5ms" for `ms=0` — upper-bound time assertions are extremely flaky; even cooperative yields can exceed 5ms on a loaded system.

**Fix:** Use `tokio::time::pause()` for deterministic time control:
```rust
let time_handle = tokio::time::pause();
// ... call handler ...
time_handle.advance(Duration::from_millis(10)).await;
assert_eq!(time_handle.current(), Duration::from_millis(10));
```
This makes the test deterministic regardless of system load. The real-timer integration test (Scenario 15) already validates the actual sleep behavior through NATS.

### D4 [MEDIUM] — `test_sleep_with_ms_u64_max_is_accepted` lacks implementation strategy

**Location:** `martin-fowler-tests.md` line 39

**Problem:** The plan says *"test does NOT actually sleep for u64::MAX ms"* but provides no mechanism. Without `tokio::time::pause()`, a test that calls `sleep_handler` with `{"ms": 18446744073709551615}` will hang for ~584 million years. The implementer must use `tokio::time::pause()` + `advance()` or a timeout with explicit expected failure, but the plan doesn't specify which.

**Fix:** Specify `tokio::time::pause()` as the mechanism and assert the result is `Ok(b"\"slept\"")` after advancing time by 0ms (don't advance — just verify the payload was accepted and returned success).

### D5 [MEDIUM] — Missing valid-JSON-non-object edge cases for `sleep_handler`

**Location:** Error Taxonomy (contract.md lines 106-111), `martin-fowler-tests.md` error section (lines 24-32)

**Problem:** The contract's error taxonomy covers three categories: "not valid JSON", "missing ms key", "ms value is not u64". But there's a fourth category: **valid JSON that is not an object**. The payloads `42` (JSON number), `"hello"` (JSON string), `null`, `true` are all valid JSON, and `serde_json::from_slice` will succeed — but `.as_object()` will return `None`. The plan has `test_sleep_rejects_json_array_payload` for arrays but misses numbers, strings, null, and booleans. These are distinct parsing branches.

**Fix:** Add tests:
- `test_sleep_rejects_json_number_payload` — payload `b"42"`
- `test_sleep_rejects_json_null_payload` — payload `b"null"`
- (Strings and booleans are less likely in practice but could be added for completeness)

### D6 [LOW] — Missing test for `sleep_handler` with extra JSON fields (lenient parsing)

**Location:** `martin-fowler-tests.md` happy path section (lines 18-20)

**Problem:** Real-world payloads frequently include metadata beyond the handler's required fields. The contract says payload must "contain" a `"ms"` key (contract.md:47, word "containing" implies lenient). But there's no test verifying `{"ms": 10, "trace_id": "abc-123", "extra": true}` is accepted. This is a common production scenario where the engine adds routing/trace metadata.

**Fix:** Add `test_sleep_accepts_payload_with_extra_json_fields` — payload `b#"{"ms":10,"trace_id":"abc"}"#`, assert `Ok(slept)`.

---

## Positive Observations

These strengths are worth preserving in the revision:

1. **Zero mocks** — Every test runs the real async functions with real `Bytes` values. Testing Trophy philosophy is respected.
2. **Specific Given-When-Then values** — Scenarios use concrete byte sequences (`b"\x00\x01\x02\xff\xfe"`) and specific JSON payloads, not vague descriptions.
3. **Error path exhaustiveness** — 9 distinct failure modes for `sleep_handler` covering invalid UTF-8, invalid JSON, missing key, wrong types, empty, and array payloads.
4. **Integration tests follow existing patterns** — The E2E tests mirror the structure of `worker_integration_tests.rs` (NATS setup, enqueue, spawn worker with shutdown timer, verify consumption).
5. **Contract verification tests** — Explicit tests for idempotency, handler preservation, and no-panic invariants.
6. **Verification commands** include `cargo clippy -D warnings` and the correct test filters.

---

## Resubmission Criteria

Address all 6 defects above. At minimum:
- [ ] Resolve D1+D2: Move `register_defaults` verification to integration tests OR specify an access strategy for `handlers`
- [ ] Resolve D3: Use `tokio::time::pause()` for all timing assertions in unit tests
- [ ] Resolve D4: Specify `tokio::time::pause()` for u64::MAX test
- [ ] Resolve D5: Add at minimum `test_sleep_rejects_json_number_payload` and `test_sleep_rejects_json_null_payload`
- [ ] Resolve D6: Add `test_sleep_accepts_payload_with_extra_json_fields`
