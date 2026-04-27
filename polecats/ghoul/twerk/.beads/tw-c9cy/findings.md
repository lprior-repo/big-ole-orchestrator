# Findings: tw-c9cy - standalone: repair local quick-start job execution

## Issue Summary
When submitting `examples/hello-shell.yaml` to `POST /jobs?wait=true` in standalone mode, the job gets stuck in PENDING and the blocking request hangs indefinitely.

## Research Conducted

### Files Analyzed
- `crates/twerk-cli/src/run.rs` - Standalone engine startup
- `crates/twerk-app/src/engine/engine_lifecycle.rs` - Standalone mode initialization  
- `crates/twerk-app/src/engine/coordinator/mod.rs` - Coordinator job subscriptions
- `crates/twerk-app/src/engine/coordinator/handlers/job_handlers.rs` - Job event processing
- `crates/twerk-app/src/engine/coordinator/handlers/task_handlers.rs` - Task completion flow
- `crates/twerk-app/src/engine/worker/mod.rs` - Worker queue subscriptions
- `crates/twerk-infrastructure/src/broker/` - Broker implementations
- `crates/twerk-web/src/api/handlers/jobs/create.rs` - Job creation handler

### Root Cause Analysis (from prior analysis)

The job flow in standalone mode:
1. POST /jobs creates job with state=Pending, publishes via `publish_job`
2. Coordinator's `subscribe_for_jobs` handler receives job via `handle_job_event`
3. `handle_job_event(Pending)` calls `start_job`
4. `start_job` transitions job to Scheduled, creates task, calls `handle_pending_task`
5. `Scheduler::schedule_task` publishes task to "default" queue via `publish_task`
6. Worker's `subscribe_for_tasks("default", handler)` should receive task
7. Worker executes task via `execute_task`
8. Task completion triggers `handle_task_completed` -> `handle_top_level_task_completed`
9. When all tasks done, `broker.publish_job(&completed_job)` is called
10. Coordinator receives job via `subscribe_for_jobs` -> `handle_job_event(Completed)`
11. `complete_job` calls `publish_event("job.completed", ...)` to typed channels
12. `wait_for_job_completion` receives event via `subscription.recv()`

### Known Issues from Prior Analysis

1. **Broker Proxy Initialization Order**: `BrokerProxy` delegates to inner broker. If `subscribe_for_tasks` is called before the broker is initialized, it returns `BrokerNotInitialized` error. This could happen during startup race conditions.

2. **Shell Runtime Configuration**: The config shows `runtime.type = "shell"` and `runtime.shell.cmd = ["bash", "-c"]`. If shell runtime has issues, tasks won't execute properly.

3. **Dolt Server Instability**: The Dolt server (beads database) is not consistently accessible during this session, preventing bead updates.

## Status
- Research completed
- Dolt server connectivity issues prevented bead updates
- Unable to implement fixes due to:
  1. Worktree at `/home/lewis/gt/polecats/ghoul/twerk/` is not a git repository
  2. Actual source code is at `/home/lewis/src/twerk/` (different location)
  3. Dolt server not reliably accessible

## Recommendations
1. Start Dolt server reliably before attempting bead work
2. Use actual source repo at `/home/lewis/src/twerk/` for code changes
3. Add instrumentation to trace actual execution flow in standalone mode
4. Verify broker initialization order in standalone startup sequence