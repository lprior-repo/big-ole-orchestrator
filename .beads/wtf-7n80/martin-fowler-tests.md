# Martin Fowler Test Plan

## Happy Path Tests
- test_copies_ui_module_successfully
- test_copies_graph_module_successfully
- test_copies_linter_module_successfully
- test_creates_wtf_client_placeholder
- test_re_exports_ui_graph_wtf_client_in_lib
- test_dioxus_features_web_desktop_router_enabled
- test_project_compiles_without_errors
- test_no_restate_references_in_dependency_tree

## Error Path Tests
- test_returns_error_when_parent_dir_missing
- test_returns_error_when_restate_reference_detected
- test_returns_error_when_compilation_fails

## Edge Case Tests
- test_handles_empty_ui_directory
- test_handles_nested_directory_structure
- test_preserves_file_permissions

## Contract Verification Tests
- test_precondition_parent_dir_exists
- test_precondition_source_dirs_exist
- test_postcondition_all_modules_copied
- test_postcondition_no_restate_references
- test_invariant_module_structure_preserved

## Given-When-Then Scenarios

### Scenario 1: Successful Module Copy
Given: Parent directory crates/wtf-frontend/ exists
And: Source directories exist in /home/lewis/src/oya-frontend/src/
When: setup_wtf_frontend() is called
Then: All modules are copied successfully
And: lib.rs is updated with re-exports
And: Returns Ok(())

### Scenario 2: Restate Reference Detection
Given: Oya source files contain Restate types
When: setup_wtf_frontend() copies files
Then: Files are copied but flagged
And: Final state has no Restate references in wtf-frontend

### Scenario 3: Compilation Verification
Given: All files copied and configured
When: cargo build is run
Then: Build succeeds with no errors
And: No Restate dependencies are compiled

## End-to-End Scenario
Given: Fresh wtf-engine repository
When: Bead wtf-7n80 is applied
Then: crates/wtf-frontend/ is fully set up
And: cargo check passes in wtf-frontend
And: grep -r "restate" crates/wtf-frontend/ returns no results
