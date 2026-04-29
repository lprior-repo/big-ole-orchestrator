# Findings: tw-38x - Health Check Endpoint

## Task
Implement GET /health that returns API server status, fjall storage connectivity, active actor count, and uptime. Return 200 if all healthy, 503 if any component is down.

## Investigation Summary

### 1. API Server Infrastructure - DOES NOT EXIST

The `vo serve` command is defined in CLI (`crates/vo-cli/src/cli.rs`) and routed via `ServeHandler` in `registry.rs`, but the implementation file `crates/vo-cli/src/commands/serve.rs` **does not exist**.

```
crates/vo-cli/src/commands/
├── check.rs
├── doctor_checks.rs
├── doctor.rs
├── init.rs
├── mod.rs         # references `pub mod serve;` but file doesn't exist
├── purge.rs
├── rebuild.rs
└── status.rs
```

### 2. vo-api Crate - DOES NOT EXIST

The `vo-cli/Cargo.toml` lists `vo-api` as a dependency:
```toml
vo-api = { version = "0.1.0", path = "../vo-api" }
```

However, `crates/vo-api/` does not exist in the workspace.

### 3. Existing HTTP/Axum Usage

- `axum = "0.8"` is in workspace dependencies
- `tower-http` with cors, timeout, trace features is available
- No actual HTTP server implementation exists in the codebase
- No `axum::Router`, `make_service`, or handler implementations found

### 4. Storage Layer

- `vo-storage` crate is referenced but `crates/vo-storage/src/` does not exist
- `fjall` is a workspace dependency (v3)
- No fjall `Database` instantiation or health-check patterns found

### 5. Actor System

- `ractor = "0.15"` is in workspace dependencies
- `vo-actor` crate path exists in Cargo.toml but `crates/vo-actor/` is empty (no src/)
- No actor spawn monitoring or active count tracking exists

## Conclusion

**Cannot implement health check endpoint - the API server infrastructure does not exist.**

The bead asks to add a health endpoint to an API server that hasn't been built. This would require:

1. Creating `vo-api` crate with axum router setup
2. Creating `serve.rs` implementation that initializes the HTTP server
3. Creating storage layer integration with fjall
4. Creating actor system monitoring for active actor count
5. Implementing uptime tracking
6. Then adding the `/health` endpoint

This is a feature that blocks on architecture implementation, not a standalone enhancement.

## Recommendation

File a new bead for the foundational API server infrastructure (vo-api crate + serve command implementation), then this health check bead depends on that work.

**Closing with reason: no-changes: API server infrastructure (vo-api crate, serve.rs) does not exist - requires architecture work first**
