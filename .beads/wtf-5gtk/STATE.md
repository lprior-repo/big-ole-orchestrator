# STATE: 7 - ARCHITECTURAL DRIFT CHECK COMPLETE

## Timestamp: 2026-03-22T00:00:00Z

## Status: PERFECT

## QA Report Summary

- `cargo check -p wtf-api --lib`: ✓ FINISHED
- `cargo test -p wtf-api --lib`: ✓ 30 passed; 0 failed
- QA Report: `qa-report.md` written
- QA Review: `qa-review.md` written - APPROVED
- Red Queen Report: `red-queen-report.md` written - PASS
- Black Hat Review: `black-hat-review.md` written - ADVISORY (non-blocking)
- Kani Justification: `kani-justification.md` written - SKIPPED (no unsafe code)
- Architectural Drift: `architectural-drift.md` written - PERFECT (257 lines < 300)

## Issue Fixed

Axum routing returns 404 for malformed URLs (e.g., `/api/v1/workflows//journal`) BEFORE the handler runs. The contract has been updated to reflect reality:

### Contract Changes

1. **Routing-layer rejections (404)**: Invalid path structure (double-slash, missing segments) now correctly documented as 404
2. **Handler-layer rejections (400)**: Empty/whitespace ID, missing namespace separator - still 400 but only if routing passes
3. **Instance not found (404)**: Valid format but non-existent instance in event store - 404 handler-layer

### Updated Files

- `contract.md`: Added HTTP Status Code Decision Tree, Routing-Layer vs Handler-Layer distinction, updated Error Taxonomy table
- `martin-fowler-tests.md`: Added new section 4 "Routing-Layer Rejection Tests", renumbered remaining sections

### Error Taxonomy (Updated)

| Error Code | HTTP Status | Layer | Condition |
|------------|-------------|-------|-----------|
| `invalid_id` | 400 | Handler | Empty or whitespace instance ID |
| `not_found` | 404 | Handler | Instance ID not found in event store |
| `route_not_found` | 404 | Routing | Invalid path structure (double-slash, missing segments) |
| `method_not_allowed` | 405 | Routing | Wrong HTTP method |
| `actor_error` | 500 | Handler | Event store unavailable |
| `journal_error` | 500 | Handler | Error during journal replay |
