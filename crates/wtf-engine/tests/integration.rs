//! Integration tests for wtf-engine BinaryRegistry.
//!
//! All tests use real filesystem operations, real subprocess invocations,
//! and real SHA-256 hashing. No mocks.

mod common;

use std::os::unix::fs::PermissionsExt;

use common::*;
use wtf_engine::*;

// ===========================================================================
// BinaryRegistry::register — happy path (B-REG-14..21)
// ===========================================================================

// B-REG-14
#[test]
fn register_stores_versioned_binary_when_source_binary_exists_and_supports_graph() {
    // Given
    let (temp_dir, registry) = create_test_registry();
    let source = make_test_binary(temp_dir.path(), &valid_single_node_graph());
    let source_path = bp(&source);
    let name = wn("deploy-prod");

    // When
    let result = registry.register(&source_path, name.clone());

    // Then
    assert_eq!(result, Ok(()));
    let resolved = registry.resolve(&name).expect("resolved");
    assert!(resolved.0.as_path().starts_with(temp_dir.path()));
    assert!(std::fs::metadata(resolved.0.as_path()).is_ok());
}

// B-REG-15
#[test]
fn register_computes_correct_sha256_hash_when_binary_is_hashed() {
    use sha2::{Digest, Sha256};

    // Given
    let (temp_dir, registry) = create_test_registry();
    let source = make_test_binary(temp_dir.path(), &valid_single_node_graph());
    let source_path = bp(&source);
    let name = wn("hash-test");

    // Compute expected hash from the source file content
    let source_content = std::fs::read(&source).expect("read source");
    let expected_hash = format!("{:x}", Sha256::digest(&source_content));

    // When
    let result = registry.register(&source_path, name.clone());

    // Then
    assert_eq!(result, Ok(()));
    let (_, binary_hash, _) = registry.resolve(&name).expect("resolved");
    assert_eq!(binary_hash.as_str(), expected_hash);
}

// B-REG-16
#[test]
fn register_copies_binary_to_versions_dir_when_hash_directory_does_not_exist() {
    // Given
    let (temp_dir, registry) = create_test_registry();
    let source = make_test_binary(temp_dir.path(), &valid_single_node_graph());
    let source_path = bp(&source);
    let name = wn("copy-test");

    // When
    registry.register(&source_path, name).expect("register");

    // Then: versioned file exists with identical content
    let (_, _, _) = registry.resolve(&wn("copy-test")).expect("resolve");
    let resolved = registry.resolve(&wn("copy-test")).expect("resolve");
    let source_content = std::fs::read(source_path.as_path()).expect("read source");
    let versioned_content = std::fs::read(resolved.0.as_path()).expect("read versioned");
    assert_eq!(source_content, versioned_content);
}

// B-REG-17
#[test]
fn register_skips_copy_when_hash_directory_already_exists() {
    use sha2::{Digest, Sha256};

    // Given
    let (temp_dir, registry) = create_test_registry();
    let source = make_test_binary(temp_dir.path(), &valid_single_node_graph());
    let source_path = bp(&source);

    // Compute hash and pre-create the versioned path with the same binary content
    // (same hash = same content). Mark it executable so --graph can run on it.
    let source_content = std::fs::read(&source).expect("read source");
    let hash_hex = format!("{:x}", Sha256::digest(&source_content));
    let versioned_path = temp_dir.path().join(&hash_hex);
    std::fs::create_dir_all(versioned_path.parent().expect("parent")).expect("mkdir");
    std::fs::write(&versioned_path, &source_content).expect("write original");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&versioned_path)
            .expect("metadata")
            .permissions();
        perms.set_mode(perms.mode() | 0o755);
        std::fs::set_permissions(&versioned_path, perms).expect("chmod");
    }

    let name = wn("collision-test");

    // When
    let result = registry.register(&source_path, name);

    // Then
    assert_eq!(result, Ok(()));
    // Verify the copy was skipped — content is unchanged from what we wrote
    let content = std::fs::read(&versioned_path).expect("read");
    assert_eq!(content, source_content);
}

