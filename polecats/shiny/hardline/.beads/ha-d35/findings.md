# BLACKHAT Security Audit Report — wave3-3

**Scope:** Full Veloxide codebase (`/home/lewis/gt/crates/`)
**Date:** 2026-04-24
**Auditor:** shiny (polecat, hardline rig)

---

## Executive Summary

Adversarial security audit of 1000+ Rust source files across 15 crates. Found **51 findings**: 4 Critical, 8 High, 22 Medium, 17 Low.

The most severe systemic issues are:
1. **No subprocess sandboxing** — child binaries run with full engine privileges
2. **No authentication/authorization** on the HTTP API
3. **Serde Deserialize bypasses constructor validation** on security-critical types (crypto, IDs, credentials)
4. **Multiple TOCTOU races** in lease acquisition, deduplication, and receipt storage

---

## CRITICAL Findings (4)

### C1: Server Panic on Invalid Instance ID
- **File:** `vo-api/src/handlers/workflow_start.rs:63`
- **Type:** Denial of Service
- **Attack:** Single HTTP request with `instance_id: "not-a-ulid"` crashes the entire server via `.expect()`
- **Fix:** Replace `.expect()` with proper error handling returning 400

### C2: No Authentication or Authorization on Any Endpoint
- **File:** `vo-api/src/router.rs:48-118`
- **Type:** Missing Auth
- **Attack:** Any network user can start, terminate, signal, or list all workflows
- **Fix:** Add auth middleware (API key, JWT, mTLS) as tower layer

### C3: No Path Validation on Subprocess Executable
- **File:** `vo-executor/src/subprocess.rs:118`
- **Type:** Command Injection
- **Attack:** Any influence over `executable_path` enables arbitrary binary execution with engine privileges
- **Fix:** Validate absolute path, canonicalize, verify allowlisted directory, verify ownership/permissions

### C4: No Subprocess Sandbox/Resource Isolation
- **File:** `vo-executor/src/subprocess.rs:118-147`, `vo-ipc/src/run.rs:30-52`
- **Type:** Sandbox Escape
- **Attack:** Child processes inherit full UID/GID, filesystem, network, and capability access. A malicious workflow binary can read the database, exfiltrate secrets, pivot to other systems.
- **Fix:** Apply landlock (LSM), seccomp syscall filtering, drop capabilities, add PR_SET_NO_NEW_PRIVS

---

## HIGH Findings (8)

### H1: Permissive CORS Allows Any Origin
- **File:** `vo-api/src/router.rs:116`
- **Fix:** Replace `CorsLayer::permissive()` with explicit origin allowlist

### H2: Unbounded Event Replay Enables DoS
- **File:** `vo-api/src/handlers/query.rs:30-86,93-159,165-234`
- **Attack:** Workflow generating millions of events, then GET timeline forces unbounded memory allocation
- **Fix:** Add pagination with configurable max limit

### H3: Secrets Transmitted in Plaintext Over FD3 Pipe
- **File:** `vo-ipc/src/envelope.rs:16`
- **Attack:** Secrets (API keys, passwords) sent as cleartext JSON to arbitrary child binaries
- **Fix:** Use kernel keyring or `memfd_create` with `MFD_SECRET`

### H4: TOCTOU Race in Binary Validation + Execution
- **File:** `vo-ipc/src/config.rs:30-34`, `vo-ipc/src/run.rs:30`
- **Attack:** Replace validated executable with symlink between `canonicalize()` and `execve()`
- **Fix:** Use `O_PATH | O_NOFOLLOW` + `fexecve`

### H5: Argument Injection via fd3_payload Split
- **File:** `vo-ipc/src/config.rs:99-104`
- **Attack:** `split_whitespace()` on fd3_payload used as command args; no escaping
- **Fix:** Use structured format (JSON array) instead of whitespace splitting

