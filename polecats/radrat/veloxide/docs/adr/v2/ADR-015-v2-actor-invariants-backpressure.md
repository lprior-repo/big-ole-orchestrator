# ADR 015 (v2): Actor Invariants and Backpressure Inversion

## Status
Accepted

## Context
A durable workflow engine relies entirely on the correctness of its concurrency model. 
1. **Stale Actor Resurrection:** If the engine wakes up an actor from a timer while a previous instance of that actor is still shutting down, two actors will exist for the same workflow, resulting in duplicate executions.
2. **Backpressure Inversion:** If 5,000 actors simultaneously send events to the `DbWriterActor`, the writer's mailbox will grow unboundedly. The system moves the pressure from the disk to the RAM, eventually causing an Out-Of-Memory (OOM) crash while continuing to accept new webhooks.

## Decision

### 1. The Single-Writer Invariant (Strict Registration)
The Engine must guarantee: **At most ONE active `ractor` actor per workflow instance at any point in time.**
- The Master Orchestrator maintains an `ActiveInstances` registry (e.g., `DashMap<InstanceId, ()>`).
- Before the Engine (or the Timer loop) spawns an actor, it must acquire the logical lock in the registry. 
- If the lock is held, the wake-up signal is queued until the existing actor cleanly terminates and releases the lock via its `post_stop` hook.

### 2. Bounded Mailboxes and Ingress Shedding
To prevent Backpressure Inversion, the `DbWriterActor` is configured with a **strictly bounded mailbox** (e.g., 10,000 messages).
- When the mailbox is full, sending actors will block (yield). This is correct behavior.
- The Engine monitors the `DbWriterActor`'s mailbox depth as a core health metric.
- If the mailbox exceeds 80% capacity, the HTTP API automatically begins returning `429 Too Many Requests` or `503 Service Unavailable`, shedding load at the network ingress before it enters the actor system.

## Consequences
- **Positive:** Mathematically impossible to double-execute a workflow step due to stale actors.
- **Positive:** The engine protects its own RAM by pushing backpressure to the edge of the system.
- **Negative:** External systems must be prepared to handle HTTP 429s during massive viral spikes.