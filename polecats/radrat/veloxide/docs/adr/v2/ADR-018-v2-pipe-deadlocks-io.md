# ADR 018 (v2): Pipe Deadlocks and I/O Boundaries

## Status
Accepted

## Context
When the Engine passes JSON state to a workflow binary over pipes (FD3 in, FD4 out), it is vulnerable to the classic Unix Pipe Deadlock. 
If the Engine writes a 200KB payload to FD3 synchronously, the kernel pipe buffer (64KB) fills up. The Engine blocks until the child reads. If the child hasn't started reading (e.g., still initializing `tokio`), and then later tries to write a large payload back to FD4 before reading, both sides block forever.

## Decision
We enforce strict asynchronous pipe handling and defined IO sequences.

### The Engine Sequence
The Engine must **never** block synchronously on pipe I/O.
1. The Engine spawns the child with FD3/FD4 configured as asynchronous pipes.
2. The Engine uses `tokio::io::copy` to stream the payload into FD3.
3. **CRITICAL:** The Engine immediately closes the write end of FD3 upon completion to send EOF.
4. The Engine concurrently (using `tokio::select!` or `tokio::join!`) reads from FD4 into a bounded buffer until EOF.
5. The Engine calls `waitpid()` on the child.

### The SDK Sequence
The `vo-sdk` enforces a strict sequence on the binary side:
1. Read all data from FD3 until EOF.
2. **Close FD3 immediately.**
3. Execute the user's task logic.
4. Write output to FD4.
5. **Close FD4.**
6. Exit.

## Consequences
- **Positive:** Mathematically eliminates pipe deadlocks regardless of payload size.
- **Positive:** Closing FD3 early frees kernel buffer resources during long-running tasks.
- **Negative:** Requires careful async stream management on the Engine side to prevent dropping partial writes if the child panics early.