# Findings: tw-ro8v - Webhook HMAC Signature Validation

## Issue
vo-api: Webhook handler must validate HMAC signature before processing

## Implementation Summary

### Files Changed
1. **Cargo.toml (workspace)** - Added `hmac = "0.12"` and `hex = "0.4"` dependencies
2. **crates/vo-api/Cargo.toml** - Added `hmac`, `sha2`, and `hex` dependencies
3. **crates/vo-api/src/handlers/webhook.rs** - New file with HMAC validation
4. **crates/vo-api/src/handlers/mod.rs** - Added webhook module export
5. **crates/vo-api/src/router.rs** - Added webhook route and WebhookState to AppState
6. **crates/vo-api/src/handlers/query.rs** - Fixed pre-existing bug: made `HistoryQueryParams` public

### Implementation Details

#### webhook.rs
- `WebhookState` struct holds the secret key
- `verify_hmac_signature()` function:
  - Takes secret, body bytes, and signature header
  - Strips `sha256=` prefix from signature
  - Decodes hex signature
  - Computes HMAC-SHA256 using `hmac` crate
  - Uses constant-time comparison via `mac.verify_slice()`
  - Returns `WebhookError` on failure
- `webhook_handler()` async function:
  - Extracts `X-Signature` header
  - Reads raw body using `axum::body::to_bytes()`
  - Validates HMAC signature
  - Parses body as `V3StartRequest` (validates JSON structure)
  - Returns 200 OK with `{"status": "received"}` on success
  - Returns 401 Unauthorized on signature failure
  - Returns 400 Bad Request on body parse failure

#### Route Added
- `POST /api/v1/webhook` - Webhook endpoint with HMAC validation

#### Tests
- `test_valid_signature` - Valid HMAC passes
- `test_invalid_signature_wrong_secret` - Wrong secret fails
- `test_invalid_signature_tampered_body` - Tampered body fails
- `test_invalid_signature_format_not_hex` - Invalid hex fails
- `test_invalid_signature_missing_prefix` - Missing sha256= prefix fails
- `test_missing_signature_header_value` - Empty signature fails

## Security
- HMAC-SHA256 with constant-time comparison prevents timing attacks
- Signature format validation before comparison
- 401 returned for missing/invalid signatures

## Pre-existing Issues Fixed
- `HistoryQueryParams` in query.rs was private but used in public function - made it public

## Pre-existing Issues (Not Fixed)
- `HistoryQueryParams` visibility warning still exists in query.rs
- sse.rs has compilation errors with `axum::response::sse::Event::data()` API changes
- v3_test.rs missing fields in V3StartRequest initializer
- These are separate issues not related to webhook implementation