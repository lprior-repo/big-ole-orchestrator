# Test Plan Defects for wtf-c0l

## Defect 1: Redundant/Untestable Contract Verification Tests
**Severity**: Medium  
**Location**: `martin-fowler-tests.md` lines 24-28

The "Contract Verification Tests" section duplicates tests already covered in Happy Path/Error Path:
- `test_precondition_invocation_id_not_empty` duplicates `test_get_journal_returns_400_when_invocation_id_empty`
- `test_precondition_master_actor_available` is **not testable at runtime** - Extension<ActorRef> is compile-time enforced per contract.md P2
- `test_postcondition_200_with_journal_response_on_success` duplicates `test_get_journal_returns_200_with_entries_when_workflow_exists`
- `test_postcondition_404_on_not_found` duplicates `test_get_journal_returns_404_when_workflow_not_found`
- `test_postcondition_entries_sorted_by_seq` duplicates `test_get_journal_returns_sorted_entries_by_seq`

**Remediation**: Remove the entire "Contract Verification Tests" section. Tests should describe behavior, not implementation constraints. Keep only the behaviorally-named tests.

---

## Defect 2: Missing Test for Whitespace-Only Invocation ID
**Severity**: High  
**Location**: Edge Case Tests section

Empty string is tested (400), but whitespace-only strings (e.g., `"   "`) are not covered. The error taxonomy `Error::InvalidInvocationId` should include both empty and whitespace-only strings.

**Remediation**: Add `test_get_journal_returns_400_when_invocation_id_is_whitespace`

---

## Defect 3: Missing Test for Pre-actor Validation
**Severity**: High  
**Location**: Scenario 4 / Error Path Tests

The Given-When-Then for Scenario 4 states "No attempt to contact actor" when invocation_id is empty, but no test explicitly verifies that the handler validates invocation_id **before** making any actor call. This is critical for performance and correctness.

**Remediation**: Add `test_get_journal_validates_invocation_id_before_actor_call` that mocks the actor to verify `OrchestratorMsg::GetJournal` is never sent for invalid invocation_ids.

---

## Defect 4: Missing Explicit Invariant Tests
**Severity**: Medium  
**Location**: Missing from test plan

Contract invariants I1 and I3 lack explicit tests:
- **I1** (Response contains all journal entries): No test verifies the returned entries match exactly what the orchestrator returned
- **I3** (Each entry has seq, type, timestamp): Tested implicitly in happy path, but not explicitly verified

**Remediation**: Add `test_get_journal_response_contains_all_orchestrator_entries` and `test_get_journal_entries_have_required_fields`.

---

## Defect 5: No Test for Q4 Violation Scenario
**Severity**: Low  
**Location**: Missing test for contract.md VIOLATES Q4

Contract.md line 63 states: "If entries returned out of order → still returns 200 (data integrity issue)". No test explicitly covers this "returns 200" behavior when entries are unsorted.

**Remediation**: Add `test_get_journal_returns_200_regardless_of_entry_order` to document the current (suboptimal) behavior, or clarify whether entries MUST be sorted before returning 200.
