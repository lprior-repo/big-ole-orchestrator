bead_id: wtf-c0l
bead_title: "bead: Implement get_journal handler"
phase: "STATE 1"
updated_at: "2026-03-20T23:15:00Z"

# Martin Fowler Test Plan

## Happy Path Tests
- test_get_journal_returns_200_with_entries_when_workflow_exists
- test_get_journal_returns_200_with_empty_entries_when_no_events
- test_get_journal_returns_sorted_entries_by_seq

## Error Path Tests
- test_get_journal_returns_404_when_workflow_not_found
- test_get_journal_returns_400_when_invocation_id_empty
- test_get_journal_returns_400_when_invocation_id_whitespace_only
- test_get_journal_returns_500_when_actor_communication_fails

## Edge Case Tests
- test_get_journal_handles_special_characters_in_invocation_id
- test_get_journal_handles_long_invocation_id
- test_get_journal_handles_workflow_with_single_entry

## Invariant Tests
- test_invariant_i1_response_contains_all_journal_entries
- test_invariant_i3_each_entry_has_required_fields

## Q4 Violation Test
- test_q4_violation_entries_unsorted_still_returns_200

## Validation Order Tests
- test_invocation_id_validated_before_actor_call

## Given-When-Then Scenarios

### Scenario 1: Successfully retrieve journal with entries
**Given**: A workflow with `invocation_id = "abc-123"` exists and has 3 journal entries
**When**: `get_journal(Extension(master), Path("abc-123"))` is called
**Then**:
- Returns `Ok(200 OK)`
- Response body is `Json(JournalResponse { invocation_id: "abc-123", entries: [...] })`
- Entries are sorted by seq ascending (0, 1, 2)
- Each entry contains seq, type, timestamp fields

### Scenario 2: Retrieve journal for workflow with no events
**Given**: A workflow with `invocation_id = "empty-456"` exists but has no journal entries
**When**: `get_journal(Extension(master), Path("empty-456"))` is called
**Then**:
- Returns `Ok(200 OK)`
- Response body is `Json(JournalResponse { invocation_id: "empty-456", entries: [] })`
- Empty array is valid response (not an error)

### Scenario 3: Return 404 when workflow doesn't exist
**Given**: No workflow with `invocation_id = "nonexistent"` exists
**When**: `get_journal(Extension(master), Path("nonexistent"))` is called
**Then**:
- Returns `Err(404 NotFound)`
- No response body
- No modification to system state

### Scenario 4: Return 400 when invocation_id is empty
**Given**: An empty string is provided as invocation_id
**When**: `get_journal(Extension(master), Path(""))` is called
**Then**:
- Returns `Err(400 BadRequest)`
- No attempt to contact actor

### Scenario 5: Return 400 when invocation_id is whitespace-only
**Given**: A whitespace-only string `"   "` is provided as invocation_id
**When**: `get_journal(Extension(master), Path("   "))` is called
**Then**:
- Returns `Err(400 BadRequest)`
- No attempt to contact actor

### Scenario 6: Handle actor communication failure
**Given**: The master ActorRef's channel is closed/failing
**When**: `get_journal(Extension(master), Path("valid-id"))` is called
**Then**:
- Returns `Err(500 Internal Server Error)`
- Error is logged appropriately

### Scenario 7: Invocation_id validated before actor call
**Given**: An invalid invocation_id (empty string) AND actor communication would fail
**When**: `get_journal(Extension(failing_actor), Path(""))` is called
**Then**:
- Returns `Err(400 BadRequest)` (not 500)
- Actor was never contacted
- Proves P1 is checked before P2

### Scenario 8: Invariant I1 - All entries present
**Given**: A workflow with 5 journal entries
**When**: `get_journal(Extension(master), Path("workflow-with-5-entries"))` is called
**Then**:
- Response contains exactly 5 entries (no omissions)
- All entries belong to the specified invocation_id

### Scenario 9: Invariant I3 - Required fields populated
**Given**: A workflow with entries that have seq, type, timestamp set
**When**: `get_journal(Extension(master), Path("valid-id"))` is called
**Then**:
- Every entry has non-null seq field
- Every entry has non-null type field
- Every entry has non-null timestamp field

### Scenario 10: Q4 violation - Unsorted entries return 200
**Given**: Orchestrator returns entries out of order (seq: [3, 1, 2])
**When**: `get_journal(Extension(master), Path("valid-id"))` is called
**Then**:
- Returns `Ok(200 OK)` (handler must sort, not reject)
- Response entries are sorted ascending by seq: [1, 2, 3]
- This tests that handler enforces Q4 even when source data is unsorted

## Specific Test Cases

### test_get_journal_returns_200_with_entries_when_workflow_exists
```rust
// Mock OrchestratorMsg::GetJournal to return vec![entry1, entry2, entry3]
// Call get_journal with valid invocation_id
// Assert status is 200 OK
// Assert response contains invocation_id and 3 entries
```

### test_get_journal_returns_200_with_empty_entries_when_no_events
```rust
// Mock OrchestratorMsg::GetJournal to return empty vec![]
// Call get_journal
// Assert status is 200 OK
// Assert entries array is empty
```

### test_get_journal_returns_404_when_workflow_not_found
```rust
// Mock OrchestratorMsg::GetJournal to return None or error indicating not found
// Call get_journal with nonexistent invocation_id
// Assert status is 404 Not Found
```

### test_get_journal_returns_400_when_invocation_id_empty
```rust
// Call get_journal with Path("")
// Assert status is 400 Bad Request
// Assert actor was NOT called (verify mock received no messages)
```

### test_get_journal_returns_400_when_invocation_id_whitespace_only
```rust
// Call get_journal with Path("   ")
// Assert status is 400 Bad Request
// Assert actor was NOT called
```

### test_invocation_id_validated_before_actor_call
```rust
// Setup mock actor that tracks if it was called
// Call get_journal with invalid invocation_id
// Assert status is 400
// Assert actor mock received 0 messages
```

### test_invariant_i1_response_contains_all_journal_entries
```rust
// Mock orchestrator to return known 5 entries
// Call get_journal
// Assert response contains exactly 5 entries
// Assert all entry IDs match the returned entries
```

### test_invariant_i3_each_entry_has_required_fields
```rust
// Mock orchestrator to return entries
// Call get_journal
// For each entry, assert seq.is_some(), type.is_some(), timestamp.is_some()
```

### test_q4_violation_entries_unsorted_still_returns_200
```rust
// Mock orchestrator to return entries with seq out of order [5, 2, 4, 1, 3]
// Call get_journal
// Assert status is 200 OK
// Assert returned entries have seq in ascending order: [1, 2, 3, 4, 5]
// This verifies handler sorts entries even when source is unsorted
```

## Exit Criteria
- [ ] All preconditions have corresponding runtime checks
- [ ] All postconditions are verifiable via status codes and response body
- [ ] Error taxonomy maps to HTTP status codes correctly
- [ ] Test names describe behavior unambiguously
- [ ] Every violation example has a matching named test
- [ ] Invariant I1 and I3 have explicit test coverage
- [ ] Validation order (P1 before P2) is verified
