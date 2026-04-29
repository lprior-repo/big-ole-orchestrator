# Findings: tw-tqzd - Add connection pooling for Dolt SQL in beads

## Issue Summary
Implement connection pooling for the Dolt SQL connections used by bd. Reuse connections across commands. Set pool size based on concurrent polecat count. Add health checking for pooled connections.

## Research Findings

### 1. Architecture Analysis

The `bd` tool is a Go binary that manages beads (issues) stored in Dolt SQL databases. Each `bd` CLI invocation creates a new OS process that:
1. Parses command-line arguments
2. Creates a new `sql.DB` connection pool via `sql.Open("mysql", dsn)`
3. Executes SQL operations
4. Closes the connection pool and exits

The connection pooling already exists **within** each `bd` process (using Go's `database/sql` package's built-in pooling with `SetMaxOpenConns` and `SetMaxIdleConns`), but the overhead comes from:
- Process creation overhead (~10-50ms)
- New TCP connection establishment to Dolt server
- New MySQL connection authentication
- Connection configuration (session variables, character sets, etc.)

### 2. Beads Source Code Location

The `bd` source code is located in the Go module cache:
```
/home/lewis/go/pkg/mod/github.com/steveyegge/beads@v1.0.0/
```

Key files:
- `cmd/bd/` - CLI command implementations
- `internal/storage/dolt/store.go` - Dolt storage implementation with connection pooling
- `internal/doltserver/servermode.go` - Server mode configuration

### 3. Current Connection Pooling in Beads

In `internal/storage/dolt/store.go`, the `openServerConnection` function (line 1108):
```go
db.SetMaxOpenConns(maxOpen)  // default 10
db.SetMaxIdleConns(min(5, maxOpen))
db.SetConnMaxLifetime(5 * time.Minute)
```

The `Config` struct has a `MaxOpenConns` field for pool sizing.

### 4. The Problem

During fleet-feed cycles, many `bd` commands run in sequence across multiple polecats. Each invocation:
- Spawns a new process
- Creates a new connection pool
- Establishes new TCP/MySQL connections
- Then closes everything

This repeated connection setup causes measurable overhead.

### 5. Solutions Considered

**Option A: Daemon Mode for bd**
Modify `bd` to run as a long-lived daemon that maintains a persistent connection pool and accepts requests via Unix socket or HTTP. Each `bd` command would become a lightweight client request instead of a full process spawn.

**Option B: Connection Pooling Proxy**
Create a local proxy (similar to pgbouncer for PostgreSQL) that:
- Maintains a pool of connections to Dolt SQL server
- Routes `bd` command requests through the pool
- Enables connection reuse across `bd` invocations

**Option C: Use beadsdk Directly**
Replace `bd` CLI calls with direct use of the `beadsdk.Storage` interface in Go. This eliminates subprocess overhead entirely (~600ms per operation savings noted in gastown's `Beads` struct comments).

### 6. Veloxide Context

Veloxide is a Rust project that calls `bd` as an external CLI tool via shell commands. The veloxide codebase does not directly interact with Dolt SQL - it only spawns `bd` subprocesses.

The gastown codebase (at `/home/lewis/src/gastown/`) wraps `bd` CLI calls and already has an in-process `beadsdk.Storage` path that bypasses subprocess overhead.

### 7. Recommendation

This issue requires modifications to the `bd` tool itself, not the veloxide codebase. The `bd` source is a separate Go project at `github.com/steveyegge/beads`.

Implementation path:
1. Add a daemon mode to `bd` that maintains a persistent connection pool
2. Or create a `bd proxy` sidecar that pools connections
3. Or expand gastown's use of `beadsdk.Storage` to eliminate `bd` subprocess calls

## Conclusion

**This bead cannot be implemented from within the veloxide worktree.** The fix requires access to the `bd` source code repository (github.com/steveyegge/beads) to:
1. Add daemon/proxy mode with persistent connection pooling
2. Add health checking for pooled connections
3. Configure pool size based on concurrent polecat count

The veloxide team should either:
- File this as a feature request in the beads repository
- Or implement a connection proxy as a separate Rust/Go service
