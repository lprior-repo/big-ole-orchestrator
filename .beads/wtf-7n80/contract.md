# Contract Specification

## Context
- Bead ID: wtf-7n80
- Title: wtf-frontend: module setup — copy Oya structure, wire Cargo.toml, replace restate_client
- Feature: Initial module setup for wtf-frontend crate
- Domain terms: Dioxus, Frontend UI, Graph Visualization, Workflow Modeling
- Assumptions: Oya frontend structure is the reference; we copy and adapt
- Open questions: None

## Scope
Set up crates/wtf-frontend/ based on /home/lewis/src/oya-frontend/:
1. Copy src/ui/, src/graph/, src/linter/ into crates/wtf-frontend/src/
2. Update Cargo.toml: dioxus=0.7 features=[web,desktop,router], remove restate_client dependency
3. Create src/wtf_client/mod.rs placeholder
4. In src/lib.rs: re-export ui, graph, wtf_client modules

## Preconditions
- [ ] Parent directory crates/wtf-frontend/ exists
- [ ] Source directories exist in /home/lewis/src/oya-frontend/src/
- [ ] No Restate references in final output

## Postconditions
- [ ] crates/wtf-frontend/src/ui/ contains copied UI modules
- [ ] crates/wtf-frontend/src/graph/ contains copied graph modules
- [ ] crates/wtf-frontend/src/linter/ contains copied linter modules
- [ ] crates/wtf-frontend/src/wtf_client/mod.rs exists as placeholder
- [ ] crates/wtf-frontend/src/lib.rs re-exports ui, graph, wtf_client modules
- [ ] Cargo.toml has correct dioxus features
- [ ] Project compiles without errors
- [ ] Zero Restate dependencies in dependency tree

## Invariants
- [ ] All copied modules maintain functional equivalence with Oya originals
- [ ] Module structure remains < 300 lines per file
- [ ] No Restate types leak into wtf-frontend

## Error Taxonomy
- Error::ModuleNotFound - when copied module does not exist
- Error::RestateReference - when Restate types are detected
- Error::CompilationFailed - when cargo build fails

## Contract Signatures
- setup_wtf_frontend() -> Result<(), Error>

## Type Encoding
| Constraint | Enforcement Level | Type/Pattern |
|---|---|---|
| Parent dir exists | Compile-time | Path validation |
| No Restate refs | Runtime check | grep for "restate" |
| Compiles | Runtime | cargo check |

## Violation Examples
- VIOLATES P1: crates/wtf-frontend/ does not exist -- should fail at setup
- VIOLATES P3: Restate reference found after copy -- should return Err(Error::RestateReference)
- VIOLATES P6: cargo build fails -- should return Err(Error::CompilationFailed)

## Ownership Contracts
- setup_wtf_frontend takes ownership of source paths for copying
- No shared state between threads

## Non-goals
- [ ] Not implementing actual UI logic (done in subsequent beads)
- [ ] Not removing all Oya-specific code (kept for later adaptation)
- [ ] Not implementing full wtf_client (placeholder only)