// B-REG-18
#[test]
fn register_parses_graph_stdout_as_workflow_definition_with_exact_node_count_when_output_is_valid_json(
) {
    // Given
    let (temp_dir, registry) = create_test_registry();
    let source = make_test_binary(temp_dir.path(), &valid_three_node_graph());
    let source_path = bp(&source);
    let name = wn("my-workflow");

    // When
    let result = registry.register(&source_path, name.clone());

    // Then
    assert_eq!(result, Ok(()));
    let (_, _, definition) = registry.resolve(&name).expect("resolved");
    assert_eq!(definition.nodes.len(), 3);
    assert_eq!(definition.workflow_name, wn("my-workflow"));
}

// B-REG-19
#[test]
fn register_sets_status_to_active_when_registration_succeeds() {
    // Given
    let (temp_dir, registry) = create_test_registry();
    let source = make_test_binary(temp_dir.path(), &valid_single_node_graph());
    let source_path = bp(&source);
    let name = wn("status-test");

    // When
    registry.register(&source_path, name).expect("register");

    // Then
    let entries = registry.list();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].1.status, RegistrationStatus::Active);
}

// B-REG-20
#[test]
fn register_replaces_existing_registration_with_exact_new_hash_when_same_workflow_re_registered() {
    use sha2::{Digest, Sha256};

    // Given
    let (temp_dir, registry) = create_test_registry();
    let source1 = make_test_binary(temp_dir.path(), &valid_single_node_graph());
    let source1_path = bp(&source1);
    let name = wn("deploy-prod");

    registry
        .register(&source1_path, name.clone())
        .expect("first register");

    let (_, old_hash, _) = registry.resolve(&name).expect("resolve old");
    let old_hash_str = old_hash.as_str().to_string();

    // Create a DIFFERENT binary with a different hash
    let source2 = make_test_binary(temp_dir.path(), &valid_two_node_graph());
    let source2_path = bp(&source2);

    let source2_content = std::fs::read(&source2).expect("read");
    let new_hash_str = format!("{:x}", Sha256::digest(&source2_content));
    assert_ne!(old_hash_str, new_hash_str);

    // When
    let result = registry.register(&source2_path, name.clone());

    // Then
    assert_eq!(result, Ok(()));
    let (versioned_path, new_hash, _) = registry.resolve(&name).expect("resolve new");
    assert_eq!(new_hash.as_str(), new_hash_str);
    assert!(versioned_path
        .as_path()
        .to_string_lossy()
        .contains(&new_hash_str));
    // Old versioned binary still exists on disk
    let old_versioned = temp_dir.path().join(&old_hash_str);
    assert!(std::fs::metadata(&old_versioned).is_ok());
}

// B-REG-21
#[test]
fn register_returns_ok_when_all_steps_succeed() {
    // Given
    let (temp_dir, registry) = create_test_registry();
    let source = make_test_binary(temp_dir.path(), &valid_single_node_graph());
    let source_path = bp(&source);
    let name = wn("ok-test");

    // When
    let result = registry.register(&source_path, name);

    // Then
    assert_eq!(result, Ok(()));
}

// ===========================================================================
// BinaryRegistry::register — error paths (B-REG-22..30)
// ===========================================================================

// B-REG-22
#[test]
fn register_returns_binary_not_found_when_source_binary_does_not_exist() {
    // Given
    let (temp_dir, registry) = create_test_registry();
    let missing = temp_dir.path().join("does-not-exist");
    let source_path = bp(&missing);
    let name = wn("missing");

    // When
    let result = registry.register(&source_path, name);

    // Then
    assert!(matches!(
        result,
        Err(BinaryRegistryError::BinaryNotFound { .. })
    ));
    assert_eq!(registry.len(), 0);
}

// B-REG-23
#[test]
fn register_returns_not_executable_when_source_binary_lacks_execute_permission() {
    // Given
    let (temp_dir, registry) = create_test_registry();
    let non_exec = temp_dir.path().join("non-executable");
    std::fs::write(&non_exec, "#!/bin/sh\nexit 1").expect("write");
    std::fs::set_permissions(&non_exec, std::fs::Permissions::from_mode(0o644)).expect("chmod");
    let source_path = bp(&non_exec);
    let name = wn("noexec");

    // When
    let result = registry.register(&source_path, name);

    // Then
    assert!(matches!(
        result,
        Err(BinaryRegistryError::NotExecutable { .. })
    ));
    assert_eq!(registry.len(), 0);
}

