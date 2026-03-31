//! Common test helpers for wtf-engine integration and E2E tests.

#![allow(dead_code)]

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use wtf_engine::BinaryPath;

/// Create a real executable shell script that supports `--graph`.
///
/// The script prints `graph_json` to stdout when called with `--graph` and exits 0.
/// For any other argument, it exits 1.
pub fn make_test_binary(dir: &Path, graph_json: &str) -> PathBuf {
    // Use a unique filename to avoid collisions when multiple binaries are created
    // in the same directory (e.g., concurrent e2e tests).
    let unique_name = format!("test-binary-{}", ulid::Ulid::new());
    let script_path = dir.join(unique_name);
    let mut file = std::fs::File::create(&script_path).expect("create script");
    write!(
        file,
        r#"#!/bin/sh
if [ "$1" = "--graph" ]; then
cat <<'GRAPH_EOF'
{graph_json}
GRAPH_EOF
    exit 0
fi
exit 1
"#
    )
    .expect("write script");
    drop(file);
    std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755))
        .expect("chmod +x");
    script_path
}

/// Create a test binary whose `--graph` exits non-zero with stderr output.
pub fn make_test_binary_graph_fail(dir: &Path, exit_code: i32, stderr_msg: &str) -> PathBuf {
    let script_path = dir.join("test-binary-fail");
    let mut file = std::fs::File::create(&script_path).expect("create script");
    write!(
        file,
        r#"#!/bin/sh
if [ "$1" = "--graph" ]; then
    printf '%s' '{stderr_msg}' >&2
    exit {exit_code}
fi
exit 1
"#
    )
    .expect("write script");
    drop(file);
    std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755))
        .expect("chmod +x");
    script_path
}

/// Create a test binary whose `--graph` prints raw text (not JSON) to stdout.
pub fn make_test_binary_invalid_json(dir: &Path, output: &str) -> PathBuf {
    let script_path = dir.join("test-binary-badjson");
    let mut file = std::fs::File::create(&script_path).expect("create script");
    write!(
        file,
        r#"#!/bin/sh
if [ "$1" = "--graph" ]; then
    printf '%s' '{output}'
    exit 0
fi
exit 1
"#
    )
    .expect("write script");
    drop(file);
    std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755))
        .expect("chmod +x");
    script_path
}

/// Create a test binary whose `--graph` prints nothing (empty stdout) and exits 0.
pub fn make_test_binary_empty_stdout(dir: &Path) -> PathBuf {
    let script_path = dir.join("test-binary-empty");
    let mut file = std::fs::File::create(&script_path).expect("create script");
    write!(
        file,
        r#"#!/bin/sh
if [ "$1" = "--graph" ]; then
    exit 0
fi
exit 1
"#
    )
    .expect("write script");
    drop(file);
    std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755))
        .expect("chmod +x");
    script_path
}

// ---------------------------------------------------------------------------
// Valid graph JSON helpers
// ---------------------------------------------------------------------------

/// Valid single-node workflow JSON.
pub fn valid_single_node_graph() -> String {
    r#"{"workflow_name":"test-wf","nodes":[{"node_name":"node-a","retry_policy":{"max_attempts":3,"backoff_ms":1000,"backoff_multiplier":2.0}}],"edges":[]}"#.to_string()
}

/// Valid 3-node linear workflow JSON.
pub fn valid_three_node_graph() -> String {
    r#"{"workflow_name":"my-workflow","nodes":[{"node_name":"node-a","retry_policy":{"max_attempts":3,"backoff_ms":1000,"backoff_multiplier":2.0}},{"node_name":"node-b","retry_policy":{"max_attempts":3,"backoff_ms":1000,"backoff_multiplier":2.0}},{"node_name":"node-c","retry_policy":{"max_attempts":3,"backoff_ms":1000,"backoff_multiplier":2.0}}],"edges":[{"source_node":"node-a","target_node":"node-b","condition":"Always"},{"source_node":"node-b","target_node":"node-c","condition":"Always"}]}"#.to_string()
}

/// Valid 3-node graph with custom workflow_name.
pub fn valid_graph_with_name(name: &str) -> String {
    format!(
        r#"{{"workflow_name":"{name}","nodes":[{{"node_name":"node-a","retry_policy":{{"max_attempts":3,"backoff_ms":1000,"backoff_multiplier":2.0}}}}],"edges":[]}}"#
    )
}

/// Valid graph JSON with empty nodes list (will fail structural validation).
pub fn graph_with_empty_nodes() -> String {
    r#"{"workflow_name":"empty-wf","nodes":[],"edges":[]}"#.to_string()
}

/// Valid JSON but missing required fields (only workflow_name).
pub fn graph_missing_fields() -> String {
    r#"{"workflow_name":"x"}"#.to_string()
}

/// Valid JSON with a cycle: a -> b -> a.
pub fn graph_with_cycle() -> String {
    r#"{"workflow_name":"cycle-wf","nodes":[{"node_name":"a","retry_policy":{"max_attempts":3,"backoff_ms":1000,"backoff_multiplier":2.0}},{"node_name":"b","retry_policy":{"max_attempts":3,"backoff_ms":1000,"backoff_multiplier":2.0}}],"edges":[{"source_node":"a","target_node":"b","condition":"Always"},{"source_node":"b","target_node":"a","condition":"Always"}]}"#.to_string()
}

/// Valid single-node graph with a specific node name.
pub fn valid_graph_single_node(node_name: &str) -> String {
    format!(
        r#"{{"workflow_name":"test","nodes":[{{"node_name":"{node_name}","retry_policy":{{"max_attempts":3,"backoff_ms":1000,"backoff_multiplier":2.0}}}}],"edges":[]}}"#
    )
}

/// Valid 2-node graph.
pub fn valid_two_node_graph() -> String {
    r#"{"workflow_name":"two-node-wf","nodes":[{"node_name":"node-b1","retry_policy":{"max_attempts":3,"backoff_ms":1000,"backoff_multiplier":2.0}},{"node_name":"node-b2","retry_policy":{"max_attempts":3,"backoff_ms":1000,"backoff_multiplier":2.0}}],"edges":[]}"#.to_string()
}

// ---------------------------------------------------------------------------
// Registry construction helpers
// ---------------------------------------------------------------------------

/// Create a BinaryRegistry with a real temp directory as versions_dir.
pub fn create_test_registry() -> (tempfile::TempDir, wtf_engine::BinaryRegistry) {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let versions_dir =
        BinaryPath::new(temp_dir.path().to_path_buf()).expect("versions_dir is absolute");
    let registry = wtf_engine::BinaryRegistry::new(versions_dir).expect("registry");
    (temp_dir, registry)
}

/// Create a BinaryPath from a PathBuf (panics if not absolute).
pub fn bp(path: &Path) -> BinaryPath {
    BinaryPath::new(path.to_path_buf()).expect("absolute path")
}

/// Create a WorkflowName (panics if invalid).
pub fn wn(name: &str) -> wtf_types::WorkflowName {
    wtf_types::WorkflowName::parse(name).expect("valid workflow name")
}

/// Create a BinaryHash (panics if invalid).
pub fn bh(hex: &str) -> wtf_types::BinaryHash {
    wtf_types::BinaryHash::parse(hex).expect("valid binary hash")
}

/// Create an InstanceId for testing.
pub fn test_instance_id() -> wtf_types::InstanceId {
    let ulid = ulid::Ulid::new();
    wtf_types::InstanceId::parse(&ulid.to_string()).expect("valid instance id")
}
