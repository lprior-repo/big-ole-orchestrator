# Test Review: wtf-blc

## Metadata
bead_id: wtf-blc, bead_title: Define HTTP request/response types for API, phase: 2, updated_at: 2026-03-20T00:00:00Z

## Status: REJECTED

## Defects Found

### 1. Missing Compile-Time Type Enforcement (CRITICAL)

| Precondition | Contract Says | Defect |
|---|---|---|
| P3: signal_name non-empty | Compile-time via NewType | Implementation uses `String`, NOT `SignalName` newtype |
| P6: retry_after_seconds > 0 | Runtime (contract says runtime) | Should be `NonZeroU64` for compile-time enforcement |

**Contract specifies compile-time enforcement but actual types use runtime validation.**

### 2. Missing Test Coverage for Violation Examples

| Violation Example | Has Test? |
|---|---|
| VIOLATES P4: status "unknown" serialization fails | NO |
| VIOLATES Q1: StartWorkflowResponse invalid invocation_id | NO |
| VIOLATES Q2: status != "running" on success | NO |
| VIOLATES Q4: current_step != 0 when status -> running | NO |
| VIOLATES Q6: acknowledged != true | NO |
| VIOLATES P5: (no violation defined - u32 is unsigned so can't fail) | N/A |
| VIOLATES I2: invocation_id mutation | NO |
| VIOLATES I4: invalid JSON | NO |

### 3. Testing Trophy Violation

- **Kent Beck Testing Trophy**: Integration tests > unit tests
- **Defect**: 0 actual integration tests exist
- All tests are unit-level (serde serialization/deserialization)
- Given-When-Then scenarios (1-6) are API-level but are NOT executable tests - they are descriptive scenarios only

### 4. BDD Format Inconsistency

- Contract Violation Tests use proper Given-When-Then
- Happy Path Tests and Error Path Tests use bullet points WITHOUT Given-When-Then format
- Dan North BDD requires consistent GWT format across ALL tests

### 5. Postconditions Not Directly Verifiable

Postconditions Q3, Q5, Q7 require constructor/validation methods that:
- Are NOT defined in Contract Signatures
- Are NOT testable without implementation

**Example**: Q5 requires `JournalResponse::validate()` but this method doesn't exist in the type definitions.

### 6. Invariant Tests Missing

| Invariant | Has Test? |
|---|---|
| I1: RFC3339 timestamps | Partial (timestamp tests exist but don't enforce RFC3339 on construction) |
| I2: invocation_id immutable | NO |
| I3: journal append-only | NO |
| I4: valid JSON serialization | Partial (roundtrip exists but doesn't verify all edge cases) |

### 7. Error Taxonomy Gaps

Contract lists 8 error variants but tests reference:
- `ParseError::EmptyWorkflowName`
- `ParseError::InvalidWorkflowNameFormat`
- `ParseError::InvalidUlidFormat`
- `ParseError::EmptySignalName`
- `ValidationError::InvalidRetryAfterSeconds`
- `InvariantViolation::*`

**Missing from Error Taxonomy**:
- `ParseError` variants (prefixed under `InvalidInput`?)
- `ValidationError` 
- `InvariantViolation`

### 8. P6 Violation Test Missing

Contract defines P6 (`retry_after_seconds > 0`) but:
- No violation example provided
- No corresponding test in martin-fowler-tests.md

## Summary

The test plan has **structural coverage** but fails on:
1. **Violates contract-test parity** - 6 violation examples have no tests
2. **Not executable** - Given-When-Then scenarios are descriptive only, not actual test code
3. **Type system gaps** - Preconditions P3 and P6 not properly typed at compile-time as specified
4. **Integration test gap** - No HTTP-layer integration tests (Testing Trophy)
5. **BDD inconsistency** - Only partial use of Given-When-Then format
