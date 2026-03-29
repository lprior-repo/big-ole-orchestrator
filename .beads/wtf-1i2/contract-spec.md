# Contract Specification: create_routes

## Context

- **Bead ID:** vo-1i2
- **Title:** Implement create_routes function with all middleware layers
- **Location:** `vo-api/src/routes.rs`
- **Feature:** Axum HTTP router construction with middleware composition per ADR-012

### Domain Terms

| Term | Definition |
|------|------------|
| `Router` | Axum router that matches HTTP requests to handlers |
| `ActorRef<OrchestratorMsg>` | Smart pointer to the Orchestrator actor |
| `TraceLayer` | Tower middleware for HTTP request tracing |
| `Extension` | Axum middleware for injecting values into request context |
| `JsonBodyLayer` | **NEW tower Layer type to be created by this bead** - extracts JSON body into typed `JsonBody<T>` request extension |
| `JsonBody<T>` | **NEW custom Extractor type to be created by this bead** - validates and extracts JSON from request body |
| `OrchestratorMsg` | Message enum for orchestrator actor communication |
| `KvStores` | Key-value storage handle (sled::Db wrapper) |

### Assumptions

1. `JsonBodyLayer` and `JsonBody<T>` are NEW types to be created as part of this bead - they do not exist in the codebase
2. `TraceLayer::new()` is the correct constructor (not `TraceLayer::new_for_http()`)
3. The `master` ActorRef is the sole state dependency (kv is handled separately in app)
4. **CORRECTED:** The `/health` route DOES require `Extension(master)` injection for actor introspection

### Open Questions

1. ~~**JsonBodyLayer implementation**: The bead spec references `JsonBodyLayer::<JsonBody>::new()` but no such type exists in the codebase.~~ **RESOLVED:** This is a NEW type to be created by this bead
2. **Route prefix**: The bead spec shows routes starting with `/api/v1/` prefix. Should this be a nested router or flat routes?
3. **Metric endpoint**: The current `app.rs` has a `/metrics` endpoint. Should `create_routes` include it?

---

## Preconditions

- [ ] `master` ActorRef must be provided to `create_routes` - router construction itself succeeds regardless of actor lifecycle state
- [ ] The `OrchestratorMsg` message type must be capable of handling `GetEventStore`, `GetStatus`, `StartWorkflow`, `Terminate`, `Signal`, and `ListActive` variants
- [ ] If `TraceLayer::new()` fails to initialize, the resulting Router must still be functional (tracing is non-critical)
- [ ] If `JsonBodyLayer` initialization fails, the Router must still be functional (body parsing errors are runtime, not init-time)

---

## Postconditions

- [ ] Returned `Router` contains exactly 5 route registrations:
  - `GET /health` → `health_handler`
  - `POST /api/v1/workflows` + `GET /api/v1/workflows` → `start_workflow`, `list_workflows`
  - `GET /api/v1/workflows/:id` + `DELETE /api/v1/workflows/:id` → `get_workflow`, `terminate_workflow`
  - `POST /api/v1/workflows/:id/signals` → `send_signal`
  - `GET /api/v1/workflows/:id/journal` → `get_journal`
- [ ] Middleware layers are applied in order: `TraceLayer` outermost, then `Extension(master)`, then `JsonBodyLayer` innermost
- [ ] All routes under `/api/v1/` have access to `Extension(master)`
- [ ] `GET /health` has access to `Extension(master)` (health check may need actor introspection)
- [ ] The Router is cloneable and can be served by `axum::serve`
- [ ] No routes overlap or conflict (each HTTP method + path combination is unique)

---

## Invariants

- [ ] **Route completeness**: All 7 handlers referenced in routes exist and are public exports from `handlers` module
- [ ] **Handler signature consistency**: All handlers accept `Extension<ActorRef<OrchestratorMsg>>` as first extractor
- [ ] **Middleware non-interference**: Each layer operates independently; no layer panics or aborts on valid inputs
- [ ] **Router idempotency**: Calling `create_routes(master)` multiple times with same `master` produces semantically equivalent Routers (cloned routes)
- [ ] **Actor lifecycle**: The returned Router holds a clone of `master`; original ActorRef's lifetime is independent
- [ ] **JsonBodyLayer is constructible**: `JsonBodyLayer::<JsonBody>::new()` must be callable and return a valid Layer