// B-REG-24
#[test]
fn register_returns_hash_failed_with_exact_variant_when_read_permission_revoked() {
    // Given
    let (temp_dir, registry) = create_test_registry();
    let source = make_test_binary(temp_dir.path(), &valid_single_node_graph());
    let source_path = bp(&source);
    let name = wn("hash-fail");

    // chmod 0o100: execute-only. Passes executable check but fails read (hash).
    std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o100))
        .expect("chmod execute-only");

    // When
    let result = registry.register(&source_path, name);

    // Then: must be HashFailed exclusively
    assert!(matches!(
        result,
        Err(BinaryRegistryError::HashFailed { .. })
    ));
    assert_eq!(registry.len(), 0);
}

// B-REG-25
#[test]
fn register_returns_copy_failed_with_io_error_source_when_file_copy_to_versions_dir_fails() {
    // Given: versions_dir points to a non-existent parent that can't be created
    // Use a read-only parent directory
    let outer = tempfile::TempDir::new().expect("temp dir");
    let readonly_parent = outer.path().join("readonly");
    std::fs::create_dir_all(&readonly_parent).expect("mkdir");
    std::fs::set_permissions(&readonly_parent, std::fs::Permissions::from_mode(0o555))
        .expect("chmod readonly");

    let versions_dir = bp(&readonly_parent.join("versions"));
    let registry = BinaryRegistry::new(versions_dir).expect("registry");

    let source = make_test_binary(outer.path(), &valid_single_node_graph());
    let source_path = bp(&source);
    let name = wn("copy-fail");

    // When
    let result = registry.register(&source_path, name);

    // Then
    assert!(matches!(
        result,
        Err(BinaryRegistryError::CopyFailed { source: _, .. })
    ));
    assert_eq!(registry.len(), 0);
}

// B-REG-26
#[test]
fn register_returns_graph_discovery_failed_when_graph_exits_non_zero() {
    // Given
    let (temp_dir, registry) = create_test_registry();
    let source = make_test_binary_graph_fail(temp_dir.path(), 1, "graph error");
    let source_path = bp(&source);
    let name = wn("graph-fail");

    // When
    let result = registry.register(&source_path, name.clone());

    // Then
    assert!(matches!(
        result,
        Err(BinaryRegistryError::GraphDiscoveryFailed {
            ref workflow_name,
            exit_code,
            ref stderr
        }) if workflow_name.as_str() == "graph-fail" && exit_code == 1 && stderr.contains("graph error")
    ));
    assert_eq!(registry.len(), 0);
}

// B-REG-27
#[test]
fn register_returns_invalid_graph_output_when_graph_stdout_is_not_valid_json() {
    // Given
    let (temp_dir, registry) = create_test_registry();
    let source = make_test_binary_invalid_json(temp_dir.path(), "not json at all");
    let source_path = bp(&source);
    let name = wn("bad-json");

    // When
    let result = registry.register(&source_path, name.clone());

    // Then
    assert!(matches!(
        result,
        Err(BinaryRegistryError::InvalidGraphOutput {
            ref workflow_name,
            ref parse_error
        }) if workflow_name.as_str() == "bad-json" && !parse_error.is_empty()
    ));
    assert_eq!(registry.len(), 0);
}

// B-REG-28
#[test]
fn register_returns_workflow_definition_invalid_when_graph_output_fails_structural_validation() {
    // Given
    let (temp_dir, registry) = create_test_registry();
    let source = make_test_binary(temp_dir.path(), &graph_with_empty_nodes());
    let source_path = bp(&source);
    let name = wn("empty-def");

    // When
    let result = registry.register(&source_path, name.clone());

    // Then
    assert!(matches!(
        result,
        Err(BinaryRegistryError::WorkflowDefinitionInvalid {
            ref workflow_name,
            ref reason
        }) if workflow_name.as_str() == "empty-def" && !reason.is_empty()
    ));
    assert_eq!(registry.len(), 0);
}

// B-REG-29
#[test]
fn register_does_not_insert_entry_when_any_step_fails() {
    // Given
    let (temp_dir, registry) = create_test_registry();
    let missing = temp_dir.path().join("nonexistent");
    let source_path = bp(&missing);
    let name = wn("no-insert");

    // When
    let result = registry.register(&source_path, name.clone());

    // Then
    assert!(matches!(
        result,
        Err(BinaryRegistryError::BinaryNotFound { .. })
    ));
    assert_eq!(
        registry.resolve(&name),
        Err(BinaryRegistryError::NotFound {
            workflow_name: name,
        })
    );
}

