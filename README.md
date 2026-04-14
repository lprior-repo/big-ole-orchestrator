# vo-engine

DAG-based durable workflow execution engine combining:

- **petgraph DAGs** for workflow representation
- **ractor actors** for supervision and isolation
- **fjall** for embedded, async-first persistence (LSM-Tree)
- **Step Functions parity** (minus long waits)
- **Single fat binary** with MIT license

## Features

- Full AWS Step Functions parity (Pass, Task, Choice, Parallel, Map, Wait, etc.)
- Journal-based replay for crash recovery
- ractor actor model with Erlang-style supervision
- petgraph-powered DAG execution
- Single binary deployment (API + Worker + Frontend + DB)
- 3x parallelism by default

## Quick Start

```bash
# Build
cargo build --release

# Run server
cargo run --release -- serve

# Run CLI
cargo run --release -- --help
```

## Documentation

- [Architecture](docs/architecture.md)
- [ADR Index](docs/adr/)

## Crates

| Crate | Description |
|-------|-------------|
| `vo-core` | Core types, DAG, journal, replay |
| `vo-storage` | fjall persistence layer |
| `vo-actor` | ractor actors |
| `vo-worker` | Worker loop, activity execution |
| `vo-api` | Axum HTTP API |
| `vo-cli` | CLI client |
| `vo-frontend` | Dioxus web UI |
| `vo-common` | Shared types |
| `vo-types` | Domain types with proptest support |
| `vo-ipc` | FD3/FD4 pipe protocol, subprocess I/O |
| `vo-sdk` | Developer macro crate (`#[vo_task]`) |
| `vo-sdk-macros` | Procedural macros for vo-sdk |
| `vo-executor` | Execute-node error handling, timeouts, retries |
| `vo-linter` | Static analysis for workflow definitions |

## License

MIT
