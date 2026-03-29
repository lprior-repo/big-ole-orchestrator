# vo-engine

DAG-based durable workflow execution engine combining:

- **petgraph DAGs** for workflow representation
- **ractor actors** for supervision and isolation
- **sled** for embedded, async-first persistence
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
| `vo-storage` | sled persistence layer |
| `vo-actor` | ractor actors |
| `vo-worker` | Worker loop, activity execution |
| `vo-api` | Axum HTTP API |
| `vo-cli` | CLI client |
| `vo-frontend` | Dioxus web UI |
| `vo-common` | Shared types |

## License

MIT