// B-REG-30
#[test]
fn register_does_not_leave_partial_versioned_copy_when_failure_after_copy() {
    use sha2::{Digest, Sha256};

    // Given
    let (temp_dir, registry) = create_test_registry();
    let source = make_test_binary_graph_fail(temp_dir.path(), 1, "graph error");
    let source_path = bp(&source);
    let name = wn("cleanup-test");

    // Compute expected hash
    let source_content = std::fs::read(&source).expect("read source");
    let hash_hex = format!("{:x}", Sha256::digest(&source_content));

    // When
    let result = registry.register(&source_path, name);

    // Then
    assert!(matches!(
        result,
        Err(BinaryRegistryError::GraphDiscoveryFailed { .. })
    ));

    // No partial versioned copy should exist
    let versioned_path = temp_dir.path().join(&hash_hex);
    assert!(
        !versioned_path.exists(),
        "partial versioned copy should not exist at {versioned_path:?}"
    );
}

// ===========================================================================
// BinaryRegistry::register — edge cases (B-REG-56..59)
// ===========================================================================

// B-REG-56
#[test]
fn register_rejects_binary_with_missing_required_graph_fields_when_graph_output_incomplete() {
    // Given
    let (temp_dir, registry) = create_test_registry();
    let source = make_test_binary(temp_dir.path(), &graph_missing_fields());
    let source_path = bp(&source);
    let name = wn("missing-fields");

    // When
    let result = registry.register(&source_path, name.clone());

    // Then
    assert!(matches!(
        result,
        Err(BinaryRegistryError::WorkflowDefinitionInvalid {
            ref workflow_name,
            ref reason
        }) if workflow_name.as_str() == "missing-fields" && !reason.is_empty()
    ));
    assert_eq!(registry.len(), 0);
}

// B-REG-57
#[test]
fn register_rejects_binary_with_graph_cycle_when_graph_output_contains_cycle() {
    // Given
    let (temp_dir, registry) = create_test_registry();
    let source = make_test_binary(temp_dir.path(), &graph_with_cycle());
    let source_path = bp(&source);
    let name = wn("cycle-wf");

    // When
    let result = registry.register(&source_path, name.clone());

    // Then
    assert!(matches!(
        result,
        Err(BinaryRegistryError::WorkflowDefinitionInvalid {
            ref workflow_name,
            ref reason
        }) if workflow_name.as_str() == "cycle-wf" && !reason.is_empty()
    ));
    assert_eq!(registry.len(), 0);
}

// B-REG-58
#[test]
fn register_returns_binary_not_found_when_source_path_is_directory_not_file() {
    // Given
    let (temp_dir, registry) = create_test_registry();
    let dir_path = temp_dir.path().join("a-directory");
    std::fs::create_dir_all(&dir_path).expect("mkdir");
    let source_path = bp(&dir_path);
    let name = wn("dir-source");

    // When
    let result = registry.register(&source_path, name);

    // Then
    assert!(matches!(
        result,
        Err(BinaryRegistryError::BinaryNotFound { .. })
    ));
    assert_eq!(registry.len(), 0);
}

// B-REG-59
#[test]
fn register_stores_graph_definition_as_is_when_workflow_name_differs_from_graph_output() {
    // Given
    let (temp_dir, registry) = create_test_registry();
    let graph_json = valid_graph_with_name("graph-name");
    let source = make_test_binary(temp_dir.path(), &graph_json);
    let source_path = bp(&source);
    let name = wn("register-name");

    // When
    let result = registry.register(&source_path, name.clone());

    // Then
    assert_eq!(result, Ok(()));
    let (_, _, definition) = registry.resolve(&name).expect("resolved");
    assert_eq!(definition.workflow_name, wn("graph-name"));
}

// ===========================================================================
// BinaryRegistry::resolve (B-REG-31..33, B-REG-60)
// ===========================================================================

