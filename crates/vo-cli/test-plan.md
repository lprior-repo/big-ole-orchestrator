# vo-cli Command Edge Case Coverage Test Plan

## Context
- **Issue**: ve-li2n - TDD Red: vo-cli command coverage
- **Current**: 188 tests in vo-cli
- **Gap**: Command edge cases need deeper coverage

## Test Categories

### 1. Invalid Flag Combinations

| Command | Edge Case | Expected Behavior |
|---------|-----------|-------------------|
| `gc` | `--dry-run --engine-url` with invalid URL | Reject malformed URL |
| `gc` | Unknown flags like `--unknown-flag` | Clap error |
| `check` | Pass multiple paths | Clap error (expects single path) |
| `init` | Both `--project-dir .` and positional | Use first or error |
| `purge` | `--instance` with empty string | Reject empty instance ID |
| `history` | `--instance` with empty string | Reject empty instance ID |

### 2. Missing Required Arguments

| Command | Missing Arg | Expected Clap Error |
|---------|-------------|---------------------|
| `purge` | `--instance` | MissingRequiredArgument |
| `check` | `<path>` | MissingRequiredArgument |
| `history` | `--instance` | MissingRequiredArgument |
| `gc` | None (all optional) | OK with defaults |

### 3. Concurrent Command Execution

- [ ] `vo gc --dry-run` + `vo check <file>` simultaneously
- [ ] Multiple `vo gc --dry-run` racing
- [ ] `vo init` while `vo lock` running
- [ ] Verify no shared state corruption

### 4. Output Format Consistency

| Command | Consistency Check |
|---------|-------------------|
| `check` on ELF | Output: `<path>: valid ELF binary` |
| `check` on MachO | Output: `<path>: valid Mach-O 64-bit binary` |
| `check` multiple runs | Same output for same binary |
| `gc --dry-run` | Output format stable across runs |

## Test File Structure

```
crates/vo-cli/tests/
├── edge_case_tests.rs       # Invalid flags + missing args
├── concurrent_tests.rs      # Concurrent execution
└── output_format_tests.rs   # Output consistency
```

## Implementation Notes

### Edge Case Tests (unit_tests.rs additions)
```rust
// Invalid flag combinations
interpret_cli_from(vec!["vo", "gc", "--dry-run", "--unknown-flag"])
interpret_cli_from(vec!["vo", "check", "/bin/ls", "/bin/bash"])

// Missing required args
interpret_cli_from(vec!["vo", "purge"])  // Missing --instance
interpret_cli_from(vec!["vo", "history"]) // Missing --instance

// Invalid values
interpret_cli_from(vec!["vo", "purge", "--instance", ""])
interpret_cli_from(vec!["vo", "history", "--instance", ""])
```

### Concurrent Tests
- Use `tokio::join!` or `tokio::spawn` for concurrency
- Verify each command completes successfully
- Check for data races or shared state issues

### Output Format Tests
- Parse stdout and verify format regex
- Run same command 3x and compare output strings
