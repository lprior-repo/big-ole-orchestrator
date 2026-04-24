# ADR 024 (v2): SSE Connection Leaks and Broadcast Limits

## Status
Accepted

## Context
The Engine exposes a Server-Sent Events (SSE) endpoint for the Dioxus UI. Axum holds an open TCP connection for every active browser tab.
If a user closes their laptop lid without closing the browser tab, the TCP connection enters a "half-open" state. The server does not know the client is gone and continues buffering events into memory forever. With 20 leaked connections, the server slowly bleeds memory (OOM).

Additionally, if the Engine is processing 10,000 events/sec, blasting all 10,000 events to every connected UI client will overwhelm the server's network stack and the browser's rendering engine.

## Decision

### 1. SSE Keepalive and Timeout
The Engine must implement a strict keepalive mechanism.
- The Axum SSE handler emits a `:keepalive` ping comment every 15 seconds.
- If the underlying TCP socket fails to write the ping (Broken Pipe), the Engine instantly drops the connection and frees the memory.

### 2. Bounded Broadcast Channels
The SSE endpoint must not use unbounded queues.
- The Engine pushes events to a `tokio::sync::broadcast` channel with a strict capacity (e.g., `1000` events).
- If a client connection is too slow to read (e.g., a slow mobile network) and falls behind by more than 1000 events, `tokio` will yield a `Lagged` error.
- The Engine catches the `Lagged` error and forcibly drops the SSE connection. 
- The Dioxus UI is programmed to catch this disconnection, fetch the latest full state via a standard HTTP `GET`, and seamlessly reconnect to the SSE stream.

## Consequences
- **Positive:** Protects the Engine from memory leaks caused by dead UI clients.
- **Positive:** Protects the Engine from network saturation caused by slow UI clients.
- **Negative:** The Dioxus UI must handle reconnection and state-reconciliation gracefully, adding slight frontend complexity.