### H6: Use-After-Free in SpscQueue Sender/Receiver
- **File:** `vo-ipc/src/spsc.rs:18-24`
- **Attack:** Drop Arc<SpscQueue> while Sender/Receiver still hold raw pointers → UAF
- **Fix:** Store `Arc<SpscQueue<T>>` in Sender/Receiver instead of raw pointers

### H7: Unvalidated Command String in SpawnSupervisor
- **File:** `vo-actor/src/spawn_supervisor.rs:804-809`
- **Attack:** Command string from persisted storage passed directly to process spawn without validation
- **Fix:** Validate against allowlist of known binary paths

### H8: TOCTOU Race in Lease Acquisition
- **File:** `vo-storage/src/lease_partition/fjall_lease_store.rs:140-178`
- **Attack:** Two concurrent callers both acquire lease for same instance → double-execution of side effects
- **Fix:** Wrap check + fence token + insert in single fjall `OwnedWriteBatch`

---

## MEDIUM Findings (22)

| # | Finding | File | Type |
|---|---------|------|------|
| M1 | Missing security headers | `vo-api/router.rs` | Defense-in-Depth |
| M2 | No request body size limits | `vo-api/router.rs` | Resource Exhaustion |
| M3 | Internal error messages leaked to clients | `vo-api/workflow_start.rs:137`, `workflow.rs:197` | Info Disclosure |
| M4 | WebSocket: no Origin check, no connection limit | `vo-api/ws.rs:184-245` | WS Hijacking |
| M5 | Payload size not validated at config construction | `vo-ipc/config.rs:23` | Late Rejection |
| M6 | No seccomp/PR_SET_NO_NEW_PRIVS on children | `vo-ipc/run.rs:30-52` | Insufficient Sandboxing |
| M7 | Schema validation allows missing required fields | `vo-ipc/envelope.rs:149-174` | Schema Bypass |
| M8 | WriteBudget RefCell not thread-safe | `vo-storage/append.rs:85-170` | Data Race / Panic |
| M9 | Hardcoded timestamp "100" in effect journal commit | `vo-storage/fjall_journal.rs:78-83` | Data Integrity |
| M10 | MmapCache insert TOCTOU race | `vo-storage/mmap_cache.rs:100-148` | Race Condition |
| M11 | TOCTOU in dedupe store `contains` | `vo-storage/fjall_dedupe.rs:141-155` | Broken Idempotency |
| M12 | TOCTOU in receipt/effect journal inserts | `vo-storage/fjall_receipt_store.rs:43-61` | Broken Exactly-Once |
| M13 | False unsafe Send+Sync impl on SchedulerQueue | `vo-executor/scheduler/queue.rs:208-209` | UB / Data Race |
| M14 | StepId::new() bypasses validation | `vo-executor/types.rs:14-17` | Input Validation Bypass |
| M15 | Backoff overflow / infinite retries | `vo-executor/types.rs:162-173` | Retry Abuse |
| M16 | Unbounded global state DashMaps | `vo-executor/state.rs:28-31` | Resource Exhaustion |
| M17 | Unbounded semaphore map growth | `vo-actor/semaphore/workflow.rs:44-59` | Memory Leak |
| M18 | Non-thread-safe InstanceRegistry behind Send+Sync | `vo-actor/instance_registry.rs:184-205` | Data Race |
| M19 | CommandEnvelope serde Deserialize bypasses version validation | `vo-types/command_envelope.rs:62` | Validation Bypass |
| M20 | WrappedDek/EncryptedBlob serde bypasses size validation | `vo-types/encryption.rs:75,104` | Crypto Bypass |
| M21 | SecretValue/Credential serde bypasses validation | `vo-types/credentials.rs:307,401` | Crypto Bypass |
| M22 | Recursive DFS stack overflow in cycle detection | `vo-types/workflow/mod.rs:238-275` | DoS / Stack Overflow |

---

## LOW Findings (17)

