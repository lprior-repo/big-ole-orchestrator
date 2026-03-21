# Contract Specification

## Metadata
- bead_id: wtf-blc
- bead_title: Define HTTP request/response types for API
- phase: 1
- updated_at: 2026-03-20T00:00:00Z

## Context

- **Feature**: Define HTTP request/response types for wtf-engine REST API
- **Domain terms**: 
  - `invocation_id` - Unique identifier for a workflow execution instance (ULID)
  - `workflow_name` - Name of the workflow definition
  - `input` - JSON payload for workflow initialization
  - `signal` - Asynchronous message to modify workflow state
  - `journal` - Immutable sequence of workflow execution events
  - `current_step` - Index of the currently executing step (0-indexed)
- **Source ADR**: ADR-012 (API Design), ADR-010 (Error Handling Taxonomy)
- **Location**: `wtf-api/src/types.rs`
- **Assumptions**:
  - All timestamps are RFC3339 format
  - invocation_id uses ULID format (26 chars, Crockford base32)
  - All JSON values use serde_json::Value for flexibility
  - Status values are lowercase strings from defined enum

## Preconditions

1. **P1**: `workflow_name` must be non-empty ASCII string matching `[a-z][a-z0-9_]*`
2. **P2**: `invocation_id` must be valid ULID format (26 chars, alphanumeric)
3. **P3**: `signal_name` must be non-empty ASCII string matching `[a-z][a-z0-9_]+`
4. **P4**: `status` value must be one of: "pending", "running", "completed", "failed", "cancelled"
5. **P5**: `current_step` must be >= 0 when status is "running"
6. **P6**: `retry_after_seconds` must be > 0 when present

## Postconditions

1. **Q1**: `StartWorkflowResponse` contains valid invocation_id (26-char ULID, Crockford base32)
2. **Q2**: `StartWorkflowResponse.status` is "running" on success
3. **Q3**: `WorkflowStatus.updated_at` >= `WorkflowStatus.started_at`
4. **Q4**: `WorkflowStatus.current_step` is 0 when status transitions to "running"
5. **Q5**: `JournalResponse.entries` are sorted by `seq` ascending
6. **Q6**: `SignalResponse.acknowledged` is `true` on successful signal delivery
7. **Q7**: `ErrorResponse.retry_after_seconds` is `None` for non-retryable errors

## Invariants

1. **I1**: All response types with timestamps must have valid RFC3339 format
2. **I2**: invocation_id is immutable once assigned (no setters, no modifications post-construction)
3. **I3**: Journal entries are append-only (seq only increases)
4. **I4**: All types must serialize to valid JSON and deserialize back to equivalent values

## Error Taxonomy

### ParseError (Invalid Input Format)
| Variant | Description |
|---|---|
| `ParseError::EmptyWorkflowName` | workflow_name is empty string |
| `ParseError::InvalidWorkflowNameFormat` | workflow_name does not match `[a-z][a-z0-9_]*` |
| `ParseError::EmptySignalName` | signal_name is empty string |
| `ParseError::InvalidSignalNameFormat` | signal_name does not match `[a-z][a-z0-9_]+` |
| `ParseError::InvalidUlidFormat` | invocation_id is not valid 26-char Crockford base32 |
| `ParseError::InvalidTimestampFormat` | timestamp is not valid RFC3339 |

### ValidationError (Business Rule Violation)
| Variant | Description |
|---|---|
| `ValidationError::InvalidRetryAfterSeconds` | retry_after_seconds is 0 or None for required error types |
| `ValidationError::InvalidStatusTransition` | attempted transition not allowed (e.g., running -> pending) |
| `ValidationError::InvalidCurrentStep` | current_step is inconsistent with status |

### InvariantViolation (Postcondition Failure)
| Variant | Description |
|---|---|
| `InvariantViolation::UpdatedBeforeStarted` | updated_at timestamp precedes started_at |
| `InvariantViolation::EntriesNotSorted` | Journal entries not in ascending seq order |
| `InvariantViolation::InvalidRetryForErrorType` | retry_after_seconds set for non-retryable error |
| `InvariantViolation::InvocationIdModified` | attempt to modify immutable invocation_id |

### DomainError (Operational Failures)
From ADR-010, mapped to HTTP responses:

| Error Variant | HTTP Status | retry_after_seconds |
|---|---|---|
| `DomainError::WorkflowNotFound` | 404 | None |
| `DomainError::InvocationNotFound` | 404 | None |
| `DomainError::InvalidInput` | 400 | None |
| `DomainError::AtCapacity` | 409 | Some(5) |
| `DomainError::InvalidTransition` | 409 | None |
| `DomainError::ActivityFailed` | 500 | None |
| `DomainError::Storage` | 500 | None |
| `DomainError::Serialization` | 500 | None |

## Contract Signatures

