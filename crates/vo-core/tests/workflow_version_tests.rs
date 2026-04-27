//! WorkflowVersion integration tests.

use std::collections::HashSet;

use vo_core::workflow_version::{WorkflowVersion, WorkflowVersionError};
use vo_types::{BinaryHash, WorkflowName, TimestampMs};

fn make_version(wf: &str, hash: &str) -> WorkflowVersion {
    let name = WorkflowName::parse(wf).expect("workflow name should be valid");
    let hash = BinaryHash::parse(hash).expect("hash should be valid");
    let ts = TimestampMs::try_from(1712200000000u64).unwrap();
    WorkflowVersion::new(name, hash, ts).expect("version should be created")
}

#[test]
fn workflow_version_creation_with_valid_hash() {
    let hash = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let version = make_version("test-workflow", hash);

    assert_eq!(version.name().as_str(), "test-workflow");
    assert_eq!(version.hash().as_str(), hash);
    assert_eq!(version.schema_version(), 1);
    assert!(version.binary_path().contains(hash));
}

#[test]
fn workflow_version_rejects_short_hash() {
    let name = WorkflowName::parse("test").unwrap();
    let short_hash = BinaryHash::parse("aabbccdd").unwrap();
    let ts = TimestampMs::try_from(1712200000000u64).unwrap();

    let result = WorkflowVersion::new(name, short_hash, ts);
    assert_eq!(result, Err(WorkflowVersionError::HashTooShort));
}

#[test]
fn workflow_version_binary_path_format() {
    let hash = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let version = make_version("my-workflow", hash);

    let expected_prefix = format!("/var/wtf/versions/{}/my-workflow", hash);
    assert_eq!(
        version.binary_path(),
        expected_prefix,
        "binary_path should follow /var/wtf/versions/<hash>/<name> format"
    );
}

#[test]
fn workflow_version_json_roundtrip() {
    let version = make_version(
        "serialization-test",
        "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
    );

    let json = serde_json::to_string(&version).expect("serialization should succeed");
    assert!(
        json.contains("\"workflow_name\""),
        "JSON should use workflow_name field"
    );
    assert!(
        json.contains("\"version_hash\""),
        "JSON should use version_hash field"
    );

    let parsed: WorkflowVersion =
        serde_json::from_str(&json).expect("deserialization should succeed");
    assert_eq!(parsed, version, "version should round-trip through JSON");
}

#[test]
fn workflow_version_is_hashable() {
    let v1 = make_version(
        "workflow-a",
        "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
    );
    let v2 = make_version(
        "workflow-b",
        "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
    );

    let mut set = HashSet::new();
    set.insert(v1.clone());
    set.insert(v2.clone());

    assert_eq!(
        set.len(),
        2,
        "different versions should be distinct in HashSet"
    );
    assert!(set.contains(&v1), "set should contain v1");
    assert!(set.contains(&v2), "set should contain v2");

    let v1_copy = make_version(
        "workflow-a",
        "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
    );
    assert!(
        set.contains(&v1_copy),
        "set should find equivalent version by hash"
    );
}