| # | Finding | File | Type |
|---|---------|------|------|
| L1 | Namespace validation bypassed in workflow_start.rs | `vo-api/workflow_start.rs:39` | Input Validation |
| L2 | SSE/WS broadcast all events (no instance filtering) | `vo-api/sse.rs:212`, `ws.rs:203` | Info Disclosure |
| L3 | Sync mutex on search engine in async context | `vo-api/query.rs:322` | Lock Contention |
| L4 | FD_CLOEXEC on FD3/FD4 in bus.rs (incorrect) | `vo-ipc/bus.rs:92-97` | Logic Error |
| L5 | PID-based kill vulnerable to PID recycling | `vo-ipc/run.rs:238-240` | Signal Wrong Process |
| L6 | Child can rename itself via prctl | `vo-ipc/run.rs:32` | Forensic Evasion |
| L7 | MmapCache key sanitization gaps | `vo-storage/mmap_cache.rs:295-298` | Input Validation |
| L8 | Snapshot pipe delimiter fragility | `vo-storage/snapshots/mod.rs:79-98` | Data Integrity |
| L9 | Recovery throttle accounting inconsistency | `vo-storage/snapshot_recovery.rs:199-210` | Logic Error |
| L10 | Crate-level unsafe_code allowance | `vo-storage/lib.rs:39` | Code Hygiene |
| L11 | Unbounded StepId string length | `vo-executor/types.rs:11` | DoS |
| L12 | new_unchecked bypasses payload size validation | `vo-actor/signal_messages.rs:140-142` | Validation Bypass |
| L13 | Available permit counter drift | `vo-actor/semaphore/execution.rs:64-73` | Logic Error |
| L14 | SpawnId::new() skips all validation | `vo-types/string_types.rs:342` | ID Spoofing |
| L15 | EventEnvelope.instance_id accepts arbitrary strings | `vo-types/events/envelope.rs:10` | ID Spoofing |
| L16 | CommandEnvelope version 0 accepted | `vo-types/command_envelope.rs:100` | Protocol Confusion |
| L17 | No size limits on workflow deserialization | `vo-sdk/graph.rs:71` | Resource Exhaustion |

---

## Systemic Vulnerability Patterns

### Pattern 1: Serde Deserialize Bypasses Constructor Validation
**Affected types:** CommandEnvelope, WrappedDek, EncryptedBlob, SecretValue, Credential, WorkflowSpec
**Root cause:** Types with validated `new()`/`parse()` constructors derive `serde::Deserialize`, creating a second path that skips all validation.
**Recommendation:** Audit all `#[derive(Deserialize)]` types. Implement custom Deserialize or use `#[serde(deserialize_with)]` to enforce invariants.

### Pattern 2: No Subprocess Isolation
**Affected:** vo-executor, vo-ipc
**Root cause:** Child processes run with identical privileges, filesystem access, and network access as the engine.
**Recommendation:** Apply defense-in-depth: landlock + seccomp + PR_SET_NO_NEW_PRIVS + namespace isolation.

### Pattern 3: TOCTOU Races in Storage Layer
**Affected:** vo-storage (lease, dedupe, receipt, effect journal, mmap_cache)
**Root cause:** Check-then-act patterns across separate storage operations without atomic transactions.
**Recommendation:** Use fjall batch commits or per-key mutexes for serialization.

### Pattern 4: Unbounded Resource Allocation
**Affected:** vo-api (event replay, request bodies), vo-executor (state maps), vo-actor (semaphore maps)
**Root cause:** No upper bounds on collection sizes, request payloads, or query results.
**Recommendation:** Add configurable limits on all user-controlled sizes.

---

## Remediation Priority

1. **Immediate (P0):** C1 (panic), C2 (no auth), C3+C4 (subprocess sandbox) — exploitable now
2. **Short-term (P1):** H1-H8 — high-impact, requires moderate effort
3. **Medium-term (P2):** M1-M22 — systemic patterns requiring architectural changes
4. **Low-priority (P3):** L1-L17 — defense-in-depth and hardening
