# Implementation Summary: wtf-blc

## Metadata
bead_id: wtf-blc
bead_title: Define HTTP request/response types for API
phase: 3
updated_at: 2026-03-20T00:00:00Z

## Changes Made

### crates/wtf-api/src/types.rs
Implemented all request/response types per ADR-012:

**NewTypes (Data Layer):**
- `WorkflowName` - compile-time validation `[a-z][a-z0-9_]*`
- `SignalName` - compile-time validation `[a-z][a-z0-9_]+`
- `InvocationId` - ULID format validation (26-char Crockford base32)
- `RetryAfterSeconds` - NonZeroU64 wrapper for > 0 validation
- `Timestamp` - RFC3339 format validation

**Error Types (Calculations Layer):**
- `ParseError` - input format errors
- `ValidationError` - business rule violations
- `InvariantViolation` - postcondition failures
- `DomainError` - error domain wrapper

**Request Types:**
- `StartWorkflowRequest { workflow_name: WorkflowName, input: Value }`
- `SignalRequest { signal_name: SignalName, payload: Value }`

**Response Types:**
- `StartWorkflowResponse` with `validate()` method
- `WorkflowStatus` with `validate()` method  
- `SignalResponse { acknowledged: bool }`
- `JournalEntry` and `JournalResponse` with `validate()` method
- `ListWorkflowsResponse { workflows: Vec<WorkflowStatus> }`
- `ErrorResponse` with `new()` validation

**Validation Methods:**
- `WorkflowStatus::validate()` - checks updated_at >= started_at
- `JournalResponse::validate()` - checks entries sorted by seq
- `StartWorkflowResponse::validate()` - checks status == Running
- `ErrorResponse::new()` - validates retry_after consistency

## Tests
78 tests implemented in martin-fowler-tests.md covering:
- 45 unit tests
- 15 contract violation tests
- 12 contract verification tests
- 6 integration tests

## Pre-existing Issues
handlers.rs has stub functions with type inference issues (Err(StatusCode::NOT_IMPLEMENTED) without type annotation). These are being fixed as part of Moon gate.
