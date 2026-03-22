# Martin Fowler Test Plan: wtf lint

## Test Naming Convention
Tests follow `test_<scenario>_<expected_outcome>` pattern per Dave Farley/Dan North BDD style.

## Happy Path Tests

### test_lint_valid_file_returns_zero_exit_code
Given: A valid Rust workflow file with no lint violations
When: `wtf lint <path-to-valid-file>` is executed
Then: Exit code is 0

### test_lint_valid_directory_returns_zero_when_all_files_clean
Given: A directory containing only valid Rust workflow files with no lint violations
When: `wtf lint <path-to-directory>` is executed
Then: Exit code is 0

### test_lint_single_file_outputs_diagnostics_to_stdout
Given: A Rust file with lint violations
When: `wtf lint <path-to-file>` is executed
Then: Diagnostics are written to stdout

## Error Path Tests

### test_lint_file_with_violations_returns_exit_code_1
Given: A Rust workflow file that triggers L001-L006 violations
When: `wtf lint <path-to-violating-file>` is executed
Then: Exit code is 1

### test_lint_file_with_parse_error_returns_exit_code_2
Given: A file with syntax errors that cannot be parsed
When: `wtf lint <path-to-syntax-error-file>` is executed
Then: Exit code is 2

### test_lint_nonexistent_file_returns_error
Given: A path that does not exist
When: `wtf lint <nonexistent-path>` is executed
Then: Exit code is non-zero with error message on stderr

### test_lint_directory_with_mixed_results_returns_aggregate_exit_code
Given: Directory with some valid and some invalid files
When: `wtf lint <path-to-mixed-directory>` is executed
Then: Exit code is 1 (at least one violation found)

## Edge Case Tests

### test_lint_empty_directory_returns_zero
Given: An empty directory
When: `wtf lint <empty-dir>` is executed
Then: Exit code is 0, no diagnostics output

### test_lint_file_with_only_whitespace_returns_parse_error
Given: A .rs file containing only whitespace
When: `wtf lint <whitespace-file>` is executed
Then: Exit code is 2 (parse error)

### test_lint_non_rust_file_ignored_by_default
Given: A directory containing both .rs and .txt files
When: `wtf lint <directory>` is executed without special flags
Then: Only .rs files are analyzed

### test_lint_symlink_resolved
Given: A symlink pointing to a valid Rust file
When: `wtf lint <symlink-path>` is executed
Then: The target file is linted

### test_lint_nested_directory_recurses
Given: A directory tree with .rs files at multiple depths
When: `wtf lint <root-directory>` is executed
Then: All .rs files in all subdirectories are linted

## Format Tests

### test_lint_json_format_produces_valid_json_array
Given: A file with lint violations
When: `wtf lint --format json <path>` is executed
Then: stdout is a valid JSON array of diagnostic objects

### test_lint_human_format_produces_readable_output
Given: A file with lint violations
When: `wtf lint --format human <path>` is executed
Then: stdout contains severity, code, and message in human-readable form

### test_lint_no_violations_json_empty_array
Given: A valid file with no lint violations
When: `wtf lint --format json <path>` is executed
Then: stdout is `[]` (empty JSON array)

## Contract Verification Tests

### test_precondition_path_exists
Given: An invalid path
When: CLI argument parsing occurs
Then: Error: "path does not exist"

### test_postcondition_exit_code_consistent_with_results
Given: Files with known violation counts
When: Exit codes are collected
Then: exit_code = 0 iff total_violations == 0 AND total_parse_errors == 0
Then: exit_code = 1 iff total_violations > 0 AND total_parse_errors == 0
Then: exit_code = 2 iff total_parse_errors > 0

### test_invariant_no_panic_on_corrupt_files
Given: A binary file named .rs
When: `wtf lint <binary-file.rs>` is executed
Then: Process exits gracefully with parse error, not panics

### test_invariant_output_format_not_mixed
Given: `--format json` flag is used
When: Output is produced
Then: No human-readable text appears in stdout

## Given-When-Then Scenarios

### Scenario 1: Lint a workflow file with violations
Given: A workflow definition file at `./workflows/parse_orders.rs` containing a `time::now()` call (L001 violation)
And: `wtf lint --format human ./workflows/parse_orders.rs` is invoked
When: The linter runs
Then: Output contains "error[WTF-L001]: Non-deterministic time call detected"
And: Exit code is 1

### Scenario 2: Lint multiple files in directory
Given: Directory `./workflows/` containing 3 .rs files
And: File1.rs has 0 violations, File2.rs has 1 L002 violation, File3.rs has 2 L003 violations
When: `wtf lint ./workflows/` is invoked
Then: All 3 diagnostics are reported
And: Exit code is 1
And: Summary shows "3 violations found"

### Scenario 3: JSON output for machine consumption
Given: A file with violations
When: `wtf lint --format json <path>` is executed
Then: Each diagnostic object contains: `{"code": "WTF-L001", "severity": "error", "message": "...", "span": [start, end], "suggestion": "..."}`
And: Output is valid JSON (parseable by jq)

## Edge Cases for Coverage

### test_lint_permission_denied
Given: A file with no read permissions
When: `wtf lint <file>` is executed
Then: Error message on stderr, exit code 1 or 2 (not 0)

### test_lint_file_larger_than_memory
Given: A very large .rs file (>1GB)
When: Linting is attempted
Then: Returns error gracefully (not OOM panic)

### test_lint_concurrent_file_access
Given: Files being modified during linting
When: `wtf lint` runs
Then: Reports current state at scan time, no race conditions

---

## Exit Code Mapping
| Condition | Exit Code |
|---|---|
| No violations, no parse errors | 0 |
| One or more violations, no parse errors | 1 |
| One or more parse errors (regardless of violations) | 2 |
| CLI argument error | 1 |

## Output Format Specification

### Human Format
```
error[WTF-L001]: Non-deterministic time call in workflow function
  --> src/workflows/orders.rs:15:5
  = note: suggestion: use a deterministic time source
warning[WTF-L004]: ctx.call inside closure with non-deterministic dispatch
  --> src/workflows/orders.rs:42:10
```

### JSON Format
```json
[
  {
    "code": "WTF-L001",
    "severity": "error",
    "message": "Non-deterministic time call in workflow function",
    "span": {"start": 142, "end": 155},
    "suggestion": "use a deterministic time source",
    "file": "src/workflows/orders.rs"
  }
]
```
