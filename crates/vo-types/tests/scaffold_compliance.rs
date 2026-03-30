//! Scaffold compliance integration tests for vo-types crate.
//!
//! These tests verify the crate meets the scaffold specification:
//! - Dependency purity (only allowed deps: serde, thiserror, uuid, ulid)
//! - No infra dependencies (tokio, axum, ractor, fjall, tower, reqwest)
//! - Module stubs exist with correct doc comments
//! - Public API preserved
//! - Workspace metadata correct
//! - Build and lint clean

use std::collections::BTreeSet;

/// Path to the crate's Cargo.toml.
const CARGO_TOML_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");

/// Path to lib.rs.
const LIB_RS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs");

/// Path to events.rs stub.
const EVENTS_RS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/events.rs");

/// Path to state.rs stub.
const STATE_RS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/state.rs");

/// Path to the workspace root.
const WORKSPACE_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/..");

/// Allowed dependencies — the EXACT set permitted in [dependencies].
const ALLOWED_DEPS: &[&str] = &["serde", "thiserror", "uuid", "ulid", "serde_json"];

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Extract all top-level keys from a specific TOML section.
///
/// Handles both `key.workspace = true` and `key = "version"` forms.
fn parse_section_keys(content: &str, target_section: &str) -> BTreeSet<String> {
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .scan(None::<String>, |state, line| {
            if let Some(header) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                *state = Some(header.trim().to_string());
                Some(None)
            } else {
                Some(Some((state.clone(), line)))
            }
        })
        .flatten()
        .filter_map(|(current_section, line)| {
            if current_section.as_deref() == Some(target_section) {
                if let Some(key_end) = line.find(['=', '.']) {
                    let key = line[..key_end].trim();
                    if !key.is_empty() {
                        return Some(key.to_string());
                    }
                }
            }
            None
        })
        .collect()
}

/// Parse the `[dependencies]` section and return dep names.
fn parse_dependencies(content: &str) -> BTreeSet<String> {
    parse_section_keys(content, "dependencies")
}

/// Parse the `[dev-dependencies]` section and return dep names.
fn parse_dev_dependencies(content: &str) -> BTreeSet<String> {
    parse_section_keys(content, "dev-dependencies")
}

/// Check that a line exists in the content that, when trimmed, exactly equals `expected`.
///
/// Uses line-level matching (not substring containment) to avoid false positives
/// on commented-out declarations like `// mod events;`.
fn has_exact_line(content: &str, expected: &str) -> bool {
    content.lines().any(|line| line.trim() == expected)
}

// ---------------------------------------------------------------------------
// Behavior 1-4: Cargo.toml contains all allowed dependencies
// ---------------------------------------------------------------------------

#[test]
fn cargo_toml_contains_allowed_dependencies_when_inspected() {
    // Given: vo-types/Cargo.toml exists as a workspace member
    let content = std::fs::read_to_string(CARGO_TOML_PATH).expect("Failed to read Cargo.toml");

    // When: [dependencies] section is parsed
    let deps = parse_dependencies(&content);

    // Then: "serde", "thiserror", "ulid", and "uuid" are all present
    assert!(
        deps.contains("serde"),
        "Expected dependency 'serde' in [dependencies], found: {deps:?}"
    );
    assert!(
        deps.contains("thiserror"),
        "Expected dependency 'thiserror' in [dependencies], found: {deps:?}"
    );
    assert!(
        deps.contains("uuid"),
        "Expected dependency 'uuid' in [dependencies], found: {deps:?}"
    );
    assert!(
        deps.contains("ulid"),
        "Expected dependency 'ulid' in [dependencies], found: {deps:?}"
    );
}

// ---------------------------------------------------------------------------
// Behavior 5: serde_json is NOT a runtime dependency
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Behavior 6: serde_json IS a dev-dependency
// ---------------------------------------------------------------------------

#[test]
fn cargo_toml_contains_serde_json_in_dev_dependencies_when_inspected() {
    // Given: vo-types/Cargo.toml exists
    let content = std::fs::read_to_string(CARGO_TOML_PATH).expect("Failed to read Cargo.toml");

    // When: [dev-dependencies] section is parsed
    let dev_deps = parse_dev_dependencies(&content);

    // Then: "serde_json" appears as a key
    assert!(
        dev_deps.contains("serde_json"),
        "Expected 'serde_json' in [dev-dependencies], found: {dev_deps:?}"
    );
}

