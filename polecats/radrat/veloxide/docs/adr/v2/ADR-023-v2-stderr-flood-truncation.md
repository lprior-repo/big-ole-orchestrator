# ADR 023 (v2): The Stderr Flood and Truncation Guard

## Status
Accepted

## Context
The Engine captures `stderr` from child binaries to provide useful logging in the UI. 
If a buggy task prints 10GB of error logs in a tight loop, or continuously streams logs without ever exiting, the Engine's memory buffer will overflow, crashing the entire orchestrator (an OOM Memory Bomb via Stderr).

## Decision
The Engine implements **Bounded Stderr Capture**.

1. The Engine reads `stderr` from the child process into a strict capacity-limited buffer (e.g., `MAX_STDERR_BYTES = 1MB`).
2. Once the buffer hits 1MB, the Engine **stops reading** from the `stderr` pipe.
3. Crucially, the Engine does *not* close the pipe (which would send a `SIGPIPE` and unexpectedly kill the child). 
4. The Engine lets the OS kernel buffer fill up. If the child attempts to write more logs, its `write()` call blocks.
5. If the child's main thread blocks on this write, it hangs. The Engine's execution timeout (e.g., 60 seconds) will eventually fire and kill the child cleanly for taking too long.
6. The captured logs are saved to `fjall` with a clear suffix: `[... TRUNCATED AT 1MB ...]`.

## Consequences
- **Positive:** Mathematically impossible to OOM the Engine via logging attacks.
- **Positive:** Accidental debug loops result in clean, understandable timeout failures rather than cascading system crashes.
- **Negative:** Legitimate (but massive) debugging logs may be lost, requiring users to log directly to external systems if they need >1MB of trace data per task.