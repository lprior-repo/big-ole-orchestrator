# Test Plan: vo-qz46 — wtf serve actor assembly

bead_id: vo-qz46
bead_title: vo-cli: wtf serve actor assembly, axum binding, and graceful shutdown
phase: test-plan
updated_at: 2026-03-21T04:27:00Z

## Given-When-Then Test Cases

### GWT-001: Happy Path Server Start and Shutdown

**Given**: NATS is running on localhost:4222 with JetStream enabled
**And**: data directory is empty or contains valid sled DB
**And**: no process is using the target port (e.g., 4222)

**When**: `wtf serve --port 4222 --nats-url nats://localhost:4222` is executed

**Then**:
- MasterOrchestrator actor spawns successfully
- HTTP server binds to port 4222
- `/health` endpoint returns 200 OK
- Sending SIGTERM terminates the server gracefully
- Server exits with code 0

### GWT-002: Server Refuses to Start on Already-Bound Port

**Given**: a process is listening on port 4222

**When**: `wtf serve --port 4222` is executed

**Then**:
- Server returns error: "port already in use"
- Server exits with code 1

### GWT-003: Health Endpoint Returns OK During Runtime

**Given**: `wtf serve` is running

**When**: `curl http://localhost:4222/health` is called

**Then**:
- Response status is 200 OK
- Response body contains `{"status":"ok"}` or similar

### GWT-004: Graceful Shutdown on SIGTERM

**Given**: `wtf serve` is running with active workflow instances

**When**: SIGTERM is sent to the process

**Then**:
- HTTP server stops accepting new connections
- In-flight requests complete (up to timeout)
- Sled snapshots are flushed
- NATS connection is closed
- Process exits with code 0

### GWT-005: Graceful Shutdown on SIGINT

**Given**: `wtf serve` is running

**When**: SIGINT (Ctrl+C) is sent to the process

**Then**:
- Same behavior as SIGTERM

### GWT-006: Shutdown Completes Within Timeout

**Given**: `wtf serve` is running with long-running workflow instances

**When**: SIGTERM is sent

**Then**:
- Shutdown completes within 30 seconds
- If actors don't drain in time, they are force-stopped
- Sled flush is attempted even on force-stop

### GWT-007: Start Workflow Via API

**Given**: `wtf serve` is running and NATS JetStream has workflow definitions

**When**: `POST /api/v1/workflows` is called with valid workflow payload

**Then**:
- Response status is 202 Accepted
- Workflow instance is created
- MasterOrchestrator receives StartWorkflow message

### GWT-008: Heartbeat Watcher Detects Expired Instance

**Given**: `wtf serve` is running and a workflow instance exists with an expired heartbeat

**When**: The heartbeat TTL (10s) elapses without refresh

**Then**:
- Heartbeat watcher detects the expired entry
- `HeartbeatExpired` message is sent to MasterOrchestrator
- MasterOrchestrator triggers recovery for the dead instance

## Integration Test Requirements

### IT-001: End-to-End Workflow Execution

**Test**:
1. Start `wtf serve`
2. Submit a workflow definition
3. Start a workflow instance
4. Verify instance appears in `vo-instances` KV
5. Signal the instance
6. Verify state transitions
7. Terminate via SIGTERM
8. Verify clean shutdown

### IT-002: Crash Recovery

**Test**:
1. Start `wtf serve`
2. Start a workflow instance
3. Kill the process with SIGKILL (no graceful shutdown)
4. Restart `wtf serve`
5. Verify instance is recovered from JetStream replay

## Mock Points

- `NatsClient`: Use embedded NATS for tests
- `sled_db`: Use temporary directory for tests
- `ActorRef<OrchestratorMsg>`: Mock for unit tests

## Verification Criteria

- [ ] Server starts and binds to port
- [ ] `/health` returns 200
- [ ] SIGTERM triggers graceful shutdown
- [ ] SIGINT triggers graceful shutdown
- [ ] Shutdown completes within 30s timeout
- [ ] Sled snapshots are flushed on shutdown
- [ ] NATS connection is closed cleanly
- [ ] Actor spawn failure is handled gracefully
- [ ] Port conflict is reported clearly