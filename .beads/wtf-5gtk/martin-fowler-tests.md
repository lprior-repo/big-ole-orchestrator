# Martin Fowler Tests: Phase 4 — API Layer (wtf-5gtk)

bead_id: wtf-5gtk
bead_title: epic: Phase 4 — API Layer (wtf-api)
phase: 4
updated_at: 2026-03-21T04:12:32Z

---

## Test Strategy

Following Given-When-Then (BDD) style from Martin Fowler. Tests are organized by feature scenarios for the workflow definition ingestion endpoint.

---

## Feature: Workflow Definition Validation Endpoint

### Scenario 1: Valid workflow source passes linting

**Given** a workflow source file with no lint violations
**When** the client sends `POST /api/v1/workflows/validate` with valid source
**Then** the response status is `200 OK` with `valid: true` and empty diagnostics array

### Scenario 2: Invalid workflow source returns lint errors

**Given** a workflow source file containing `std::time::SystemTime::now()`
**When** the client sends `POST /api/v1/workflows/validate`
**Then** the response contains `valid: false` and a diagnostic with code `WTF-L001`

### Scenario 3: Parse error returns 400

**Given** syntactically invalid Rust source code
**When** the client sends `POST /api/v1/workflows/validate`
**Then** the response status is `400 Bad Request` with error code `parse_error`

### Scenario 4: Multiple lint violations returns all diagnostics

**Given** a workflow source with both non-deterministic time AND tokio::spawn
**When** the client sends `POST /api/v1/workflows/validate`
**Then** the response contains diagnostics for both `WTF-L001` and `WTF-L005`

### Scenario 5: Warning severity does not invalidate workflow

**Given** a workflow source with a `WTF-L004` warning (ctx-in-closure)
**When** the client sends `POST /api/v1/workflows/validate`
**Then** the response has `valid: true` with the warning diagnostic

### Scenario 6: Empty source returns parse error

**Given** an empty workflow source string
**When** the client sends `POST /api/v1/workflows/validate`
**Then** the response status is `400 Bad Request` with `parse_error`

### Scenario 7: Non-Rust content returns parse error

**Given** a workflow source containing Python code
**When** the client sends `POST /api/v1/workflows/validate`
**Then** the response status is `400 Bad Request` with `parse_error`

---

## Feature: Endpoint Error Handling

### Scenario 8: Missing request body returns 400

**Given** no request body is sent
**When** the client sends `POST /api/v1/workflows/validate`
**Then** the response status is `400 Bad Request`

### Scenario 9: Malformed JSON returns 400

**Given** the request body is not valid JSON
**When** the client sends `POST /api/v1/workflows/validate`
**Then** the response status is `400 Bad Request`

---

## Feature: Linter Rule Coverage

### Scenario 10: L001 - Non-deterministic time detected

**Given** workflow source calling `SystemTime::now()` or `Instant::now()`
**When** the linter runs
**Then** a `WTF-L001` error diagnostic is produced

### Scenario 11: L002 - Non-deterministic random detected

**Given** workflow source calling `rand::random()` or similar
**When** the linter runs
**Then** a `WTF-L002` error diagnostic is produced

### Scenario 12: L003 - Direct async I/O detected

**Given** workflow source with direct `tokio::fs` or `reqwest` calls
**When** the linter runs
**Then** a `WTF-L003` error diagnostic is produced

### Scenario 13: L004 - ctx-in-closure warning

**Given** workflow source with `ctx.` call inside a closure
**When** the linter runs
**Then** a `WTF-L004` warning diagnostic is produced

### Scenario 14: L005 - tokio::spawn detected

**Given** workflow source with `tokio::spawn`
**When** the linter runs
**Then** a `WTF-L005` error diagnostic is produced

### Scenario 15: L006 - std::thread::spawn detected

**Given** workflow source with `std::thread::spawn`
**When** the linter runs
**Then** a `WTF-L006` error diagnostic is produced

---

## Child Beads to Create

Based on this decomposition, the following child bead should be created:

1. **Workflow Definition Validation Endpoint** (task, priority 1)
   - `POST /api/v1/workflows/validate` handler
   - Integration with `wtf-linter`
   - Unit tests for handler
   - Error mapping

---

## Test Doubles / Mocks

- Mock `wtf-linter` for unit testing the handler
- Use syn's `parse_quote!` for generating valid/invalid test source code

---

## Out-of-Scope for This Epic

- Integration tests with real NATS/Storage (covered by child beads wtf-wdxg, wtf-k0ck)
- Load testing
- OpenAPI spec generation
