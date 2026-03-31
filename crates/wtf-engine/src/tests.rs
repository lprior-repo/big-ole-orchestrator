use super::*;
use std::path::{Path, PathBuf};
use wtf_types::{BinaryHash, WorkflowDefinition, WorkflowName};

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn wn(name: &str) -> WorkflowName {
    WorkflowName::parse(name).expect("valid workflow name")
}

fn bh(hex: &str) -> BinaryHash {
    BinaryHash::parse(hex).expect("valid binary hash")
}

fn valid_single_node_json() -> String {
    r#"{"workflow_name":"test-wf","nodes":[{"node_name":"node-a","retry_policy":{"max_attempts":3,"backoff_ms":1000,"backoff_multiplier":2.0}}],"edges":[]}"#.to_string()
}

// =========================================================================
// BinaryPath (B-REG-01..07, B-REG-64..68)
// =========================================================================

#[test]
fn binary_path_accepts_absolute_path_when_path_starts_with_slash() {
    let path = PathBuf::from("/usr/local/bin/my-binary");
    let result = BinaryPath::new(path.clone());
    assert_eq!(result, Ok(BinaryPath(path)));
    assert_eq!(
        result.unwrap().as_path(),
        Path::new("/usr/local/bin/my-binary")
    );
}

#[test]
fn binary_path_rejects_relative_with_non_absolute_error_when_path_not_starting_with_slash() {
    let path = PathBuf::from("relative/path/binary");
    let result = BinaryPath::new(path.clone());
    assert!(matches!(
        result,
        Err(BinaryRegistryError::NonAbsolutePath { path: ref p }) if p == "relative/path/binary"
    ));
}

#[test]
fn binary_path_rejects_empty_with_non_absolute_error_when_path_is_empty() {
    let path = PathBuf::from("");
    let result = BinaryPath::new(path.clone());
    assert!(matches!(
        result,
        Err(BinaryRegistryError::NonAbsolutePath { path: ref p }) if p.is_empty()
    ));
}

#[test]
fn binary_path_returns_inner_path_when_as_path_called() {
    let bp = BinaryPath::new(PathBuf::from("/opt/wtf/binary")).expect("absolute");
    assert_eq!(bp.as_path(), Path::new("/opt/wtf/binary"));
}

#[test]
fn binary_path_returns_parent_directory_when_parent_called() {
    let bp = BinaryPath::new(PathBuf::from("/opt/wtf/binary")).expect("absolute");
    assert_eq!(bp.parent(), Path::new("/opt/wtf"));
}

#[test]
fn binary_path_displays_path_string_exactly_when_formatted() {
    let bp = BinaryPath::new(PathBuf::from("/opt/wtf/binary")).expect("absolute");
    assert_eq!(format!("{bp}"), "/opt/wtf/binary");
}

#[test]
fn binary_path_converts_to_pathbuf_when_into_trait_used() {
    let bp = BinaryPath::new(PathBuf::from("/opt/wtf/binary")).expect("absolute");
    let pb: PathBuf = bp.into();
    assert_eq!(pb, PathBuf::from("/opt/wtf/binary"));
}

#[test]
fn binary_path_accepts_root_path_slash_when_constructed() {
    let path = PathBuf::from("/");
    let result = BinaryPath::new(path.clone());
    assert_eq!(result, Ok(BinaryPath(path)));
    assert_eq!(result.unwrap().as_path(), Path::new("/"));
}

#[test]
fn binary_path_rejects_path_with_tilde_prefix_when_constructed() {
    let path = PathBuf::from("~/bin/foo");
    let result = BinaryPath::new(path);
    assert!(matches!(
        result,
        Err(BinaryRegistryError::NonAbsolutePath { path: ref p }) if p == "~/bin/foo"
    ));
}

#[test]
fn binary_path_handles_max_length_path_without_panic_when_constructed() {
    let long_path = format!("/{}", "a".repeat(4097));
    let path = PathBuf::from(&long_path);
    let result = BinaryPath::new(path.clone());
    match result {
        Ok(bp) => assert_eq!(bp.as_path(), path.as_path()),
        Err(BinaryRegistryError::NonAbsolutePath { .. }) => {}
        Err(_) => panic!("unexpected error variant"),
    }
}

