# ADR 013 (v2): System Resilience (Thundering Herds, Storage Watchdogs)

## Status
Accepted

## Context
When deploying a workflow engine on a single node, operational physics dictate failure modes.
1. **The Thundering Herd:** If the server crashes with 5,000 active instances, upon restart the Engine could instantly attempt to replay and resume all 5,000 instances, pinning the CPU and likely triggering another crash.
2. **The Storage Stall:** Even before the disk is full, LSM compaction debt, write stalls, or a wedged flush path can make the `DbWriterActor` the global bottleneck.
3. **Clock Skew:** Relying solely on `fire_at` timestamps for hibernation wakeups breaks if the server's NTP clock jumps backwards or forwards.

## Decision
We implement a three-tiered system resilience protocol.

### 1. Crash Recovery Startup Throttle
On startup, the Engine does not instantly resume all in-flight instances.
- The Engine reads the instances from `fjall` and places them in a recovery queue.
- It processes the queue in configurable batches.
- Recovery consumes reserved class budget so live ingress and recovery cannot starve each other indefinitely.

### 2. Storage Watchdog and Degraded Mode
A background Tokio task monitors:
- filesystem free space,
- `DbWriterActor` commit latency,
- writer queue depth,
- flush timeout frequency,
- storage stall or compaction-backlog indicators.

If critical thresholds are crossed, the Engine enters **Degraded Mode**:
- New workflows and non-critical signals are rejected with `HTTP 503`.
- Non-critical blob writes and projections are paused or deferred.
- Recovery and in-flight exact workflows receive reserved budget.
- A strict flush timeout still applies; if the writer cannot make forward progress, the Engine shuts down cleanly rather than deadlocking in place.

### 3. Dual-Clock Timer Verification
To survive clock skew, the Engine records both an absolute timestamp and a monotonic duration when a workflow hibernates.
- The `TimerScheduled` event contains `fire_at` (absolute) and `duration_ms` (monotonic relative to insertion).
- The reanimator loop verifies `fire_at <= Utc::now() OR elapsed_since_set >= duration_ms`.

## Consequences
- **Positive:** The system degrades gracefully under storage pressure instead of silently stalling.
- **Positive:** Recovery no longer competes blindly with live traffic.
- **Positive:** Time-travel bugs caused by NTP syncs are eliminated.
- **Negative:** Additional monitoring, thresholds, and operational states must be maintained.
