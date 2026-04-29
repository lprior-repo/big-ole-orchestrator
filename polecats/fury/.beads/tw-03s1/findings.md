# Bead tw-03s1 Findings: unwrap_in_prod Linter Rule

## Implementation Summary

Created new linter rule `L003` to detect `.unwrap()` and `.expect()` calls in production code.

### Files Created

1. `crates/vo-linter/src/rules/unwrap_in_prod/mod.rs` - Module definition
2. `crates/vo-linter/src/rules/unwrap_in_prod/detector.rs` - Detection logic using syn::Visit
3. `crates/vo-linter/src/rules/unwrap_in_prod/rule.rs` - Rule struct implementing Rule trait
4. `crates/vo-linter/src/rules/unwrap_in_prod/tests.rs` - 45 test cases

### Files Modified

1. `crates/vo-linter/src/diagnostic.rs` - Added L003 to LintCode enum
2. `crates/vo-linter/src/rules/mod.rs` - Integrated rule into registry

### Rule Behavior

- **Detects**: `.unwrap()` and `.expect()` method calls in production functions
- **Whitelists**: Functions named `test_*` or with `#[test]` attribute
- **Severity**: ERROR
- **Suggestion**: "handle the Result/Option explicitly with ? or match"

### Key Implementation Details

- Uses `syn::visit::Visit` pattern matching the existing `random` rule
- Tracks `in_test_function` state during AST traversal
- Detects both function call syntax `func.unwrap()` and method call syntax `result.unwrap()`

### Test Results

All 121 tests pass (including 45 new unwrap_in_prod tests).

### Violation Counts by Crate

| Crate | unwrap() Count |
|-------|---------------|
| vo-actor | 551 |
| vo-api | 90 |
| vo-cli | 59 |
| vo-common | 2 |
| vo-core | 961 |
| vo-executor | 49 |
| vo-frontend | 45 |
| vo-ipc | 96 |
| vo-linter | 28 |
| vo-scheduler | 118 |
| vo-sdk | 130 |
| vo-sdk-macros | 29 |
| vo-storage | 2033 |
| vo-types | 1523 |
| vo-worker | 89 |
| **TOTAL** | **5803** |

### Notes

- The linter only detects `.unwrap()` and `.expect()` at the AST level - macro-expanded calls inside `macros.rs` files may not be detected
- Test functions are correctly whitelisted by name prefix (`test_*`) or `#[test]` attribute
- The 28 violations in vo-linter itself are in non-test code (detector.rs, rule.rs, mod.rs use `syn::parse_str(src).unwrap()` for test parsing)