#[test]
fn binary_path_handles_null_byte_path_without_panic_when_constructed() {
    let path = PathBuf::from("/tmp/\x00evil");
    let result = BinaryPath::new(path);
    match result {
        Ok(_) | Err(BinaryRegistryError::NonAbsolutePath { .. }) => {}
        Err(_) => panic!("unexpected error variant for null byte path"),
    }
}

#[test]
fn binary_path_accepts_unicode_absolute_path_when_constructed() {
    let path = PathBuf::from("/tmp/café/binary");
    let result = BinaryPath::new(path.clone());
    assert_eq!(result, Ok(BinaryPath(path)));
    assert_eq!(result.unwrap().as_path(), Path::new("/tmp/café/binary"));
}

// =========================================================================
// RegistrationStatus (B-REG-08..09)
// =========================================================================

#[test]
fn registration_status_has_exactly_two_variants_when_checked() {
    let active = RegistrationStatus::Active;
    let deactivated = RegistrationStatus::Deactivated;
    assert_ne!(active, deactivated);
    match active {
        RegistrationStatus::Active => {}
        RegistrationStatus::Deactivated => panic!("wrong variant"),
    }
    match deactivated {
        RegistrationStatus::Active => panic!("wrong variant"),
        RegistrationStatus::Deactivated => {}
    }
    assert!(std::mem::size_of::<RegistrationStatus>() <= std::mem::size_of::<u8>());
}

#[test]
fn registration_status_serde_round_trips_for_both_variants() {
    let active = RegistrationStatus::Active;
    let json_active = serde_json::to_value(active).expect("serialize active");
    let restored_active: RegistrationStatus =
        serde_json::from_value(json_active).expect("deserialize active");
    assert_eq!(restored_active, active);

    let deactivated = RegistrationStatus::Deactivated;
    let json_deact = serde_json::to_value(deactivated).expect("serialize deactivated");
    let restored_deact: RegistrationStatus =
        serde_json::from_value(json_deact).expect("deserialize deactivated");
    assert_eq!(restored_deact, deactivated);
}

// =========================================================================
// WorkflowRegistration (B-REG-10..11)
// =========================================================================

#[test]
fn workflow_registration_holds_all_fields_when_constructed() {
    let workflow_name = wn("test-workflow");
    let versioned_path = BinaryPath(PathBuf::from("/var/wtf/versions/abc123"));
    let binary_hash = bh("abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789");
    let status = RegistrationStatus::Active;
    let definition =
        WorkflowDefinition::parse(valid_single_node_json().as_bytes()).expect("valid definition");

    let reg = WorkflowRegistration {
        workflow_name: workflow_name.clone(),
        versioned_path: versioned_path.clone(),
        binary_hash: binary_hash.clone(),
        status,
        definition: definition.clone(),
    };

    assert_eq!(reg.workflow_name, workflow_name);
    assert_eq!(reg.versioned_path, versioned_path);
    assert_eq!(reg.binary_hash, binary_hash);
    assert_eq!(reg.status, RegistrationStatus::Active);
    assert_eq!(reg.definition, definition);
}

#[test]
fn workflow_registration_serde_round_trips_for_valid_registration() {
    let workflow_name = wn("serde-test");
    let versioned_path = BinaryPath(PathBuf::from("/var/wtf/versions/hash123"));
    let binary_hash = bh("abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789");
    let status = RegistrationStatus::Active;
    let definition =
        WorkflowDefinition::parse(valid_single_node_json().as_bytes()).expect("valid definition");

    let original = WorkflowRegistration {
        workflow_name,
        versioned_path,
        binary_hash,
        status,
        definition,
    };

    let json = serde_json::to_value(&original).expect("serialize");
    let restored: WorkflowRegistration = serde_json::from_value(json).expect("deserialize");
    assert_eq!(restored, original);
}

// =========================================================================
// BinaryRegistry::new (B-REG-12..13, B-REG-70)
// =========================================================================

#[test]
fn registry_creates_empty_registry_when_versions_dir_is_absolute() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let versions_dir = BinaryPath::new(temp_dir.path().to_path_buf()).expect("absolute path");
    let registry = BinaryRegistry::new(versions_dir).expect("empty registry");
    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);
}