```rust
use std::num::NonZeroU64;

// NewType: Compile-time enforcement for P1, P3
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct WorkflowName(String);

impl WorkflowName {
    pub fn new(s: impl AsRef<str>) -> Result<Self, ParseError> {
        let s = s.as_ref();
        if s.is_empty() {
            return Err(ParseError::EmptyWorkflowName);
        }
        if !Regex::new(r"^[a-z][a-z0-9_]*$").unwrap().is_match(s) {
            return Err(ParseError::InvalidWorkflowNameFormat);
        }
        Ok(Self(s.to_string()))
    }
    pub fn as_str(&self) -> &str { &self.0 }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SignalName(String);

impl SignalName {
    pub fn new(s: impl AsRef<str>) -> Result<Self, ParseError> {
        let s = s.as_ref();
        if s.is_empty() {
            return Err(ParseError::EmptySignalName);
        }
        if !Regex::new(r"^[a-z][a-z0-9_]+$").unwrap().is_match(s) {
            return Err(ParseError::InvalidSignalNameFormat);
        }
        Ok(Self(s.to_string()))
    }
    pub fn as_str(&self) -> &str { &self.0 }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvocationId(String);

impl InvocationId {
    pub fn from_str(s: impl AsRef<str>) -> Result<Self, ParseError> {
        let s = s.as_ref();
        if s.len() != 26 {
            return Err(ParseError::InvalidUlidFormat);
        }
        if !ULID_REGEX.is_match(s) {
            return Err(ParseError::InvalidUlidFormat);
        }
        Ok(Self(s.to_string()))
    }
    pub fn as_str(&self) -> &str { &self.0 }
}

// NewType: Compile-time enforcement for P6
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetryAfterSeconds(NonZeroU64);

impl RetryAfterSeconds {
    pub fn new(seconds: u64) -> Result<Self, ValidationError> {
        NonZeroU64::new(seconds)
            .map(Self)
            .ok_or(ValidationError::InvalidRetryAfterSeconds)
    }
    pub fn get(&self) -> u64 { self.0.get() }
}

// Request types - deserialized from HTTP body
#[derive(Deserialize)]
pub struct StartWorkflowRequest {
    pub workflow_name: WorkflowName,
    pub input: serde_json::Value,
}

#[derive(Deserialize)]
pub struct SignalRequest {
    pub signal_name: SignalName,
    pub payload: serde_json::Value,
}

// Response types - serialized to HTTP body
#[derive(Serialize)]
pub struct StartWorkflowResponse {
    pub invocation_id: InvocationId,
    pub workflow_name: String,
    pub status: WorkflowStatusValue,
    pub started_at: Timestamp,
}

impl StartWorkflowResponse {
    pub fn validate(&self) -> Result<(), InvariantViolation> {
        if self.status != WorkflowStatusValue::Running {
            return Err(InvariantViolation::InvalidStatusForResponse);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkflowStatusValue {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Serialize)]
pub struct WorkflowStatus {
    pub invocation_id: InvocationId,
    pub workflow_name: String,
    pub status: WorkflowStatusValue,
    pub current_step: u32,
    pub started_at: Timestamp,
    pub updated_at: Timestamp,
}

impl WorkflowStatus {
    pub fn validate(&self) -> Result<(), InvariantViolation> {
        if self.updated_at.as_datetime() < self.started_at.as_datetime() {
            return Err(InvariantViolation::UpdatedBeforeStarted);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Timestamp(String);

impl Timestamp {
    pub fn new(s: impl AsRef<str>) -> Result<Self, ParseError> {
        let s = s.as_ref();
        chrono::DateTime::parse_from_rfc3339(s)
            .map_err(|_| ParseError::InvalidTimestampFormat)?;
        Ok(Self(s.to_string()))
    }
    pub fn as_datetime(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339(&self.0)
            .unwrap()
            .with_timezone(&chrono::Utc)
    }
}

#[derive(Serialize)]
pub struct SignalResponse {
    pub acknowledged: bool,
}

#[derive(Serialize)]
pub struct JournalEntry {
    pub seq: u32,
    #[serde(flatten)]
    pub entry_type: JournalEntryType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fire_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

#[derive(Serialize)]
#[serde(tag = "type")]
pub enum JournalEntryType {
    Run,
    Wait,
}

#[derive(Serialize)]
pub struct JournalResponse {
    pub invocation_id: InvocationId,
    pub entries: Vec<JournalEntry>,
}

impl JournalResponse {
    pub fn validate(&self) -> Result<(), InvariantViolation> {
        if !is_sorted(self.entries.iter().map(|e| e.seq)) {
            return Err(InvariantViolation::EntriesNotSorted);
        }
        Ok(())
    }
}

#[derive(Serialize)]
pub struct ListWorkflowsResponse {
    pub workflows: Vec<WorkflowStatus>,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_seconds: Option<RetryAfterSeconds>,
}

impl ErrorResponse {
    pub fn new(
        error: impl Into<String>,
        message: impl Into<String>,
        retry_after: Option<RetryAfterSeconds>,
    ) -> Result<Self, InvariantViolation> {
        let error_str = error.into();
        if is_retryable_error(&error_str) && retry_after.is_none() {
            return Err(InvariantViolation::InvalidRetryForErrorType);
        }
        if !is_retryable_error(&error_str) && retry_after.is_some() {
            return Err(InvariantViolation::InvalidRetryForErrorType);
        }
        Ok(Self {
            error: error_str,
            message: message.into(),
            retry_after_seconds: retry_after,
        })
    }
}
```

