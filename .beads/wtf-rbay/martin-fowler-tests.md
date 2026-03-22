# Martin Fowler Test Plan: WTF-L006

## Happy Path Tests
- `test_emits_no_diagnostic_for_code_without_thread_spawn` — workflow with no spawn calls returns empty diagnostics
- `test_emits_no_diagnostic_for_helper_function` — `std::thread::spawn` in non-workflow async fn is ignored

## Error Path Tests
- `test_returns_parse_error_for_invalid_rust` — invalid Rust syntax returns LintError::ParseError
- `test_violation_std_thread_spawn_in_workflow_execute` — std::thread::spawn in workflow emits L006 diagnostic
- `test_violation_nested_std_thread_spawn_in_closure` — spawn inside closure within workflow is detected
- `test_violation_std_thread_spawn_in_if_branch` — spawn inside conditional branch is detected

## Edge Case Tests
- `test_handles_empty_source` — empty input returns empty diagnostics (not error)
- `test_no_false_positive_outside_workflow` — spawn in regular async fn is not flagged
- `test_no_false_positive_different_thread_spawn` — other std::thread variants are not flagged
- `test_no_false_positive_tokio_spawn` — tokio::spawn should not trigger L006 (L005 only)
- `test_multiple_std_thread_spawns_in_same_workflow` — each violation emits separate diagnostic

## Contract Verification Tests
- `test_diagnostic_contains_correct_lint_code` — emitted diagnostic.code == LintCode::L006
- `test_diagnostic_contains_suggestion` — emitted diagnostic has suggestion field set
- `test_diagnostic_message_describes_violation` — message mentions std::thread::spawn and workflow function

## Contract Violation Tests
- `test_precondition_p1_violation_invalid_syntax_returns_parse_error`
  Given: `async fn workflow { // missing parens`
  When: `lint_workflow_code(source)` is called
  Then: returns `Err(LintError::ParseError(_))` — NOT a panic

- `test_postcondition_q1_violation_emits_diagnostic`
  Given: `impl WorkflowFn for MyWorkflow { async fn execute(&self) { std::thread::spawn(|| {}); } }`
  When: `lint_workflow_code(source)` is called
  Then: returns `Ok(diagnostics)` where `diagnostics[0].code == LintCode::L006`

- `test_postcondition_q2_violation_no_diagnostic_outside_workflow`
  Given: `async fn helper() { std::thread::spawn(|| {}); }`
  When: `lint_workflow_code(source)` is called
  Then: returns `Ok(diagnostics)` where `diagnostics.is_empty()`

## Given-When-Then Scenarios

### Scenario 1: Clean workflow code passes lint
Given: A workflow impl with `async fn execute` that uses only ctx.activity() calls
When: `lint_workflow_code` is called
Then: Returns `Ok(diagnostics)` with empty diagnostics vector

### Scenario 2: std::thread::spawn in workflow is detected
Given: A workflow impl with `async fn execute` containing `std::thread::spawn(|| { do_work(); })`
When: `lint_workflow_code` is called
Then: Returns `Ok([Diagnostic { code: L006, message: contains("std::thread::spawn"), suggestion: Some(_) }])`

### Scenario 3: Nested spawn in closure is detected
Given: A workflow impl with `async fn execute` containing `some_vec.iter().for_each(|_| { std::thread::spawn(|| {}); })`
When: `lint_workflow_code` is called
Then: Returns `Ok([Diagnostic { code: L006, ... }])`

### Scenario 4: Invalid Rust returns ParseError
Given: Source code that is not valid Rust syntax
When: `lint_workflow_code` is called
Then: Returns `Err(LintError::ParseError(_))`

### Scenario 5: Multiple violations each get their own diagnostic
Given: A workflow impl with two separate `std::thread::spawn` calls
When: `lint_workflow_code` is called
Then: Returns `Ok(diagnostics)` where `diagnostics.len() == 2`