#[test]
fn registry_rejects_relative_versions_dir_with_non_absolute_error_containing_exact_path_when_path_not_absolute(
) {
    let relative = PathBuf::from("relative/versions");
    let result = BinaryPath::new(relative.clone());
    assert!(matches!(
        result,
        Err(BinaryRegistryError::NonAbsolutePath { path: ref p }) if p == "relative/versions"
    ));
}

#[test]
fn registry_accepts_non_existent_versions_dir_when_path_is_absolute() {
    let non_existent = PathBuf::from("/tmp/nonexistent-dir-xyz-12345");
    let versions_dir = BinaryPath::new(non_existent).expect("absolute path");
    let registry = BinaryRegistry::new(versions_dir).expect("registry accepts non-existent");
    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);
}

// =========================================================================
// list / len / is_empty (B-REG-43..47, B-REG-69)
// =========================================================================

#[test]
fn list_returns_exact_entries_matching_all_registrations_when_registry_has_entries() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let versions_dir = BinaryPath::new(temp_dir.path().to_path_buf()).expect("absolute");
    let registry = BinaryRegistry::new(versions_dir).expect("registry");

    let def =
        WorkflowDefinition::parse(valid_single_node_json().as_bytes()).expect("valid definition");

    let wf_a = wn("wf-a");
    let wf_b = wn("wf-b");
    let bp_a = BinaryPath(PathBuf::from("/var/wtf/versions/a"));
    let bp_b = BinaryPath(PathBuf::from("/var/wtf/versions/b"));
    let hash_a = bh("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    let hash_b = bh("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");

    registry.inner.insert(
        wf_a.clone(),
        WorkflowRegistration {
            workflow_name: wf_a.clone(),
            versioned_path: bp_a,
            binary_hash: hash_a,
            status: RegistrationStatus::Active,
            definition: def.clone(),
        },
    );
    registry.inner.insert(
        wf_b.clone(),
        WorkflowRegistration {
            workflow_name: wf_b.clone(),
            versioned_path: bp_b,
            binary_hash: hash_b,
            status: RegistrationStatus::Active,
            definition: def,
        },
    );

    let entries = registry.list();
    assert_eq!(entries.len(), 2);
    let mut names: Vec<&str> = entries.iter().map(|(n, _)| n.as_str()).collect();
    names.sort();
    assert_eq!(names, vec!["wf-a", "wf-b"]);
}

#[test]
fn list_returns_empty_vec_when_registry_is_empty() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let versions_dir = BinaryPath::new(temp_dir.path().to_path_buf()).expect("absolute");
    let registry = BinaryRegistry::new(versions_dir).expect("registry");
    let entries = registry.list();
    assert!(entries.is_empty());
}

#[test]
fn len_returns_correct_count_when_registry_has_entries() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let versions_dir = BinaryPath::new(temp_dir.path().to_path_buf()).expect("absolute");
    let registry = BinaryRegistry::new(versions_dir).expect("registry");

    let def =
        WorkflowDefinition::parse(valid_single_node_json().as_bytes()).expect("valid definition");

    registry.inner.insert(
        wn("wf-0"),
        WorkflowRegistration {
            workflow_name: wn("wf-0"),
            versioned_path: BinaryPath(PathBuf::from("/var/wtf/versions/0")),
            binary_hash: bh(&"a".repeat(64)),
            status: RegistrationStatus::Active,
            definition: def.clone(),
        },
    );
    registry.inner.insert(
        wn("wf-1"),
        WorkflowRegistration {
            workflow_name: wn("wf-1"),
            versioned_path: BinaryPath(PathBuf::from("/var/wtf/versions/1")),
            binary_hash: bh(&"a".repeat(64)),
            status: RegistrationStatus::Active,
            definition: def.clone(),
        },
    );
    registry.inner.insert(
        wn("wf-2"),
        WorkflowRegistration {
            workflow_name: wn("wf-2"),
            versioned_path: BinaryPath(PathBuf::from("/var/wtf/versions/2")),
            binary_hash: bh(&"a".repeat(64)),
            status: RegistrationStatus::Active,
            definition: def,
        },
    );

    assert_eq!(registry.len(), 3);
}

