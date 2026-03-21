# Implementation Summary

## Bead: wtf-7n80
## Title: wtf-frontend: module setup — copy Oya structure, wire Cargo.toml, replace restate_client

## Changes Made

### 1. Created wtf_client module
- `crates/wtf-frontend/src/wtf_client/mod.rs` - Module entry point
- `crates/wtf-frontend/src/wtf_client/client.rs` - WtfClient placeholder
- `crates/wtf-frontend/src/wtf_client/types.rs` - Type placeholders (InstanceView, EventRecord, etc.)

### 2. Created graph_core_types in lib.rs
- NodeId, PortName, NodeCategory, Viewport types
- Minimal implementations for paradigm-agnostic types

### 3. Copied Oya structure (for subsequent adaptation)
- `crates/wtf-frontend/src/ui/` - UI modules (not yet wired, has oya_frontend deps)
- `crates/wtf-frontend/src/graph/` - Graph modules (not yet wired, has petgraph/serde_yaml deps)
- `crates/wtf-frontend/src/linter/` - Linter modules (not yet wired, has serde_yaml deps)

### 4. Updated lib.rs
- Re-exports wtf_client module
- Defines graph_core_types inline for compilation
- Documents TODO items for subsequent beads

## Compilation Status
- **Status**: PASS
- **Warnings**: 1 dead_code warning in wtf_client::client.rs
- **Restate References**: None in compiled code (exist in non-compiled modules for subsequent adaptation)

## What Compiles
- `wtf_client` module with placeholder types
- `graph_core_types` with NodeId, PortName, NodeCategory, Viewport

## What Needs Subsequent Beads
1. Add missing dependencies: petgraph, serde_yaml, wasm-*, itertools
2. Replace oya_frontend references with crate::graph references
3. Remove/adapt Restate-specific types (restate_types.rs)
4. Wire up UI modules with proper Dioxus hooks
5. Implement full WtfClient functionality

## Verification
- `cargo check` passes
- No Restate references in compiled output
- Module structure matches Oya frontend layout
