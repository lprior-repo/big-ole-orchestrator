# Test Defects Report: wtf-5gtk

bead_id: wtf-5gtk
timestamp: 2026-03-22
reviewer: Test Reviewer
standard: Dan North BDD, ATDD, Testing Trophy
state: 2

---

## ACCEPTANCE CRITERIA STATUS

| Criterion | Status | Evidence |
|-----------|--------|----------|
| Integration tests exist | ✅ TRUE | journal_test.rs created with 7 tests |
| Unit test names renamed to BDD | ✅ TRUE | journal.rs:208-227 uses Given-When-Then |
| Test assertions match contract | ❌ FALSE | Tests assert 404, contract says 400 |

---

## CRITICAL DEFECT #1: Wrong HTTP Status Code Assertions

**Severity**: CRITICAL  
**Category**: Contract Violation (ATDD)
**Evidence**: journal_test.rs lines 31, 48, 65

### The Problem

Three integration tests assert `StatusCode::NOT_FOUND` (404) but the contract specifies `StatusCode::BAD_REQUEST` (400) for invalid IDs.

**Contract.md postcondition (lines 83-84):**
> "Invalid ID (400): Returns `ApiError` with code `"invalid_id"` and message `"empty invocation id" or `"bad id"`"

**martin-fowler-tests.md Section 4:**
> "GIVEN an empty string as the instance ID WHEN ... THEN the response status is 400 Bad Request AND the error code is `"invalid_id"`"

### Defect Details

| Test (journal_test.rs) | Asserts | Should Be | Root Cause |
|------------------------|---------|-----------|------------|
| `given_empty_id_when_get_journal_then_bad_request` (L18) | 404 | 400 | Double-slash path `/api/v1/workflows//journal` → route mismatch |
| `given_whitespace_id_when_get_journal_then_not_found` (L35) | 404 | 400 | `%20` encoded whitespace may cause route mismatch |
| `given_id_without_namespace_when_get_journal_then_not_found` (L52) | 404 | 400 | Single-segment ID may cause route mismatch |

### Handler Correctness (journal.rs)

The handler `parse_journal_request_id` (lines 75-90) correctly returns `BAD_REQUEST` for:
- Empty strings (line 76-80)
- Whitespace-only strings (line 76)
- IDs without namespace separator via `split_path_id` (lines 83-89)

**The tests are asserting the WRONG layer's error.** The 404s come from Axum path routing rejecting malformed paths, not from the handler returning 400.

---

## CRITICAL DEFECT #2: Missing Happy Path Test

**Severity**: HIGH  
**Category**: Combinatorial Permutation Gap
**Evidence**: No test verifies 200 OK with actual journal entries

### martin-fowler-tests.md Section 1:
> "GIVEN a valid namespaced instance ID with multiple journal events WHEN GET /api/v1/workflows/:id/journal is called THEN the response status is 200 OK AND the response body is a JournalResponse"

**Current Status**: No integration test verifies the success path with events.

**Required**: Add test with mocked event store returning ReplayBatch::Event entries → verify 200 + JournalResponse structure.

---

## HIGH PRIORITY DEFECT #3: Missing Empty Entries Test

**Severity**: HIGH  
**Category**: Combinatorial Permutation Gap  
**Evidence**: No test verifies 200 with empty entries array

### martin-fowler-tests.md Section 2:
> "GIVEN a valid namespaced instance ID with zero journal events WHEN ... THEN the response status is 200 OK AND the response body has entries array with length 0"

**Required**: Add test with mocked event store returning only ReplayBatch::TailReached → verify 200 + `entries: []`

---

## HIGH PRIORITY DEFECT #4: Wrong 404 Test

**Severity**: HIGH  
**Category**: Contract Violation  
**Evidence**: Test 4 (journal_test.rs:69) returns 500 for "actor unavailable", not 404 for "not found"

### martin-fowler-tests.md Section 3:
> "GIVEN a valid-format but non-existent instance ID WHEN ... THEN the response status is 404 NOT FOUND AND the error code is `"not_found"`"

**Current Status**: No test verifies valid namespace + non-existent instance → 404.

**Test 4** (`given_valid_namespaced_id_when_get_journal_without_actor_then_internal_error`) tests actor unavailability → 500, not non-existent instance → 404.

---

## MODERATE DEFECT #5: Missing validate() Contract Verification

**Severity**: MEDIUM  
**Category**: Contract Verification Gap  
**Evidence**: No test calls `JournalResponse::validate()`

### martin-fowler-tests.md Section 9:
> "GIVEN a successful journal response THEN validate() on JournalResponse returns Ok(())"

**Required**: Add test that constructs unsorted entries, calls validate(), expects Err.

---

## TESTING TRIUMPH ANALYSIS

| Layer | Required | Actual | Status |
|-------|----------|--------|--------|
| E2E (real orchestrator) | High | 0 | ❌ |
| Integration (mocked actor) | High | 7 | ⚠️ Wrong assertions |
| Unit (internal helpers) | Low | 4 | ✅ BDD named |

**Verdict**: Tests exist but assert wrong behavior. Implementation is correct per contract; tests are wrong.

---

## SUMMARY

| Defect | Severity | Category | Status |
|--------|----------|----------|--------|
| Integration tests exist | ✅ | Coverage | FIXED |
| Unit tests BDD named | ✅ | Dan North | FIXED |
| **Wrong status code assertions** | **CRITICAL** | **ATDD** | **❌ NEW** |
| Missing happy path test | HIGH | Permutation | ❌ NEW |
| Missing empty entries test | HIGH | Permutation | ❌ NEW |
| Wrong 404 test | HIGH | Contract | ❌ NEW |
| Missing validate() test | MEDIUM | Contract | ❌ NEW |

**OVERALL STATUS**: REJECTED

**VERDICT**: Tests exist and use BDD naming, but assert 404 for invalid IDs when contract specifies 400. Tests are coupled to routing-layer behavior rather than handler-layer contract.

---

## REQUIRED REMEDIATION

1. **Fix assertions**: Change lines 31, 48, 65 from `StatusCode::NOT_FOUND` to `StatusCode::BAD_REQUEST`
2. **Add happy path test**: Mock event store with ReplayBatch::Event entries → verify 200 + JournalResponse
3. **Add empty entries test**: Mock event store with only TailReached → verify 200 + `entries: []`
4. **Add not-found test**: Mock event store to return error on open_replay_stream → verify 404 + "not_found"
5. **Add validate() test**: Verify JournalResponse::validate() catches unsorted entries

---

## NEXT STATE

**STATE: 2** (test-defects documented, tests flawed and require fixes)
