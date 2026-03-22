# Contract Specification: WTF-L006

## Context
- **Feature**: Detect `std::thread::spawn()` calls inside workflow functions
- **Bead ID**: wtf-rbay
- **Bead Title**: implement wtf-linter WTF-L006: std-thread-spawn-in-workflow
- **Domain terms**: workflow function, procedural workflow, lint rule, syn Visit, Diagnostic
- **Assumptions**: 
  - Workflow functions are identified by being `async fn execute(&self, ...)` inside an `impl` block that has a method named `execute`
  - `std::thread::spawn` is detected via path matching: segments[0].ident == "std" && segments[1].ident == "thread" && segments[2].ident == "spawn"
  - Follows existing L005 (tokio-spawn) pattern, adapting for std::thread::spawn
- **Open questions**: None

## Preconditions
- [P1] Input source is valid Rust syntax (parseable by syn)
- [P2] Workflow function detection uses the same algorithm as L005: impl block with `execute` async fn

## Postconditions
- [Q1] If `std::thread::spawn()` is found inside a workflow function's execute body, exactly one Diagnostic with LintCode::L006 is emitted
- [Q2] No diagnostic is emitted for code outside workflow functions
- [Q3] No diagnostic is emitted for `std::thread::spawn` calls in non-workflow async functions
- [Q4] Multiple violations in the same workflow emit one diagnostic per violation
- [Q5] Nested `std::thread::spawn` calls (e.g., inside closures, if branches, match arms) are detected
- [Q6] The Diagnostic message describes the violation clearly and provides a suggestion

## Invariants
- [I1] The linter never panics on valid Rust input
- [I2] The linter returns ParseError (not panics) for invalid Rust input
- [I3] Diagnostics vector is empty when no violations are found

## Error Taxonomy
- `LintError::ParseError(String)` - when source cannot be parsed by syn

## Contract Signatures
```rust
pub fn lint_workflow_code(source: &str) -> Result<Vec<Diagnostic>, LintError>
```

## Type Encoding
| Precondition | Enforcement Level | Type / Pattern |
|---|---|---|
| source is valid Rust | Runtime-checked | `syn::parse_file()` returns Result |
| workflow function detection | Runtime | Custom visitor pattern with `in_workflow_fn` flag |
| std::thread::spawn detection | Compile-time | Path segments matching |

## Violation Examples (REQUIRED)
- VIOLATES Q1: `std::thread::spawn(|| { println!("bg"); })` inside workflow execute -- should produce `Err(LintError::Diagnostic(LintCode::L006))`
- VIOLATES Q2: `std::thread::spawn(|| {})` inside a non-workflow async fn -- should produce `Ok([])` (no diagnostic)
- VIOLATES Q3: `async fn helper() { std::thread::spawn(|| {}); }` -- should produce `Ok([])` (no diagnostic)
- VIOLATES Q4: Two `std::thread::spawn` calls in same workflow -- should produce `Ok([Diagnostic, Diagnostic])`
- VIOLATES Q5: `std::thread::spawn` inside closure inside workflow -- should produce `Err(LintError::Diagnostic(LintCode::L006))`

## Ownership Contracts
- No ownership transfer occurs; all inputs are borrowed (`&str`)
- No `&mut` parameters

## Non-goals
- Detecting `tokio::spawn` (handled by L005)
- Detecting other threading primitives like `std::thread::Builder::new().spawn()`
- Modifying or fixing the source code
