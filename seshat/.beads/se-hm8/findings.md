# se-hm8 Findings - Red Queen Test Quality

## Issue
Red Queen checker flagged `inp_mobile_touch_tests.rs` in prime polecat: `if let InteractionMode::Panning` blocks without `assert_eq`.

## Root Cause
Two test functions used `assert!` instead of `assert_eq!` for boolean predicate checks:
- `given_panning_mode_with_nan_coords_then_mode_constructs_without_panic` - used `assert!` for `is_nan()` checks
- `given_panning_mode_with_infinity_coords_then_mode_constructs_without_panic` - used `assert!` for `is_infinite()`/`is_sign_positive()`/`is_sign_negative()` checks

## Fix Applied
Converted `assert!` to `assert_eq!` for boolean predicates:
- `assert!(x.is_nan())` → `assert_eq!(x.is_nan(), true, "...")`
- `assert!(x.is_infinite() && x.is_sign_positive())` → `assert_eq!(x.is_infinite(), true, "...")` + `assert_eq!(x.is_sign_positive(), true, "...")`
- `assert!(!x.is_sign_positive())` → `assert_eq!(x.is_sign_positive(), false, "...")`

## Verification
- Red Queen check passes: `rg -A2 'if let InteractionMode::Panning' ... | grep -q 'assert_eq'`
- All 19 mobile touch tests compile and pass
- No other polecats affected (they all have 1 panning block that already uses `assert_eq`)