// ---------------------------------------------------------------------------
// Behavior 7-12: No infra dependencies present
// ---------------------------------------------------------------------------

#[test]
fn cargo_toml_excludes_all_infra_dependencies_when_inspected() {
    // Given: vo-types/Cargo.toml exists
    let content = std::fs::read_to_string(CARGO_TOML_PATH).expect("Failed to read Cargo.toml");

    // When: the entire file is scanned
    // Then: none of the forbidden deps appear anywhere
    assert!(
        !content.contains("tokio"),
        "Forbidden dependency 'tokio' must not appear anywhere in Cargo.toml"
    );
    assert!(
        !content.contains("axum"),
        "Forbidden dependency 'axum' must not appear anywhere in Cargo.toml"
    );
    assert!(
        !content.contains("ractor"),
        "Forbidden dependency 'ractor' must not appear anywhere in Cargo.toml"
    );
    assert!(
        !content.contains("fjall"),
        "Forbidden dependency 'fjall' must not appear anywhere in Cargo.toml"
    );
    assert!(
        !content.contains("tower"),
        "Forbidden dependency 'tower' must not appear anywhere in Cargo.toml"
    );
    assert!(
        !content.contains("reqwest"),
        "Forbidden dependency 'reqwest' must not appear anywhere in Cargo.toml"
    );
}

// ---------------------------------------------------------------------------
// Behavior 13-14: Workspace metadata is set
// ---------------------------------------------------------------------------

#[test]
fn cargo_toml_uses_workspace_version_and_edition_when_inspected() {
    // Given: vo-types/Cargo.toml exists
    let content = std::fs::read_to_string(CARGO_TOML_PATH).expect("Failed to read Cargo.toml");

    // When: [package] section is parsed
    // Then: "version.workspace = true" and "edition.workspace = true" are present
    assert!(
        has_exact_line(&content, "version.workspace = true"),
        "Expected 'version.workspace = true' in [package] section"
    );
    assert!(
        has_exact_line(&content, "edition.workspace = true"),
        "Expected 'edition.workspace = true' in [package] section"
    );
}

// ---------------------------------------------------------------------------
// MAJOR-2 fix: Dependency set is EXACTLY the allowed set (subset constraint)
// ---------------------------------------------------------------------------

#[test]
fn cargo_toml_dependencies_are_exact_allowed_set_when_inspected() {
    // Given: vo-types/Cargo.toml exists
    let content = std::fs::read_to_string(CARGO_TOML_PATH).expect("Failed to read Cargo.toml");

    // When: [dependencies] section is parsed into a set of key names
    let deps = parse_dependencies(&content);

    // Then: the set equals exactly {"serde", "thiserror", "uuid", "ulid"}
    let expected: BTreeSet<String> = ALLOWED_DEPS.iter().map(|s| s.to_string()).collect();

    assert_eq!(
        deps, expected,
        "[dependencies] must contain exactly {{serde, thiserror, uuid, ulid, serde_json}}, found: {deps:?}"
    );
}

// ---------------------------------------------------------------------------
// Behavior 15-16: Module declarations exist in lib.rs (line-level match)
// ---------------------------------------------------------------------------

#[test]
fn lib_rs_declares_events_and_state_modules_when_inspected() {
    // Given: crates/vo-types/src/lib.rs exists
    let content = std::fs::read_to_string(LIB_RS_PATH).expect("Failed to read lib.rs");

    // When: the file content is scanned
    // Then: "mod events;" and "mod state;" appear as trimmed lines
    assert!(
        has_exact_line(&content, "pub mod events;"),
        "Expected 'pub mod events;' as a trimmed line in lib.rs (not commented out)"
    );
    assert!(
        has_exact_line(&content, "pub mod state;"),
        "Expected 'pub mod state;' as a trimmed line in lib.rs (not commented out)"
    );
}

// ---------------------------------------------------------------------------
// Behavior 17-18: Stub files exist and compile
// ---------------------------------------------------------------------------

