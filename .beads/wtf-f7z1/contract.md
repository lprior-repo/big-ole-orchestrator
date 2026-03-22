# Contract Specification: wtf lint

## Context
- Feature: CLI command `wtf lint` that invokes wtf-linter to validate workflow definitions
- Domain terms:
  - `wtf lint` - CLI subcommand for linting workflow files
  - LintCode (L001-L006) - rule codes for non-deterministic behavior detection
  - Diagnostic - a lint violation with code, severity, message, suggestion, and source span
  - LintError - parse-level errors (distinct from lint violations)
- Assumptions:
  - Linter already has Diagnostic, LintCode, Severity types implemented
  - Rules are stubs (L001-L006 exist as enums but rules not yet implemented)
  - This bead only implements the CLI wiring, not the actual lint rules
- Open questions:
  - How does the linter accept file paths vs directories?
  - What is the exact API between CLI and linter crate?

## Preconditions
- [P1] Input paths must be valid file or directory paths (enforced at CLI layer via clap)
- [P2] Files must have .rs extension when filtering (or user explicitly opts into all files)
- [P3] Directory paths must be traversable (io::ReadDir must succeed)

## Postconditions
- [Q1] Command returns exit code 0 when no lint violations or parse errors are found
- [Q2] Command returns exit code 1 when one or more lint violations are found
- [Q3] Command returns exit code 2 when a parse error occurs (cannot analyze file)
- [Q4] All diagnostics are written to stdout in specified format (JSON or human-readable)
- [Q5] No diagnostics are lost (all found violations are reported)
- [Q6] Progress/errors are written to stderr

## Invariants
- [I1] The CLI never panics (all errors are handled and reported as proper exit codes)
- [I2] Output format is consistent (either all-JSON or all-human-readable, never mixed)

## Error Taxonomy
- Error::ParseError(String) - when syn fails to parse a .rs file (LintError variant)
- Error::IoError(String) - when file/directory cannot be read
- Error::NoFilesFound - when glob patterns match no files
- Error::InvalidFormat(String) - when --format value is unrecognized

## Contract Signatures
```rust
// Primary entry point - CLI to linter bridge
fn run_lint(matches: &ArgMatches) -> Result<ExitCode, CliError>

// Linter API (expected interface)
fn lint_file(path: &Path) -> Result<Vec<Diagnostic>, LintError>
fn lint_directory(path: &Path) -> Result<Vec<Diagnostic>, LintError>
```

## Type Encoding
| Precondition | Enforcement Level | Type / Pattern |
|---|---|---|
| P1: Valid paths | Compile-time | `clap(validator)` via `PathBuf` |
| P2: .rs extension | Runtime-checked | `path.extension() == Some("rs")` |
| P3: Traversable dirs | Runtime-checked | `std::fs::read_dir()` Result |

## Violation Examples (REQUIRED)
- VIOLATES Q1: `wtf lint valid_file.rs` where file has lint violations → returns exit code 1, NOT 0
- VIOLATES Q2: `wtf lint valid_file.rs` where file has NO lint violations → returns exit code 0, NOT 1
- VIOLATES Q3: `wtf lint syntax_error.rs` where file has parse error → returns exit code 2, NOT 0 or 1
- VIOLATES Q4: Running with `--format json` → stdout contains valid JSON array, NOT human-readable
- VIOLATES Q5: File with 3 violations → exactly 3 diagnostics printed, NOT 1 or 2

## Ownership Contracts
- `run_lint` takes `&ArgMatches` (shared borrow, no mutation)
- File contents are read into memory transiently (no ownership transfer)
- Diagnostics are cloned from linter output (ownership remains with linter)

## Non-goals
- Implementing the actual lint rules (L001-L006) - those are separate beads
- Modifying the linter's internal AST visitor architecture
- Adding new lint rules

---

## Scope Map
| Item | Location |
|---|---|
| CLI command definition | `wtf-cli/src/main.rs` (to be created) or `wtf-cli/src/lib.rs` |
| Linter bridge logic | `wtf-cli/src/lint.rs` (new module) |
| Diagnostic output | stdout (controlled by --format flag) |
| Progress/errors | stderr |

## Traceability
- Parent epic: `wtf-5so3` (epic: Phase 7 — CLI + Integration)
- Depends on: `wtf-linter` crate existing with Diagnostic, LintCode, Severity types
