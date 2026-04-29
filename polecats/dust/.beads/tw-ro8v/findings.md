# Findings: tw-ro8v - Webhook HMAC Signature Validation

## Issue
vo-api: Webhook handler must validate HMAC signature before processing

## Investigation
- Searched for existing webhook endpoint in vo-api handlers - NONE FOUND
- The issue describes what SHOULD exist (a webhook endpoint with HMAC validation)
- No existing webhook handler was found in the codebase

## Implementation

### Files Changed

1. **crates/vo-api/Cargo.toml**
   - Added `hmac = "0.12"` dependency
   - Added `sha2 = "0.10"` dependency
   - Added `hex = "0.4"` dependency

2. **crates/vo-api/src/handlers/webhook.rs** (NEW FILE)
   - Created `WebhookState` struct holding the HMAC secret key
   - Created `verify_webhook_signature` middleware that:
     - Extracts X-Signature header
     - Reads request body
     - Computes HMAC-SHA256 using the secret key
     - Compares signature using constant-time comparison (hmac crate's verify_slice)
     - Returns 401 if signature missing or invalid
   - Created `webhook_handler` endpoint that accepts JSON payload
   - Added unit tests for valid/invalid/missing/malformed signatures

3. **crates/vo-api/src/handlers/mod.rs**
   - Added `pub mod webhook;` and `pub use webhook::*;`

4. **crates/vo-api/src/router.rs**
   - Added WebhookState to AppState struct
   - Added /api/v1/webhook POST route with HMAC verification middleware
   - Updated test AppState to include webhook_state

5. **crates/vo-api/src/handlers/query.rs**
   - Fixed pre-existing bug: made `HistoryQueryParams` public (was `pub(self)`)

### HMAC Verification Flow
1. Client sends POST to /api/v1/webhook with X-Signature header
2. Middleware extracts X-Signature (hex-encoded HMAC-SHA256)
3. Middleware reads request body
4. Middleware computes HMAC-SHA256(secret_key, body)
5. Middleware uses constant-time comparison via hmac::Mac::verify_slice
6. If valid, request proceeds to webhook_handler; otherwise 401 returned

### Tests
Tests added in webhook.rs:
- test_valid_signature: valid HMAC passes
- test_invalid_signature: wrong secret fails
- test_missing_signature: no X-Signature header fails
- test_malformed_signature: non-hex signature fails

## Pre-existing Issues Discovered
1. SSE handler tests have compilation errors (axum API mismatch)
2. HistoryResponse missing fields in several test files
3. HistoryQueryParams was private (pub(self)) but used in public function

## Verification
- `cargo build -p vo-api` succeeds
- Test compilation fails due to PRE-EXISTING bugs in SSE handlers and HistoryResponse
- These pre-existing bugs are unrelated to webhook implementation

## Notes
- The webhook endpoint currently just logs and returns OK - actual workflow triggering was out of scope per issue description
- The HMAC validation is the security fix requested
- The `webhook_secret` appears in privacy tests (vo-types) but no actual storage mechanism exists yet