#[test]
fn is_empty_returns_false_when_registry_has_entries() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let versions_dir = BinaryPath::new(temp_dir.path().to_path_buf()).expect("absolute");
    let registry = BinaryRegistry::new(versions_dir).expect("registry");

    let def =
        WorkflowDefinition::parse(valid_single_node_json().as_bytes()).expect("valid definition");

    registry.inner.insert(
        wn("wf-1"),
        WorkflowRegistration {
            workflow_name: wn("wf-1"),
            versioned_path: BinaryPath(PathBuf::from("/var/wtf/versions/1")),
            binary_hash: bh(&"a".repeat(64)),
            status: RegistrationStatus::Active,
            definition: def,
        },
    );

    assert!(!registry.is_empty());
}

#[test]
fn is_empty_returns_true_when_registry_is_empty() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let versions_dir = BinaryPath::new(temp_dir.path().to_path_buf()).expect("absolute");
    let registry = BinaryRegistry::new(versions_dir).expect("registry");
    assert!(registry.is_empty());
}

#[test]
fn list_returns_single_entry_when_registry_has_exactly_one_registration() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let versions_dir = BinaryPath::new(temp_dir.path().to_path_buf()).expect("absolute");
    let registry = BinaryRegistry::new(versions_dir).expect("registry");

    let def =
        WorkflowDefinition::parse(valid_single_node_json().as_bytes()).expect("valid definition");

    registry.inner.insert(
        wn("solo-wf"),
        WorkflowRegistration {
            workflow_name: wn("solo-wf"),
            versioned_path: BinaryPath(PathBuf::from("/var/wtf/versions/solo")),
            binary_hash: bh(&"a".repeat(64)),
            status: RegistrationStatus::Active,
            definition: def,
        },
    );

    let entries = registry.list();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].0.as_str(), "solo-wf");
}

// =========================================================================
// ReaperReport (B-REG-48..49)
// =========================================================================

#[test]
fn reaper_report_defaults_to_empty_vectors_when_no_work_done() {
    let report = ReaperReport::default();
    assert!(report.reaped.is_empty());
    assert!(report.skipped.is_empty());
    assert!(report.failures.is_empty());
}

#[test]
fn reaper_report_holds_reaped_skipped_and_failures_when_populated() {
    let name1 = wn("reaped-wf");
    let name2 = wn("skipped-wf");
    let name3 = wn("fail-wf");
    let err = BinaryRegistryError::ReaperDeleteFailed {
        path: BinaryPath(PathBuf::from("/var/wtf/versions/fail")),
        source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "perm denied"),
    };

    let report = ReaperReport {
        reaped: vec![name1.clone()],
        skipped: vec![name2.clone()],
        failures: vec![(name3.clone(), err)],
    };

    assert_eq!(report.reaped, vec![name1]);
    assert_eq!(report.skipped, vec![name2]);
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].0, name3);
    assert!(matches!(
        &report.failures[0].1,
        BinaryRegistryError::ReaperDeleteFailed { .. }
    ));
}

// =========================================================================
// BinaryRegistryError display (B-REG-50)
// =========================================================================

