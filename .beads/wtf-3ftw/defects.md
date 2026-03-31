# Defects: wtf-3ftw

## DEFECT-1: Missing initial_state field [MAJOR]
- **Spec violation**: Section 0 clarifications state "initial_state is optional — if missing, no initial state is enforced"
- **Problem**: No `initial_state` field in FsmGraph or FsmDefinition
- **Fix**: Add `initial_state: Option<String>` to FsmGraph, parse it, store in FsmDefinition

## DEFECT-2: File exceeds 300 lines [MINOR]
- **Problem**: definition.rs is 405 lines (210 tests)
- **Fix**: Extract tests to definition_tests.rs
