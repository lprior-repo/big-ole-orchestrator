# ADR-008: Single Binary Deployment

## Status

Amended by ADR-013

The single-binary goal is preserved but NATS is now a required external dependency. The engine binary embeds the Dioxus frontend, the Ractor supervisor tree, the axum API server, and the sled snapshot store. NATS (3-node cluster or single-node for dev) is external. For local development, the engine binary ships with a `--nats-embedded` flag that starts a bundled NATS server in-process. Production deployments use a real NATS cluster. This preserves the "download and run" developer experience while enabling distributed durability.

## Context

wtf-engine is a **durable workflow execution engine** that should be:

- **Easy to deploy** - No external dependencies (no separate DB process)
- **Cross-platform** - Works on Linux, macOS, Windows
- **Self-contained** - Contains frontend (WASM), API server, worker
- **Simple operations** - One command to start, one command to update

### Deployment Options Considered

| Approach | Pros | Cons |
|----------|------|------|
| **Single binary** | One command deploy, no external deps | Larger binary size |
| **Container** | Isolation, reproducibility | Docker dependency, more complex |
| **Separate services** | Independent scaling | More complex ops, network latency |
| **Serverless** | Auto-scale, pay-per-use | Cold starts, vendor lock-in |

## Decision

We will ship wtf-engine as a **single fat binary** that contains:

1. **API server** (Axum HTTP)
2. **Worker** (workflow execution engine)
3. **Frontend** (Dioxus WASM, embedded)
4. **Database** (sled embedded)

### Binary Contents

```
wtf-engine
├── API Server (port 8080 by default)
│   └── /health, /api/v1/*
├── Worker (internal, processes workflows)
│   └── Actor system, journal replay
├── Frontend (served on same port or separate)
│   └── WASM UI for workflow visualization
└── Database (sled)
    └── Local file storage
```

### CLI Interface

```bash
# Start server with defaults
wtf serve

# Start with custom config
wtf serve --port 9000 --db-path /data/wtf.db

# Start worker (connects to server)
wtf worker --server localhost:9000

# Run single binary with embedded UI
wtf all-in-one --port 8080 --ui
```

### Configuration

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_port")]
    pub port: u16,

    #[serde(default = "default_host")]
    pub host: String,

    #[serde(default = "default_db_path")]
    pub db_path: String,

    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: usize,

    #[serde(default = "default_db_path")]
    pub log_level: String,
}

fn default_port() -> u16 { 8080 }
fn default_host() -> String { "0.0.0.0".to_string() }
fn default_db_path() -> String { "wtf-engine.db".to_string() }
fn default_max_concurrent() -> usize { 3 }
```

### Build Output

```toml
# Cargo.toml - single binary package
[[bin]]
name = "wtf"
path = "src/main.rs"

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
strip = true
```

### Docker Alternative

For users who prefer containers:

```dockerfile
FROM scratch
COPY wtf-engine /wtf-engine
ENTRYPOINT ["/wtf-engine"]
CMD ["serve"]
```

## Consequences

### Positive

- **Zero-config deployment** - Download and run
- **No external dependencies** - Not even SQLite or RocksDB
- **Simple operations** - One process to monitor
- **Reproducible** - Same binary everywhere
- **Fast startup** - No container runtime overhead

### Negative

- **Binary size** - Larger than minimal (includes WASM, all features)
- **No isolation** - Process crash affects everything
- **Memory bound** - All in one process

### Mitigations

- Binary size with LTO + strip is ~30-50MB (acceptable)
- Supervision (systemd, k8s) handles crash recovery
- Resource limits via cgroups
