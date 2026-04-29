# Bead: tw-kw9 - Add API request logging middleware

## Task
Add an axum middleware layer that logs request method, path, status code, duration, and request ID. Use tracing crate for structured logging. Include request ID in response headers for debugging. Filter out /health from logs to reduce noise.

## Implementation

### Files Created
- `crates/vo-api/src/middleware/logging.rs` - New logging middleware

### Files Modified
- `crates/vo-api/src/middleware/mod.rs` - Added logging module exports
- `crates/vo-api/src/router.rs` - Added request_logging middleware to router

### Implementation Details

#### logging.rs
- Created `request_logging` async middleware function using axum middleware pattern
- Uses ULID to generate unique request IDs per request
- Stores RequestId in request extensions for handlers to access
- Adds `X-Request-ID` header to response for client-side debugging
- Uses tracing crate for structured logging with:
  - method, path, status, request_id, duration_ms fields
  - Log level based on status code (INFO for 2xx, WARN for 4xx, ERROR for 5xx)
- Filters out `/health` and other public paths (delegates to existing `is_public_path`)

#### router.rs
- Added `middleware::from_fn(request_logging)` layer to the main router
- Layer is applied before other layers (TimeoutLayer, CorsLayer, TraceLayer)

#### middleware/mod.rs
- Added `pub mod logging;` to expose the new module
- Re-exports `RequestId`, `request_id_from_extensions`, and `request_logging`

## Key Design Decisions

1. **ULID for request IDs**: Chose ULID over UUID for better sortability and readability
2. **Response header placement**: Added X-Request-ID header after the request is processed (in the response)
3. **Log filtering**: Reused existing `is_public_path()` from auth module to filter health endpoints
4. **Integration with existing TraceLayer**: Kept existing tower-http TraceLayer for HTTP-level tracing; our middleware adds application-level structured logging

## Verification
- `cargo check -p vo-api` compiles successfully
- Clippy warnings in vo-api are pre-existing (in vo-types, vo-storage, vo-core)
- No new clippy warnings introduced by this change
