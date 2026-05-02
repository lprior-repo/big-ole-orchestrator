# ADR 049 (v2): AI Output Parsing Validation and Retry

## Status
Accepted

## Context
`vo-engine` treats AI agents as first-class citizens (ADR-008). AI agents submit workflow commands via the REST API, sending JSON payloads that are deserialized into `CommandEnvelope` and other typed structures.

However, LLMs are non-deterministic. A single AI agent may produce well-formed, valid JSON in one call and produce truncated, malformed, or schema-violating JSON in the next. When this happens, the current behavior is binary: either the JSON parses successfully and the command proceeds, or it fails with a generic parse error and the AI must reconstruct the entire request from scratch.

This creates two problems:
1. **Wasted compute**: A response that is 95% correct (e.g., a valid JSON object with one field missing or slightly wrong type) is treated the same as one that is entirely garbage.
2. **No structured retry**: The AI has no visibility into *why* its output failed, making it more likely to repeat the same error on retry.

Additionally, the invariant from ADR-008 that "schema migrations must be meticulously managed" applies in reverse: the validation layer must reject non-conforming input *before* it reaches the engine core, preventing malformed data from corrupting state.

## Decision
We implement a three-layer validation pipeline with structured retry feedback:

### 1. The JSON Integrity Layer
Before any semantic parsing occurs, the raw input bytes must be valid JSON. This is a hard boundary.

- If the input is not valid JSON, return `400 Bad Request` with error code `malformed_json`.
- This is a structural check, not a semantic one. The engine does not attempt partial recovery.

### 2. The Schema Validation Layer
After JSON is confirmed valid, the parsed `serde_json::Value` is validated against the expected schema for the target API endpoint.

For `CommandEnvelope` (used in all mutating API calls):
- All required fields must be present: `version`, `command_id`, `correlation_id`, `causation_id`, `issuer`, `issued_at`.
- Each field must be of the correct type (string, integer, etc.).
- `issuer` must be a recognized value: `ai_agent`, `api_client`, `system`, or `cli`.
- `version` must be within `MAX_SUPPORTED_COMMAND_VERSION`.
- `command_id`, `correlation_id`, and `causation_id` must satisfy `IdempotencyKey` constraints.

Missing or invalid fields produce a `400 Bad Request` with an error code and a `details` array listing each specific field error. This structured error response enables the AI to fix the exact fields that failed.

For `V3StartRequest` and other API types:
- Namespace must be non-empty and match the workspace naming conventions.
- Paradigm must be one of `fsm`, `dag`, or `procedural`.
- Workflow type must be non-empty.
- Input must be a valid JSON object (not null, array, or primitive).

### 3. The Retry and Feedback Layer
When the schema validation layer produces field-level errors, the engine returns a structured response that the AI agent can use to self-correct and retry.

**Response format for field-level failures:**
```json
{
  "error": "validation_failed",
  "code": "invalid_command_envelope",
  "message": "Command envelope failed schema validation",
  "details": [
    {
      "field": "issuer",
      "issue": "unknown_value",
      "expected": ["ai_agent", "api_client", "system", "cli"],
      "received": "llm_agent"
    },
    {
      "field": "issued_at",
      "issue": "type_mismatch",
      "expected": "integer (Unix timestamp in ms)",
      "received": "string"
    }
  ]
}
```

**AI agent retry contract:**
- AI agents SHOULD read the `details` array and regenerate only the failing fields.
- AI agents MAY regenerate the entire envelope if preferred.
- The engine enforces the rate limit from ADR-026 (1 new version per workflow per minute) on retries.
- If a specific AI agent consistently fails validation (>5 consecutive malformed submissions), the failure loop circuit breaker (ADR-026) may trigger quarantine.

### 4. The Fallback: Defensive Parsing
For API endpoints that accept `serde_json::Value` as input (e.g., `V3StartRequest.input`), the engine applies default-safe parsing:
- If JSON serialization of the input fails, return `400 Bad Request` with error code `invalid_input`.
- The engine never uses `unwrap()` or `expect()` on AI-supplied data. All fallible operations return `Result<T, Error>`.

## Implementation

### Error type extension
The existing `CommandEnvelopeError` enum (in `crates/vo-types/src/command_envelope.rs`) is extended with field-level error detail:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CommandEnvelopeError {
    // ... existing variants ...
    #[error("Missing envelope field: {0}")]
    MissingEnvelopeField(String),
    #[error("Invalid envelope field: {0}")]
    InvalidEnvelopeField(String),
    #[error("Unknown issuer value: {0}. Expected one of: {1}")]
    UnknownIssuer(String, String), // second field is comma-separated list of valid values
}
```

The helper function `envelope_string()` (line 150 of `command_envelope.rs`) is enhanced to return specific field names in errors rather than generic messages.

### API handler validation
The API handlers in `crates/vo-api/src/handlers/` are updated to use the structured error responses:

**In `workflow_start.rs` (line 47-48):**
Replace the current `unwrap_or_default()` pattern:
```rust
// BEFORE: swallows parse errors
let json_str = serde_json::to_string(env_json).unwrap_or_default();
match CommandEnvelope::from_str(&json_str) { ... }

// AFTER: structured validation
let validated = match validate_command_envelope(env_json) {
    Ok(env) => env,
    Err(e) => return validation_error_response(e),
};
```

A new `validate_command_envelope()` function in `crates/vo-api/src/types/validation.rs` performs the pre-parsing JSON structure check, then delegates to `CommandEnvelope::from_str()` for semantic validation.

**In `ingress.rs`:**
Similar treatment for ingress event parsing — structured errors instead of binary success/fail.

### AI agent guidance
The `vo-cli` history command (ADR-008) already provides AI agents with redacted operator projections. We extend this to include validation failure examples:

```bash
vo-cli validate-examples --type command_envelope
```

This prints the expected JSON schema with examples of valid and invalid values, helping AI agents learn the contract over time.

## Consequences

### Positive
- **Partial recovery**: Responses that are 95% correct get 95% credit — the AI only needs to fix the failing fields, not regenerate everything.
- **Structured feedback**: Field-level error details eliminate the "black box" problem where the AI submits malformed JSON and receives a generic "bad request" with no actionable information.
- **Security invariant maintained**: Invalid output never propagates into the engine core. The validation layer is a strict gate.
- **Self-improving AI agents**: Over time, AI agents trained on the structured error responses should produce fewer malformed submissions.

### Negative
- **More API surface**: The new structured error response adds fields (`details`) that clients must handle.
- **Parsing latency**: Schema validation adds a small amount of CPU overhead on every API call (measured in microseconds for typical payloads).
- **AI dependency**: The retry mechanism only works if the AI agent reads and acts on the error details. Poorly configured agents may still loop on the same errors until the circuit breaker trips.

### Negative (mitigated)
- **Rate limit interactions**: Aggressive retry loops could hit the ADR-026 rate limit. Mitigated by the circuit breaker already being in place.
- **Schema versioning**: If the expected schema changes (e.g., new required field), existing AI agents may produce invalid output. Mitigated by ADR-035's upcaster pattern and backward-compatible envelope versions.

## Related ADRs
- **ADR-008** (AI-Native Agent Interfaces): Defines the deterministic JSON boundaries that this ADR enforces on the input side.
- **ADR-026** (AI Feedback Loop Poisoning): The circuit breaker that handles persistent AI failures. This ADR feeds into ADR-026's quarantine decision with structured data.
- **ADR-035** (Event Schema Evolution): Schema changes must be backward-compatible to avoid breaking AI agents' output.