#[test]
fn binary_registry_error_displays_exact_variant_specific_message_for_all_variants() {
    let err = BinaryRegistryError::BinaryNotFound {
        path: BinaryPath(PathBuf::from("/fake/path")),
    };
    assert!(err.to_string().contains("binary not found at path"));

    let err = BinaryRegistryError::NotExecutable {
        path: BinaryPath(PathBuf::from("/fake/path")),
    };
    assert!(err.to_string().contains("binary is not executable"));

    let err = BinaryRegistryError::HashFailed {
        path: BinaryPath(PathBuf::from("/fake/path")),
        source: std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe broke"),
    };
    assert!(err.to_string().contains("failed to hash binary"));

    let err = BinaryRegistryError::CopyFailed {
        src: BinaryPath(PathBuf::from("/src")),
        dst: BinaryPath(PathBuf::from("/dst")),
        source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
    };
    assert!(err.to_string().contains("failed to copy binary"));

    let err = BinaryRegistryError::GraphDiscoveryFailed {
        workflow_name: wn("test-wf"),
        exit_code: 1,
        stderr: "graph error".to_string(),
    };
    assert!(err.to_string().contains("graph failed"));

    let err = BinaryRegistryError::InvalidGraphOutput {
        workflow_name: wn("bad-json"),
        parse_error: "expected value at line 1".to_string(),
    };
    assert!(err.to_string().contains("not valid JSON"));

    let err = BinaryRegistryError::WorkflowDeactivated {
        workflow_name: wn("my-wf"),
    };
    assert_eq!(err.to_string(), "workflow 'my-wf' is deactivated");

    let err = BinaryRegistryError::NotFound {
        workflow_name: wn("missing"),
    };
    assert_eq!(err.to_string(), "workflow 'missing' not found in registry");

    let err = BinaryRegistryError::ReaperDeleteFailed {
        path: BinaryPath(PathBuf::from("/var/wtf/versions/x")),
        source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "no perm"),
    };
    assert!(err.to_string().contains("failed to delete"));

    let err = BinaryRegistryError::NonAbsolutePath {
        path: "relative/path".to_string(),
    };
    assert!(err.to_string().contains("absolute"));

    let err = BinaryRegistryError::WorkflowDefinitionInvalid {
        workflow_name: wn("empty-def"),
        reason: "empty nodes list".to_string(),
    };
    assert!(err.to_string().contains("validation failed"));
}

// =========================================================================
// reap empty registry (B-REG-53)
// =========================================================================

#[test]
fn reap_returns_empty_report_when_registry_is_empty() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let versions_dir = BinaryPath::new(temp_dir.path().to_path_buf()).expect("absolute");
    let registry = BinaryRegistry::new(versions_dir).expect("registry");
    let report = registry.reap(|_| false);
    assert!(report.reaped.is_empty());
    assert!(report.skipped.is_empty());
    assert!(report.failures.is_empty());
}

mod proptests {
    use super::*;
    use proptest::prelude::*;
    use std::path::PathBuf;

    proptest::proptest! {
        #[test]
        fn binary_path_new_accepts_absolute_paths(s in "/[a-zA-Z0-9_./-]{1,255}") {
            let path = PathBuf::from(&s);
            let result = BinaryPath::new(path.clone());
            prop_assert!(result.is_ok());
            let bp = result.unwrap();
            prop_assert_eq!(bp.as_path(), path.as_path());
        }

        #[test]
        fn binary_path_new_rejects_non_absolute_paths(s in "[a-zA-Z0-9_.-]{1}[a-zA-Z0-9_./-]{0,254}") {
            let path = PathBuf::from(&s);
            let result = BinaryPath::new(path);
            let is_non_absolute = matches!(result, Err(BinaryRegistryError::NonAbsolutePath { .. }));
            prop_assert!(is_non_absolute);
        }

        #[test]
        fn binary_path_parent_is_absolute_for_multi_component(
            s in "/[a-zA-Z0-9]/[a-zA-Z0-9_./-]{1,255}"
        ) {
            let path = PathBuf::from(&s);
            let bp = BinaryPath::new(path).expect("absolute");
            prop_assert!(bp.parent().starts_with("/"));
        }

        #[test]
        fn registration_status_serde_round_trip(status in proptest::option::of(0u8..2)) {
            let status = match status {
                Some(0) | None => RegistrationStatus::Active,
                Some(1) => RegistrationStatus::Deactivated,
                Some(_) => unreachable!(),
            };
            let json = serde_json::to_value(status).expect("serialize");
            let restored: RegistrationStatus = serde_json::from_value(json).expect("deserialize");
            prop_assert_eq!(restored, status);
        }

        #[test]
        fn sha256_hex_is_always_64_chars(bytes in proptest::collection::vec(proptest::arbitrary::any::<u8>(), 0..=4096)) {
            use sha2::{Sha256, Digest};
            let hash = Sha256::digest(&bytes);
            let hex = format!("{:x}", hash);
            prop_assert_eq!(hex.len(), 64);
        }

        #[test]
        fn sha256_hex_is_valid_binary_hash(bytes in proptest::collection::vec(proptest::arbitrary::any::<u8>(), 0..=1024)) {
            use sha2::{Sha256, Digest};
            let hash = Sha256::digest(&bytes);
            let hex = format!("{:x}", hash);
            prop_assert!(BinaryHash::parse(&hex).is_ok());
        }
    }
}