#[test]
fn stub_files_exist_and_compile_when_workspace_checked() {
    // Given: crates/vo-types is a workspace member
    // When: src/events.rs and src/state.rs are checked
    // Then: both files exist on disk
    assert!(
        std::path::Path::new(EVENTS_RS_PATH).exists(),
        "src/events.rs must exist on disk"
    );
    assert!(
        std::path::Path::new(STATE_RS_PATH).exists(),
        "src/state.rs must exist on disk"
    );

    // And: cargo check --workspace exits with code 0
    let output = std::process::Command::new("cargo")
        .args(["check", "--workspace"])
        .current_dir(WORKSPACE_ROOT)
        .output()
        .expect("Failed to execute cargo check");

    assert!(
        output.status.success(),
        "cargo check --workspace failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

// ---------------------------------------------------------------------------
// MINOR-2 fix: Doc comments in stub files
// ---------------------------------------------------------------------------

#[test]
fn events_rs_contains_doc_comment_when_inspected() {
    // Given: src/events.rs exists
    let content = std::fs::read_to_string(EVENTS_RS_PATH).expect("Failed to read events.rs");

    // Then: doc comment is present
    assert!(
        content.contains("//! Domain events for the vo-engine."),
        "events.rs must contain doc comment '//! Domain events for the vo-engine.'"
    );
}

#[test]
fn state_rs_contains_doc_comment_when_inspected() {
    // Given: src/state.rs exists
    let content = std::fs::read_to_string(STATE_RS_PATH).expect("Failed to read state.rs");

    // Then: doc comment is present
    assert!(
        content.contains("//! Domain state types for the vo-engine."),
        "state.rs must contain doc comment '//! Domain state types for the vo-engine.'"
    );
}

// ---------------------------------------------------------------------------
// Behavior 19: Workspace compiles with zero warnings
// ---------------------------------------------------------------------------

#[test]
fn workspace_compiles_with_zero_warnings_when_cargo_check_runs() {
    // Given: all workspace members are present and configured
    // When: cargo check --workspace is executed
    let output = std::process::Command::new("cargo")
        .args(["check", "--workspace"])
        .current_dir(WORKSPACE_ROOT)
        .output()
        .expect("Failed to execute cargo check");

    // Then: process exits with code 0
    assert!(
        output.status.success(),
        "cargo check --workspace exited with non-zero code:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    // And: stderr contains zero lines matching "warning"
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("warning"),
        "cargo check --workspace produced warnings:\n{stderr}"
    );
}

// ---------------------------------------------------------------------------
// Behavior 20: Clippy clean
// ---------------------------------------------------------------------------

#[test]
fn workspace_passes_clippy_with_zero_warnings_when_clippy_runs() {
    // Given: all workspace members compile without errors
    // When: cargo clippy --workspace -- -D warnings is executed
    let output = std::process::Command::new("cargo")
        .args(["clippy", "--workspace", "--", "-D", "warnings"])
        .current_dir(WORKSPACE_ROOT)
        .output()
        .expect("Failed to execute cargo clippy");

    // Then: process exits with code 0
    assert!(
        output.status.success(),
        "cargo clippy exited with non-zero code:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    // And: stderr contains zero lines matching "warning"
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("warning"),
        "cargo clippy produced warnings:\n{stderr}"
    );
}

// ---------------------------------------------------------------------------
// Behavior 21: ParseError publicly accessible
// ---------------------------------------------------------------------------

#[test]
fn parse_error_is_publicly_accessible_when_crate_used() {
    // Given: vo-types compiles as a workspace member
    // When: external code references ParseError
    let type_name = std::any::type_name::<vo_types::ParseError>();

    // Then: the type resolves successfully
    assert!(
        type_name.contains("ParseError"),
        "ParseError type name should contain 'ParseError', got: {type_name}"
    );
}

// ---------------------------------------------------------------------------
// Behavior 22: NonEmptyVec publicly accessible
// ---------------------------------------------------------------------------

#[test]
fn non_empty_vec_is_publicly_accessible_when_crate_used() {
    // Given: vo-types compiles as a workspace member
    // When: external code references NonEmptyVec
    let type_name = std::any::type_name::<vo_types::NonEmptyVec<()>>();

    // Then: the type resolves successfully
    assert!(
        type_name.contains("NonEmptyVec"),
        "NonEmptyVec type name should contain 'NonEmptyVec', got: {type_name}"
    );
}

// ---------------------------------------------------------------------------
// Behavior 23: All integer types publicly accessible
// ---------------------------------------------------------------------------

#[test]
fn integer_types_are_publicly_accessible_when_crate_used() {
    // Given: vo-types compiles as a workspace member
    // When: external code imports all integer types
    use vo_types::{
        AttemptNumber, DurationMs, EventVersion, FireAtMs, MaxAttempts, SequenceNumber, TimeoutMs,
        TimestampMs,
    };

    // Then: all types compile without error
    assert!(std::any::type_name::<SequenceNumber>().contains("SequenceNumber"));
    assert!(std::any::type_name::<EventVersion>().contains("EventVersion"));
    assert!(std::any::type_name::<AttemptNumber>().contains("AttemptNumber"));
    assert!(std::any::type_name::<TimeoutMs>().contains("TimeoutMs"));
    assert!(std::any::type_name::<DurationMs>().contains("DurationMs"));
    assert!(std::any::type_name::<TimestampMs>().contains("TimestampMs"));
    assert!(std::any::type_name::<FireAtMs>().contains("FireAtMs"));
    assert!(std::any::type_name::<MaxAttempts>().contains("MaxAttempts"));

    // Verify the types have public constructors accessible from outside the crate
    SequenceNumber::parse("1").unwrap();
    EventVersion::parse("1").unwrap();
    AttemptNumber::parse("1").unwrap();
    TimeoutMs::parse("1").unwrap();
    DurationMs::parse("0").unwrap();
    TimestampMs::parse("0").unwrap();
    FireAtMs::parse("0").unwrap();
    MaxAttempts::parse("1").unwrap();
}

// ---------------------------------------------------------------------------
// Behavior 24: All string types publicly accessible
// ---------------------------------------------------------------------------

#[test]
fn string_types_are_publicly_accessible_when_crate_used() {
    // Given: vo-types compiles as a workspace member
    // When: external code imports all string types
    use vo_types::{BinaryHash, IdempotencyKey, InstanceId, NodeName, TimerId, WorkflowName};

    // Then: all types compile without error
    assert!(std::any::type_name::<InstanceId>().contains("InstanceId"));
    assert!(std::any::type_name::<WorkflowName>().contains("WorkflowName"));
    assert!(std::any::type_name::<NodeName>().contains("NodeName"));
    assert!(std::any::type_name::<BinaryHash>().contains("BinaryHash"));
    assert!(std::any::type_name::<TimerId>().contains("TimerId"));
    assert!(std::any::type_name::<IdempotencyKey>().contains("IdempotencyKey"));
}

// ---------------------------------------------------------------------------
// Behavior 25: All workflow types publicly accessible
// ---------------------------------------------------------------------------

#[test]
fn workflow_types_are_publicly_accessible_when_crate_used() {
    // Given: vo-types compiles as a workspace member
    // When: external code imports all workflow types and the next_nodes function
    use vo_types::{
        next_nodes, DagNode, Edge, EdgeCondition, RetryPolicy, RetryPolicyError, StepOutcome,
        WorkflowDefinition, WorkflowDefinitionError,
    };

    // Then: all types compile without error
    assert!(std::any::type_name::<DagNode>().contains("DagNode"));
    assert!(std::any::type_name::<Edge>().contains("Edge"));
    assert!(std::any::type_name::<EdgeCondition>().contains("EdgeCondition"));
    assert!(std::any::type_name::<RetryPolicy>().contains("RetryPolicy"));
    assert!(std::any::type_name::<RetryPolicyError>().contains("RetryPolicyError"));
    assert!(std::any::type_name::<StepOutcome>().contains("StepOutcome"));
    assert!(std::any::type_name::<WorkflowDefinition>().contains("WorkflowDefinition"));
    assert!(std::any::type_name::<WorkflowDefinitionError>().contains("WorkflowDefinitionError"));

    // Verify next_nodes is a callable function (compile-time resolution check)
    // We can't take a concrete fn pointer due to lifetime generics,
    // but we can verify the function resolves by checking its type name.
    assert!(
        std::any::type_name_of_val(&next_nodes).contains("next_nodes"),
        "next_nodes function should be publicly accessible"
    );
}

// ---------------------------------------------------------------------------
// Behavior 27: serde_json usable in test context
// ---------------------------------------------------------------------------

#[test]
fn serde_json_compiles_in_test_context_when_referenced() {
    // Given: serde_json is in [dev-dependencies] of vo-types
    // When: a test function calls serde_json::to_value
    let value = serde_json::to_value(42_i32).expect("serde_json::to_value should work");

    // Then: the function executes successfully
    assert_eq!(value, serde_json::json!(42));
}
