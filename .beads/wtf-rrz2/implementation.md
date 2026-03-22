# Implementation Summary: wtf-linter AST Walker + Diagnostic Infrastructure

## Files Changed

### crates/wtf-linter/src/lib.rs
- Added `lint_workflow_source(source: &str) -> Result<LintResult, LintError>` entry point
- Parses source using `syn::parse_file()`
- Calls `visitor::walk_workflow_functions()` to find workflow functions and run visitors
- Determines `has_errors` based on diagnostics present

### crates/wtf-linter/src/diagnostic.rs
- `LintDiagnostic`: `code: String, message: String, file: String, line: u32, col: u32, suggestion: String`
- `LintResult`: `diagnostics: Vec<LintDiagnostic>, has_errors: bool`
- `LintError::ParseError(String)`: returned when syn fails to parse
- `LintDiagnostic::is_error()`: returns true if diagnostic represents an error (not L004 warning)

### crates/wtf-linter/src/visitor.rs
- `LintVisitor` trait: `fn check(&self, fn_body: &syn::Block, diagnostics: &mut Vec<LintDiagnostic>)`
- `WorkflowFinder` struct: syn::Visit implementation that walks AST
- `is_workflow_fn()`: identifies workflow functions by `#[workflow]` attribute or `_workflow` suffix
- `walk_workflow_functions()`: entry point that instantiates visitor and walks file

## Contract Compliance

| Contract Clause | Status |
|---|---|
| P1: valid UTF-8 | ✅ `&str` guarantees |
| P2: parseable Rust | ✅ `Result<LintResult, LintError::ParseError>` |
| Q1: all diagnostics collected | ✅ Visitors called, diagnostics accumulated |
| Q2: has_errors reflects severity | ✅ Checks `LintDiagnostic::is_error()` |
| Q3: each workflow visited once | ✅ `syn::visit::visit_item_fn` continues walking |
| Q4: accurate line/col positions | ✅ Span tracking from syn |

## Notes
- Individual lint rules (L001-L006) will be implemented in separate beads
- Current implementation provides the infrastructure; rules array is empty
- When rules are added, they will implement `LintVisitor` and be passed to `walk_workflow_functions`

## Iteration 2 (infrastructure parity updates)

- Updated `LintResult::new()` semantics in `crates/wtf-linter/src/lib.rs`:
  - `has_errors` now reflects error-severity diagnostics only (warning-only sets `false`).
- Added diagnostics deduplication by `(code, span)` prior to building `LintResult`.
- Added infra tests in `lib.rs` and `engine.rs`:
  - warning-only `has_errors == false`
  - error diagnostic `has_errors == true`
  - invalid source returns parse error
  - empty source handled successfully

- Verification:
  - `cargo test -p wtf-linter -- --nocapture`