// B-REG-31
#[test]
fn resolve_returns_versioned_path_hash_and_definition_under_versions_dir_when_registration_is_active(
) {
    // Given
    let (temp_dir, registry) = create_test_registry();
    let source = make_test_binary(temp_dir.path(), &valid_single_node_graph());
    let source_path = bp(&source);
    let name = wn("deploy-prod");
    registry
        .register(&source_path, name.clone())
        .expect("register");

    // When
    let result = registry.resolve(&name);

    // Then
    let (versioned_path, binary_hash, definition) = result.expect("resolved");
    assert!(versioned_path.as_path().starts_with(temp_dir.path()));
    assert!(binary_hash.as_str().len() == 64);
    assert!(binary_hash
        .as_str()
        .chars()
        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    assert_eq!(definition.workflow_name, wn("test-wf"));
}

// B-REG-32
#[test]
fn resolve_returns_workflow_deactivated_when_registration_is_deactivated() {
    // Given
    let (temp_dir, registry) = create_test_registry();
    let source = make_test_binary(temp_dir.path(), &valid_single_node_graph());
    let source_path = bp(&source);
    let name = wn("deploy-prod");
    registry
        .register(&source_path, name.clone())
        .expect("register");
    registry.deactivate(&name).expect("deactivate");

    // When
    let result = registry.resolve(&name);

    // Then
    assert!(matches!(
        result,
        Err(BinaryRegistryError::WorkflowDeactivated {
            ref workflow_name
        }) if workflow_name.as_str() == "deploy-prod"
    ));
}

// B-REG-33
#[test]
fn resolve_returns_not_found_when_workflow_not_in_registry() {
    // Given
    let (_temp_dir, registry) = create_test_registry();
    let name = wn("nonexistent");

    // When
    let result = registry.resolve(&name);

    // Then
    assert!(matches!(
        result,
        Err(BinaryRegistryError::NotFound {
            ref workflow_name
        }) if workflow_name.as_str() == "nonexistent"
    ));
}

// B-REG-60
#[test]
fn resolve_returns_correct_definition_for_each_workflow_after_multiple_registrations() {
    // Given
    let (temp_dir, registry) = create_test_registry();
    std::fs::create_dir_all(temp_dir.path().join("a")).expect("mkdir a");
    std::fs::create_dir_all(temp_dir.path().join("b")).expect("mkdir b");
    let source_a = make_test_binary(
        &temp_dir.path().join("a"),
        &valid_graph_single_node("node-a"),
    );
    let source_b = make_test_binary(&temp_dir.path().join("b"), &valid_two_node_graph());
    let source_a_path = bp(&source_a);
    let source_b_path = bp(&source_b);
    let name_a = wn("wf-a");
    let name_b = wn("wf-b");

    registry
        .register(&source_a_path, name_a.clone())
        .expect("register a");
    registry
        .register(&source_b_path, name_b.clone())
        .expect("register b");

    // When
    let (_, _, def_a) = registry.resolve(&name_a).expect("resolve a");
    let (_, _, def_b) = registry.resolve(&name_b).expect("resolve b");

    // Then
    assert_eq!(def_a.nodes.len(), 1);
    assert_eq!(def_b.nodes.len(), 2);
    assert_ne!(def_a.workflow_name, def_b.workflow_name);
}

// ===========================================================================
// BinaryRegistry::deactivate (B-REG-34..37, B-REG-54, B-REG-61)
// ===========================================================================

// B-REG-34
#[test]
fn deactivate_transitions_active_to_deactivated_when_workflow_is_active() {
    // Given
    let (temp_dir, registry) = create_test_registry();
    let source = make_test_binary(temp_dir.path(), &valid_single_node_graph());
    let source_path = bp(&source);
    let name = wn("deploy-prod");
    registry
        .register(&source_path, name.clone())
        .expect("register");

    // When
    let result = registry.deactivate(&name);

    // Then
    assert_eq!(result, Ok(()));
    let entries = registry.list();
    assert_eq!(entries[0].1.status, RegistrationStatus::Deactivated);
    assert!(matches!(
        registry.resolve(&name),
        Err(BinaryRegistryError::WorkflowDeactivated { .. })
    ));
}

// B-REG-35
#[test]
fn deactivate_returns_ok_when_already_deactivated() {
    // Given
    let (temp_dir, registry) = create_test_registry();
    let source = make_test_binary(temp_dir.path(), &valid_single_node_graph());
    let source_path = bp(&source);
    let name = wn("deploy-prod");
    registry
        .register(&source_path, name.clone())
        .expect("register");
    registry.deactivate(&name).expect("first deactivate");

    // When
    let result = registry.deactivate(&name);

    // Then
    assert_eq!(result, Ok(()));
    let entries = registry.list();
    assert_eq!(entries[0].1.status, RegistrationStatus::Deactivated);
}

// B-REG-36
#[test]
fn deactivate_returns_not_found_when_workflow_not_in_registry() {
    // Given
    let (_temp_dir, registry) = create_test_registry();
    let name = wn("nonexistent");

    // When
    let result = registry.deactivate(&name);

    // Then
    assert!(matches!(
        result,
        Err(BinaryRegistryError::NotFound {
            ref workflow_name
        }) if workflow_name.as_str() == "nonexistent"
    ));
}

// B-REG-37
#[test]
fn deactivate_does_not_delete_versioned_binary_from_disk() {
    // Given
    let (temp_dir, registry) = create_test_registry();
    let source = make_test_binary(temp_dir.path(), &valid_single_node_graph());
    let source_path = bp(&source);
    let name = wn("deploy-prod");
    registry
        .register(&source_path, name.clone())
        .expect("register");

    let (versioned_path, _, _) = registry.resolve(&name).expect("resolve");

    // When
    let result = registry.deactivate(&name);

    // Then
    assert_eq!(result, Ok(()));
    assert!(std::fs::metadata(versioned_path.as_path()).is_ok());
}

// B-REG-54
#[test]
fn deactivate_preserves_all_registration_fields_when_workflow_is_active() {
    // Given
    let (temp_dir, registry) = create_test_registry();
    let source = make_test_binary(temp_dir.path(), &valid_three_node_graph());
    let source_path = bp(&source);
    let name = wn("field-test");
    registry
        .register(&source_path, name.clone())
        .expect("register");

    let (orig_path, orig_hash, orig_def) = registry.resolve(&name).expect("resolve before");

    // When
    let result = registry.deactivate(&name);

    // Then
    assert_eq!(result, Ok(()));
    assert!(matches!(
        registry.resolve(&name),
        Err(BinaryRegistryError::WorkflowDeactivated { .. })
    ));

    let entries = registry.list();
    let reg = &entries[0].1;
    assert_eq!(reg.binary_hash.as_str(), orig_hash.as_str());
    assert_eq!(reg.versioned_path.as_path(), orig_path.as_path());
    assert_eq!(reg.definition.nodes.len(), orig_def.nodes.len());
}

// B-REG-61
#[test]
fn deactivate_idempotent_called_twice_returns_ok_and_all_fields_unchanged() {
    // Given
    let (temp_dir, registry) = create_test_registry();
    let source = make_test_binary(temp_dir.path(), &valid_three_node_graph());
    let source_path = bp(&source);
    let name = wn("double-deact");
    registry
        .register(&source_path, name.clone())
        .expect("register");

    let (orig_path, orig_hash, orig_def) = registry.resolve(&name).expect("resolve before");

    registry.deactivate(&name).expect("first deactivate");

    // When: second deactivate
    let result = registry.deactivate(&name);

    // Then
    assert_eq!(result, Ok(()));
    let entries = registry.list();
    let reg = &entries[0].1;
    assert_eq!(reg.status, RegistrationStatus::Deactivated);
    assert_eq!(reg.binary_hash.as_str(), orig_hash.as_str());
    assert_eq!(reg.versioned_path.as_path(), orig_path.as_path());
    assert_eq!(reg.definition.nodes.len(), orig_def.nodes.len());
}

// ===========================================================================
// BinaryRegistry::reap (B-REG-38..42, B-REG-55, B-REG-63)
// ===========================================================================

// B-REG-38
#[test]
fn reap_removes_deactivated_registrations_with_zero_active_instances() {
    // Given
    let (temp_dir, registry) = create_test_registry();
    let source = make_test_binary(temp_dir.path(), &valid_single_node_graph());
    let source_path = bp(&source);
    let name = wn("old-workflow");
    registry
        .register(&source_path, name.clone())
        .expect("register");
    registry.deactivate(&name).expect("deactivate");

    // When
    let report = registry.reap(|_| false);

    // Then
    assert_eq!(report.reaped, vec![name.clone()]);
    assert!(report.skipped.is_empty());
    assert!(report.failures.is_empty());
    assert!(matches!(
        registry.resolve(&name),
        Err(BinaryRegistryError::NotFound { .. })
    ));
}

// B-REG-39
#[test]
fn reap_deletes_versioned_binaries_from_disk_for_reaped_workflows() {
    // Given
    let (temp_dir, registry) = create_test_registry();
    let source = make_test_binary(temp_dir.path(), &valid_single_node_graph());
    let source_path = bp(&source);
    let name = wn("old-workflow");
    registry
        .register(&source_path, name.clone())
        .expect("register");

    let (versioned_path, _, _) = registry.resolve(&name).expect("resolve");
    registry.deactivate(&name).expect("deactivate");

    // When
    let report = registry.reap(|_| false);

    // Then
    assert_eq!(report.reaped, vec![name]);
    assert!(std::fs::metadata(versioned_path.as_path()).is_err());
}

// B-REG-40
#[test]
fn reap_returns_not_found_on_resolve_for_reaped_workflows() {
    // Given
    let (temp_dir, registry) = create_test_registry();
    let source = make_test_binary(temp_dir.path(), &valid_single_node_graph());
    let source_path = bp(&source);
    let name = wn("old-workflow");
    registry
        .register(&source_path, name.clone())
        .expect("register");
    registry.deactivate(&name).expect("deactivate");

    // When
    registry.reap(|_| false);

    // Then
    assert!(matches!(
        registry.resolve(&name),
        Err(BinaryRegistryError::NotFound {
            ref workflow_name
        }) if workflow_name.as_str() == "old-workflow"
    ));
}

// B-REG-41
#[test]
fn reap_skips_deactivated_registrations_with_active_instances() {
    // Given
    let (temp_dir, registry) = create_test_registry();
    let source = make_test_binary(temp_dir.path(), &valid_single_node_graph());
    let source_path = bp(&source);
    let name = wn("active-workflow");
    registry
        .register(&source_path, name.clone())
        .expect("register");

    let (versioned_path, _, _) = registry.resolve(&name).expect("resolve");
    registry.deactivate(&name).expect("deactivate");

    // When
    let report = registry.reap(|_| true);

    // Then
    assert_eq!(report.skipped, vec![name.clone()]);
    assert!(!report
        .reaped
        .iter()
        .any(|n| n.as_str() == "active-workflow"));
    assert!(matches!(
        registry.resolve(&name),
        Err(BinaryRegistryError::WorkflowDeactivated { .. })
    ));
    assert!(std::fs::metadata(versioned_path.as_path()).is_ok());
}

// B-REG-42
#[test]
fn reap_continues_sweep_and_preserves_registration_when_individual_binary_deletion_fails() {
    // Given
    let outer = tempfile::TempDir::new().expect("temp dir");
    let versions_subdir = outer.path().join("versions");
    std::fs::create_dir_all(&versions_subdir).expect("mkdir versions");
    let versions_dir = BinaryPath::new(versions_subdir.clone()).expect("absolute");
    let registry = BinaryRegistry::new(versions_dir).expect("registry");

    let source = make_test_binary(outer.path(), &valid_single_node_graph());
    let source_path = bp(&source);
    let name = wn("fail-workflow");
    registry
        .register(&source_path, name.clone())
        .expect("register");

    registry.deactivate(&name).expect("deactivate");

    // Make the versions directory read-only so deletion of the binary inside fails
    std::fs::set_permissions(&versions_subdir, std::fs::Permissions::from_mode(0o555))
        .expect("chmod parent readonly");

    // When
    let report = registry.reap(|_| false);

    // Then
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].0.as_str(), "fail-workflow");
    assert!(matches!(
        &report.failures[0].1,
        BinaryRegistryError::ReaperDeleteFailed { .. }
    ));
    assert!(!report.reaped.iter().any(|n| n.as_str() == "fail-workflow"));
    assert!(matches!(
        registry.resolve(&name),
        Err(BinaryRegistryError::WorkflowDeactivated { .. })
    ));

    // Restore permissions for cleanup
    let _ = std::fs::set_permissions(&versions_subdir, std::fs::Permissions::from_mode(0o755));
}