---

## Error Taxonomy

### Scope Clarification

**IMPORTANT:** `create_routes` is a **route construction function only**. It wires up the Router with handlers but does NOT process requests itself. Error variants listed below are those that can be **indirectly tested** through `create_routes` by sending HTTP requests to the constructed Router and observing handler responses.

**Downstream errors** (parsing, validation in handler logic) are **OUT OF SCOPE** for this bead - they occur in handler implementations that belong to separate beads.

### Construction-Time Errors (None Expected)

`create_routes` is a pure constructor; it should not fail at construction time. All runtime errors manifest as HTTP error responses from handlers.

### Runtime Errors - IN SCOPE for create_routes (Testable via Router)

| Error Variant | HTTP Status | Semantic Meaning | Retryable | Concrete Response Fields |
|--------------|-------------|------------------|-----------|-------------------------|
| `ParseError::EmptyWorkflowName` | 400 Bad Request | Workflow name is empty | No | `{"error":"empty_workflow_name","message":"..."}` |
| `ParseError::InvalidWorkflowNameFormat` | 400 Bad Request | Name doesn't match `[a-z][a-z0-9_]*` | No | `{"error":"invalid_workflow_name","message":"..."}` |
| `ParseError::InvalidSignalNameFormat` | 400 Bad Request | Signal name doesn't match `[a-z][a-z0-9]+` | No | `{"error":"invalid_signal_name","message":"..."}` |
| `ParseError::InvalidUlidFormat` | 400 Bad Request | ID is not valid 26-char Crockford base32 | No | `{"error":"invalid_ulid_format","message":"..."}` |
| `ValidationError::InvalidRetryAfterSeconds` | 500 Internal Error | Retry-After value is ≤ 0 | No | `{"error":"internal_error","message":"..."}` |
| `ValidationError::InvalidStatusTransition` | 409 Conflict | Workflow state machine rejects transition | No | `{"error":"invalid_status_transition","message":"..."}` |
| `StartError::AtCapacity { running, max }` | 503 Service Unavailable | Workflow engine at capacity | Yes (5s) | `{"error":"at_capacity","message":"..."}` |
| `StartError::AlreadyExists(id)` | 409 Conflict | Instance ID already in use | No | `{"error":"instance_already_exists","message":"..."}` |
| `StartError::SpawnFailed(msg)` | 500 Internal Server Error | Actor spawn failed | No | `{"error":"spawn_failed","message":"..."}` |
| `StartError::PersistenceFailed(msg)` | 503 Service Unavailable | Metadata persistence failed | Yes (5s) | `{"error":"persistence_failed","message":"..."}` |
| `GetStatusError::Timeout` | 503 Service Unavailable | Instance actor timed out | Yes (5s) | `{"error":"timeout","message":"..."}` |
| `GetStatusError::ActorDied` | 404 Not Found | Instance actor is dead | No | `{"error":"instance_dead","message":"..."}` |
| `TerminateError::NotFound(id)` | 404 Not Found | Instance not found | No | `{"error":"instance_not_found","message":"..."}` |
| `TerminateError::Timeout(id)` | 503 Service Unavailable | Cancel timed out | Yes (5s) | `{"error":"terminate_timeout","message":"..."}` |
| `MessagingErr::ChannelClosed` | 503 Service Unavailable | Orchestrator channel closed | Yes (5s) | `{"error":"channel_closed","message":"..."}` |
| `MessagingErr::InvalidActorType` | 500 Internal Server Error | Actor type mismatch | No | `{"error":"invalid_actor_type","message":"..."}` |

### Runtime Errors - OUT OF SCOPE for create_routes

These error variants exist in the codebase but occur in **downstream parsing/validation logic** that belongs to other beads:

