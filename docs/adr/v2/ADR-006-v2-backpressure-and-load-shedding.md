# ADR 006 (v2): Backpressure and Load Shedding

## Status
Accepted

## Context
Because `vo-engine` is designed to be a "Single Node Supreme" architecture, it must aggressively protect its host operating system. 

While the `fjall` database and `ractor` actors can handle millions of operations in memory, the Execution Layer relies on `tokio::process::Command` to spawn physical OS subprocesses for user binaries. 

If 100,000 webhooks fire simultaneously, attempting to spawn 100,000 OS processes will exhaust file descriptors (ulimit), RAM, and CPU threads, crashing the host OS and the engine with it.

## Decision
We implement strict, memory-efficient backpressure using `tokio::sync::Semaphore` and HTTP Load Shedding.

### 1. The Execution Semaphore
The Engine initializes a global `tokio::sync::Semaphore` with a fixed number of permits (e.g., `MAX_CONCURRENT_BINARIES = 500`). 
When a `ractor` actor reaches a `Task` node and prepares to spawn a binary, it must `acquire()` a permit from this semaphore.

### 2. Zero-Cost Yielding
If all 500 permits are in use, the actor `await`s the permit. Because this is a Tokio async context, the actor instantly yields execution back to the runtime. The actor sits in memory consuming ~1KB of RAM and **0% CPU** while waiting in the queue. 

### 3. Ingress Load Shedding
If the internal semaphore queue grows too large (e.g., > 5,000 actors yielded and waiting for a permit), the Engine's Axum ingress router (which handles incoming webhooks) will automatically shed load.
New incoming HTTP requests will instantly receive a `HTTP 429 Too Many Requests` or `HTTP 503 Service Unavailable` with a `Retry-After` header.

## Consequences
- **Positive:** The engine is functionally indestructible under viral load. It will process exactly what the OS can handle and queue the rest safely in memory.
- **Positive:** Pushing backpressure to the HTTP ingress layer allows upstream load balancers or webhook providers (like Stripe) to handle the retry logic.
- **Negative:** Workflows at the back of a large queue will experience high execution latency (Time-To-Start). This is an unavoidable physical constraint of single-node deployments.