// B-REG-55
#[test]
fn reap_does_not_reap_active_registrations_when_called_with_mixed_entries() {
    // Given
    let (temp_dir, registry) = create_test_registry();

    // Register and deactivate "deactivated-wf"
    let source_deact = make_test_binary(temp_dir.path(), &valid_single_node_graph());
    registry
        .register(&bp(&source_deact), wn("deactivated-wf"))
        .expect("register deact");
    registry
        .deactivate(&wn("deactivated-wf"))
        .expect("deactivate");

    // Register "active-wf" (stays Active)
    let source_active = make_test_binary(temp_dir.path(), &valid_two_node_graph());
    registry
        .register(&bp(&source_active), wn("active-wf"))
        .expect("register active");

    // When
    let report = registry.reap(|_| false);

    // Then
    assert_eq!(report.reaped, vec![wn("deactivated-wf")]);
    assert!(!report.reaped.iter().any(|n| n.as_str() == "active-wf"));
    assert!(matches!(
        registry.resolve(&wn("active-wf")),
        Ok((_, _, def)) if !def.nodes.is_empty()
    ));
    assert!(matches!(
        registry.resolve(&wn("deactivated-wf")),
        Err(BinaryRegistryError::NotFound { .. })
    ));
}

// B-REG-63
#[test]
fn reap_mixed_report_contains_correct_reaped_skipped_and_failures_for_multiple_deactivated_entries()
{
    // Given
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let versions_path = temp_dir.path().to_path_buf();
    let versions_dir = BinaryPath::new(versions_path).expect("absolute");
    let registry = BinaryRegistry::new(versions_dir).expect("registry");

    // "fail-deletion" uses a separate registry with read-only parent dir
    let fail_outer = tempfile::TempDir::new().expect("temp dir");
    let fail_versions = fail_outer.path().join("versions");
    std::fs::create_dir_all(&fail_versions).expect("mkdir fail versions");
    let fail_registry =
        BinaryRegistry::new(BinaryPath::new(fail_versions.clone()).expect("absolute"))
            .expect("registry");

    std::fs::create_dir_all(temp_dir.path().join("reapable")).expect("mkdir reapable");
    let src_reapable = make_test_binary(
        &temp_dir.path().join("reapable"),
        &valid_single_node_graph(),
    );
    registry
        .register(&bp(&src_reapable), wn("reapable"))
        .expect("register reapable");
    registry
        .deactivate(&wn("reapable"))
        .expect("deactivate reapable");

    std::fs::create_dir_all(temp_dir.path().join("skipped")).expect("mkdir skipped");
    let src_skipped = make_test_binary(
        &temp_dir.path().join("skipped"),
        &valid_graph_with_name("skipped-wf"),
    );
    registry
        .register(&bp(&src_skipped), wn("skipped"))
        .expect("register skipped");
    registry
        .deactivate(&wn("skipped"))
        .expect("deactivate skipped");

    let src_fail = make_test_binary(fail_outer.path(), &valid_graph_with_name("fail-wf"));
    fail_registry
        .register(&bp(&src_fail), wn("fail-deletion"))
        .expect("register fail");
    let (_fail_versioned, _, _) = fail_registry
        .resolve(&wn("fail-deletion"))
        .expect("resolve fail");
    fail_registry
        .deactivate(&wn("fail-deletion"))
        .expect("deactivate fail");

    // Make the versions directory read-only so deletion fails
    std::fs::set_permissions(&fail_versions, std::fs::Permissions::from_mode(0o555))
        .expect("chmod readonly");

    std::fs::create_dir_all(temp_dir.path().join("active")).expect("mkdir active");
    let src_active = make_test_binary(&temp_dir.path().join("active"), &valid_two_node_graph());
    registry
        .register(&bp(&src_active), wn("still-active"))
        .expect("register active");

    // No active instances — all deactivated entries should be reaped or failed
    // When
    let report = registry.reap(|_| false);
    let fail_report = fail_registry.reap(|_| false);

    // Then
    assert!(report.reaped.iter().any(|n| n.as_str() == "reapable"));
    assert!(report.reaped.iter().any(|n| n.as_str() == "skipped"));
    assert!(fail_report
        .failures
        .iter()
        .any(|(n, _)| n.as_str() == "fail-deletion"));

    assert!(matches!(
        registry.resolve(&wn("reapable")),
        Err(BinaryRegistryError::NotFound { .. })
    ));
    assert!(matches!(
        registry.resolve(&wn("skipped")),
        Err(BinaryRegistryError::NotFound { .. })
    ));
    assert!(matches!(
        fail_registry.resolve(&wn("fail-deletion")),
        Err(BinaryRegistryError::WorkflowDeactivated { .. })
    ));
    assert!(matches!(
        registry.resolve(&wn("still-active")),
        Ok((_, _, def)) if !def.nodes.is_empty()
    ));

    // Cleanup permissions
    let _ = std::fs::set_permissions(&fail_versions, std::fs::Permissions::from_mode(0o755));
}
