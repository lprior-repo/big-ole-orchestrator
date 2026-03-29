# Test Defects - Bead vo-1xz

## Domain: Consistency
- **Defect**: The contract explicitly mentions a discrepancy between ADR-012 (200 OK) and current implementation (202 Accepted). While it acknowledges this, it doesn't resolve it, leading to ambiguous test expectations.
- **Reference**: Contract.md line 16, 24.

## Domain: Dan North BDD / Dave Farley ATDD
- **Defect**: Scenario 2 in `martin-fowler-tests.md` has a contradiction between its expected status code (404 Not Found) and its expected error code ("signal_failed"). The error taxonomy in `contract.md` (line 33) maps `InstanceNotFound` to 404, but "signal_failed" (line 34) is mapped to 400.
- **Reference**: martin-fowler-tests.md line 62-63 vs Contract.md line 33-34.

## Domain: Dave Farley ATDD (Completeness)
- **Defect**: The error taxonomy lists `ApiError::InvalidPayload` (400) and `ApiError::SignalFailed` (400), but the test plan does not specify the exact `error` string code for these in the JSON response shape.
- **Reference**: Contract.md line 32, 34 vs martin-fowler-tests.md line 17.

## Domain: Testing Trophy (Integration)
- **Defect**: There is no mention of how the `Extension<ActorRef<OrchestratorMsg>>` is initialized or mocked for these tests. Since `vo-engine` relies heavily on Ractor actors, a test plan without a clear strategy for actor isolation or integration (e.g., using a test probe or a mock orchestrator) is incomplete for ATDD.
- **Reference**: Dave Farley's emphasis on executable specifications.

## Domain: Invariants
- **Defect**: The invariant "The system state remains consistent regardless of whether the signal is successfully delivered... (at the API layer)" is too vague to be testable. It doesn't define what "consistent" means in the context of the event log or KV store.
- **Reference**: Contract.md line 28.