| Error Variant | Reason Out of Scope |
|--------------|---------------------|
| `ParseError::EmptySignalName` | Parsing happens in `SignalName` newtype constructor, not in `create_routes`. Handler implementation bead tests this. |
| `ParseError::InvalidTimestampFormat` | Parsing happens in `Timestamp` newtype constructor for journal/time fields. Not triggered by any current handler route. |
| `ParseError::UnknownStatusVariant` | Defined but NOT YET USED in any handler. Future parsing bead will cover this. |
| `ValidationError::InvalidCurrentStep` | Defined but NOT YET USED in any handler. Future validation bead will cover this. |

**Rationale:** `create_routes` only constructs the Router. It has no knowledge of parsing logic. Testing `EmptySignalName` would require sending a signal request with an empty signal name, which exercises the handler's parsing, not route construction.

---

## Contract Signatures

```rust
// Primary constructor - pure, infallible
pub fn create_routes(master: ActorRef<OrchestratorMsg>) -> Router

// JsonBodyLayer - NEW type to be implemented by this bead
pub struct JsonBodyLayer;
impl JsonBodyLayer {
    pub fn new() -> Self;
}
impl<S> Layer<S> for JsonBodyLayer {
    type Service = JsonBodyLayerService<S>;
}

// JsonBody Extractor - NEW type to be implemented by this bead
pub struct JsonBody<T>(pub T);
impl<T: de::Deserialize<'static>> FromRequestParts<S> for JsonBody<T> { ... }

// Handler signatures (from handlers module)
pub async fn health_handler() -> impl IntoResponse

pub async fn start_workflow(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
    Json(req): Json<V3StartRequest>,
) -> impl IntoResponse

pub async fn list_workflows(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
) -> impl IntoResponse

pub async fn get_workflow(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
    Path(id): Path<String>,
) -> impl IntoResponse

pub async fn terminate_workflow(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
    Path(id): Path<String>,
) -> impl IntoResponse

pub async fn send_signal(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
    Path(id): Path<String>,
    Json(req): Json<V3SignalRequest>,
) -> impl IntoResponse

pub async fn get_journal(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
    Path(id): Path<String>,
) -> impl IntoResponse
```

---

## Non-Goals

- [ ] ~~Implementing `JsonBodyLayer` / `JsonBody` types~~ **NOW IN SCOPE** - these must be created as part of this bead
- [ ] SSE endpoints (`/watch`, `/watch/:namespace`) - handled in separate routes via `app.rs`
- [ ] `/metrics` endpoint - handled in `app.rs`
- [ ] `/definitions/:type` endpoint - handled in `app.rs`
- [ ] `/instances/:id/replay-to/:seq` endpoint - handled in `app.rs`
- [ ] Request rate limiting
- [ ] Authentication/authorization
- [ ] Handler implementation error testing (belongs to handler implementation beads)

---

## Dependencies

| Crate | Purpose |
|-------|---------|
| `axum` | Web framework, Router, routing |
| `tower-http` | `TraceLayer` for HTTP request tracing |
| `tower` | Layer trait for middleware composition |
| `ractor` | ActorRef for OrchestratorMsg |
| `vo-actor` | OrchestratorMsg message types |
| `serde` | JSON serialization |
| `serde_json` | JSON parsing with error location |
| `bytes` | Bytes for binary body handling |

---

## Reference Implementation

The existing `app.rs::build_app()` shows the current route structure:

```rust
// Current (app.rs)
pub fn build_app(master: ActorRef<OrchestratorMsg>, kv: KvStores) -> Router {
    let api_routes = Router::new()
        .route("/workflows", post(handlers::start_workflow))
        .route("/workflows", get(handlers::list_workflows))
        // ... etc
        .layer(Extension(master))
        .layer(Extension(kv));
    
    Router::new()
        .route("/health", get(health::health_handler))
        .route("/metrics", get(health::metrics_handler))
        .nest("/api/v1", api_routes)
        .layer(TraceLayer::new_for_http())
}
```

The bead spec diverges by:
1. Not nesting under `/api/v1`
2. Using `TraceLayer::new()` instead of `TraceLayer::new_for_http()`
3. Including `JsonBodyLayer` as additional middleware
4. Not including `kv` as an Extension (removing it from create_routes signature)

(End of file - total 285 lines)
