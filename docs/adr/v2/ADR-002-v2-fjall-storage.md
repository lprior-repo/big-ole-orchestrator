# ADR 002 (v2): Storage Pivot to Fjall (LSM-Tree)

## Status
Accepted

## Context
v1 used NATS JetStream as the durable event log. To achieve a true "Single Binary" architecture without sacrificing Event Sourced write-throughput, we must embed the storage engine directly into the `vo-engine` process. 

Relational embedded databases (like SQLite) suffer from global write-locks, which throttle throughput when 10,000 concurrent actors attempt to append events simultaneously.

## Decision
We will use **`fjall`**, a pure-Rust Log-Structured Merge-tree (LSM-tree), as the sole storage engine. 

### The KV Schema Partitions
Because `fjall` is a Key-Value store, we will isolate data into explicit partitions:
1. **`events`**: Key = `<instance_id>:<seq>`. Value = `WorkflowEvent`. The immutable, append-only source of truth.
2. **`instances`**: Key = `<status>:<timestamp>:<instance_id>`. Value = `InstanceSummary`. Serves as a materialized view so the UI can execute prefix-scans to power the dashboard (e.g., "Get failed workflows").
3. **`timers`**: Key = `<fire_at_timestamp>:<instance_id>`. Used by the hibernation loop to re-spawn suspended actors.

### High-Throughput Batching (`DbWriterActor`)
To maximize NVMe IOPS, individual `ractor` actors will **not** call `fsync` directly. All actors will send their events to a central `DbWriterActor`. This actor will batch events in memory and execute a single group-commit flush to the disk every few milliseconds.

## Consequences
- **Positive:** Unparalleled write throughput (100k+ events/sec) natively in Rust.
- **Positive:** Zero external dependencies or C++ build scripts (unlike RocksDB).
- **Negative:** We lose SQL queryability. The UI must rely entirely on custom-built KV prefix scans for all dashboard filtering and sorting.
- **Negative:** Schema migrations require careful `serde` versioning, as there is no `ALTER TABLE` equivalent.