## Type Encoding

| Precondition | Enforcement Level | Type / Pattern |
|---|---|---|
| P1: workflow_name non-empty, valid pattern | **Compile-time via NewType** | `WorkflowName::new() -> Result<Self, ParseError>` |
| P2: invocation_id valid ULID | **Compile-time via NewType** | `InvocationId::from_str() -> Result<Self, ParseError>` |
| P3: signal_name non-empty, valid pattern | **Compile-time via NewType** | `SignalName::new() -> Result<Self, ParseError>` |
| P4: status valid enum | **Compile-time** | `enum WorkflowStatusValue { ... }` with Serialize |
| P5: current_step >= 0 | **Compile-time** | `u32` is unsigned (0 is minimum) |
| P6: retry_after_seconds > 0 | **Compile-time via NewType** | `RetryAfterSeconds::new() -> Result<Self, ValidationError>` |

## Violation Examples

### Precondition Violations
| ID | Input | Expected Error |
|---|---|---|
| VIOLATES P1 | `StartWorkflowRequest { workflow_name: "", input: {} }` | `Err(ParseError::EmptyWorkflowName)` |
| VIOLATES P1 | `StartWorkflowRequest { workflow_name: "Invalid-Name", input: {} }` | `Err(ParseError::InvalidWorkflowNameFormat)` |
| VIOLATES P2 | `WorkflowStatus { invocation_id: "x", ... }` | `Err(ParseError::InvalidUlidFormat)` |
| VIOLATES P3 | `SignalRequest { signal_name: "", payload: {} }` | `Err(ParseError::EmptySignalName)` |
| VIOLATES P3 | `SignalRequest { signal_name: "Invalid-Name", payload: {} }` | `Err(ParseError::InvalidSignalNameFormat)` |
| VIOLATES P4 | `WorkflowStatus { status: "unknown", ... }` | `Err(ParseError::UnknownStatusVariant)` |
| VIOLATES P6 | `ErrorResponse { retry_after_seconds: Some(RetryAfterSeconds::new(0)?) }` | `Err(ValidationError::InvalidRetryAfterSeconds)` |

### Postcondition Violations
| ID | Input | Expected Error |
|---|---|---|
| VIOLATES Q1 | `StartWorkflowResponse { invocation_id: InvocationId::from_str("x")?, ... }` | `Err(ParseError::InvalidUlidFormat)` - caught at construction |
| VIOLATES Q2 | `StartWorkflowResponse { status: WorkflowStatusValue::Completed, ... }` | `Err(InvariantViolation::InvalidStatusForResponse)` via `validate()` |
| VIOLATES Q3 | `WorkflowStatus { started_at: "2024-01-15T10:31:00Z", updated_at: "2024-01-15T10:30:00Z" }` | `Err(InvariantViolation::UpdatedBeforeStarted)` via `validate()` |
| VIOLATES Q4 | `WorkflowStatus { status: Running, current_step: 5, ... }` | `Err(ValidationError::InvalidCurrentStep)` via `validate()` |
| VIOLATES Q5 | `JournalResponse { entries: [seq=1, seq=0] }` | `Err(InvariantViolation::EntriesNotSorted)` via `validate()` |
| VIOLATES Q6 | `SignalResponse { acknowledged: false }` for success path | Runtime assertion failure - success always sets true |
| VIOLATES Q7 | `ErrorResponse { error: "not_found", retry_after_seconds: Some(5s) }` | `Err(InvariantViolation::InvalidRetryForErrorType)` via `new()` |

### Invariant Violations
| ID | Input | Expected Error |
|---|---|---|
| VIOLATES I1 | `Timestamp::new("invalid")?` | `Err(ParseError::InvalidTimestampFormat)` |
| VIOLATES I2 | `invocation_id.set("new")?` (if setter existed) | N/A - no setter exists by design |
| VIOLATES I3 | `JournalResponse { entries: [seq=1, seq=0] }` | `Err(InvariantViolation::EntriesNotSorted)` via `validate()` |
| VIOLATES I4 | Type that fails JSON roundtrip | Runtime assertion in integration test |

## Ownership Contracts

- All types are pure data (Data segment of functional architecture)
- No `&mut self` methods on request/response types
- `serde_json::Value` for input/payload uses shared ownership (Cow-like)
- Clone is available on all types (derives Serialize + Clone + Debug)
- No interior mutability (no Mutex/Rc in type definitions)
- NewTypes enforce invariants at construction time

## Non-goals

- No validation of workflow_name against registered workflows (handled by API handlers)
- No validation of signal_name against workflow's allowed signals (handled by orchestrator)
- No actual workflow execution logic
- No database persistence logic

(End of file - total 295 lines)
