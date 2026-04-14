use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use vo_cli::cli::{interpret_cli_from, map_error_to_exit_code, Cli, CliError, Command};
use vo_cli::commands::check::{validate_binary_header, BinaryFormat, CheckError};
use vo_cli::commands::doctor::run_doctor;
use vo_cli::commands::doctor_checks::{
    check_config_validation, check_lock_state, check_storage_integrity, check_subprocess_liveness,
    check_workspace, format_report, format_report_json, CategoryReport, CheckCategory,
    DoctorReport, Severity,
};
use vo_cli::commands::init::{run_init, InitConfig, InitError, CONFIG_FILE_NAME};
use vo_cli::commands::lock::{run_lock, LockConfig, LockError, LOCK_FILE_NAME};
use vo_cli::commands::rebuild::{run_rebuild, RebuildConfig, RebuildError, RebuildStatus};
use vo_cli::utils::file_hash;

fn setup_project(dir: &std::path::Path) {
    let vo_dir = dir.join(".vo");
    fs::create_dir_all(vo_dir.join("workflows")).unwrap();
    fs::create_dir_all(vo_dir.join("storage")).unwrap();
    fs::write(
        dir.join(CONFIG_FILE_NAME),
        "[engine]\nurl = \"http://localhost:3000\"\n\n[storage]\npath = \".vo/storage\"\n",
    )
    .unwrap();
}

#[test]
fn gc_env_default_applied_when_no_flag() {
    let cli = interpret_cli_from(vec!["vo", "gc"]).unwrap();
    if let Command::Gc { engine_url, .. } = &cli.command {
        assert_eq!(engine_url, "http://localhost:3000");
    } else {
        panic!("expected Gc");
    }
}

#[test]
fn init_storage_path_flag_only() {
    let cli = interpret_cli_from(vec!["vo", "init", "--storage-path", "/data/vo"]).unwrap();
    if let Command::Init {
        storage_path,
        project_dir,
        engine_url,
    } = &cli.command
    {
        assert_eq!(*storage_path, PathBuf::from("/data/vo"));
        assert_eq!(*project_dir, PathBuf::from("."));
        assert_eq!(engine_url, "http://localhost:3000");
    } else {
        panic!("expected Init");
    }
}

#[test]
fn init_with_unicode_paths() {
    let cli = interpret_cli_from(vec![
        "vo",
        "init",
        "--project-dir",
        "/tmp/プロジェクト",
        "--storage-path",
        "/tmp/データ",
    ])
    .unwrap();
    if let Command::Init {
        project_dir,
        storage_path,
        ..
    } = &cli.command
    {
        assert!(project_dir.to_string_lossy().contains("プロジェクト"));
        assert!(storage_path.to_string_lossy().contains("データ"));
    } else {
        panic!("expected Init");
    }
}

#[test]
fn doctor_with_runtime_pid_files() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    let runtime = dir.path().join(".vo/runtime");
    fs::create_dir_all(&runtime).unwrap();

    fs::write(runtime.join("engine.pid"), "999999999\n").unwrap();

    let report = check_subprocess_liveness(&dir.path().join(".vo"));
    assert!(report
        .checks
        .iter()
        .any(|c| c.check == "process-dead" && c.severity == Severity::Error));
}

#[test]
fn doctor_with_malformed_pid_file() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    let runtime = dir.path().join(".vo/runtime");
    fs::create_dir_all(&runtime).unwrap();

    fs::write(runtime.join("bad.pid"), "not-a-number\n").unwrap();

    let report = check_subprocess_liveness(&dir.path().join(".vo"));
    assert!(report
        .checks
        .iter()
        .any(|c| c.check == "subprocess-liveness" && c.severity == Severity::Info));
}

#[test]
fn storage_integrity_with_partitions() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    let storage = dir.path().join(".vo/storage");
    for name in &["events", "instances", "timers"] {
        fs::create_dir_all(storage.join(name)).unwrap();
    }

    let report = check_storage_integrity(&dir.path().join(".vo"), dir.path());
    assert!(report
        .checks
        .iter()
        .any(|c| c.check == "storage-partitions"));
}

#[test]
fn storage_with_non_partition_files() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    let storage = dir.path().join(".vo/storage");
    fs::write(storage.join("random.dat"), b"data").unwrap();
    fs::write(storage.join("other.txt"), b"text").unwrap();

    let report = check_storage_integrity(&dir.path().join(".vo"), dir.path());
    assert!(report
        .checks
        .iter()
        .any(|c| c.check == "storage-contents" && c.message.contains("item(s)")));
}

#[test]
fn config_toml_extra_sections_ignored() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    fs::write(
        dir.path().join(CONFIG_FILE_NAME),
        "[engine]\nurl = \"http://localhost:3000\"\n\n[storage]\npath = \".vo/storage\"\n\n[extra]\nfoo = \"bar\"\n",
    )
    .unwrap();

    let report = check_config_validation(dir.path());
    assert!(report.is_healthy());
}

#[test]
fn config_storage_path_ref_valid() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());

    let report = check_storage_integrity(&dir.path().join(".vo"), dir.path());
    assert!(report
        .checks
        .iter()
        .any(|c| c.check == "storage-path-ref" && c.severity == Severity::Info));
}

#[test]
fn config_storage_path_ref_invalid() {
    let dir = tempfile::tempdir().unwrap();
    let vo_dir = dir.path().join(".vo");
    fs::create_dir_all(vo_dir.join("storage")).unwrap();
    fs::write(
        dir.path().join(CONFIG_FILE_NAME),
        "[engine]\nurl = \"http://localhost:3000\"\n\n[storage]\npath = \".vo/nonexistent\"\n",
    )
    .unwrap();

    let report = check_storage_integrity(&vo_dir, dir.path());
    assert!(report.warnings().any(|w| w.check == "storage-path-ref"));
}

#[test]
fn init_rejects_symlink_project_dir() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("target");
    fs::create_dir_all(&target).unwrap();
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&target, &link).unwrap();

    let result = run_init(&InitConfig {
        project_dir: link,
        engine_url: "http://localhost:3000".into(),
        storage_path: PathBuf::from(".vo/storage"),
    });
    assert!(matches!(result, Err(InitError::SymlinkTarget { .. })));
}

#[test]
fn lock_with_no_vo_dir() {
    let dir = tempfile::tempdir().unwrap();
    let result = run_lock(&LockConfig {
        project_dir: dir.path().to_path_buf(),
    });
    assert!(matches!(result, Err(LockError::NotInitialized { .. })));
}

#[test]
fn lock_with_no_workflows_dir() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".vo")).unwrap();
    let result = run_lock(&LockConfig {
        project_dir: dir.path().to_path_buf(),
    });
    assert!(matches!(result, Err(LockError::NoWorkflowsDir { .. })));
}

#[test]
fn rebuild_list_projections_returns_empty_list() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    let report = run_rebuild(&RebuildConfig {
        project_dir: dir.path().to_path_buf(),
        projection_id: None,
        list_projections: true,
        force: false,
        schema_version: None,
    })
    .unwrap();
    if let RebuildStatus::Listed(projs) = &report.status {
        assert!(projs.is_empty());
    } else {
        panic!("expected Listed status");
    }
}

#[test]
fn rebuild_completed_report_format() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    let report = run_rebuild(&RebuildConfig {
        project_dir: dir.path().to_path_buf(),
        projection_id: Some("orders-projection".into()),
        list_projections: false,
        force: false,
        schema_version: None,
    })
    .unwrap();
    assert!(matches!(report.status, RebuildStatus::Completed));
    let output = report.format_progress();
    assert!(output.contains("completed"));
}

#[test]
fn cli_error_from_clap_maps_to_exit_code_2() {
    let result = interpret_cli_from(vec!["vo", "--bogus"]);
    assert!(result.is_err());
    let err = CliError::Clap(result.unwrap_err());
    assert_eq!(map_error_to_exit_code(&err), 2);
}

#[test]
fn map_error_to_exit_code_help_variants() {
    let help_err =
        CliError::Clap(clap::Command::new("vo").error(clap::error::ErrorKind::DisplayHelp, "help"));
    assert_eq!(map_error_to_exit_code(&help_err), 0);

    let version_err = CliError::Clap(
        clap::Command::new("vo").error(clap::error::ErrorKind::DisplayVersion, "ver"),
    );
    assert_eq!(map_error_to_exit_code(&version_err), 0);
}

#[test]
fn format_report_single_error_only() {
    let mut cat = CategoryReport::new(CheckCategory::Workspace);
    cat.push("broken", Severity::Error, "something broke".into());
    let report = DoctorReport {
        project_dir: PathBuf::from("/x"),
        categories: vec![cat],
    };
    let (stdout, stderr) = format_report(&report);
    assert!(stderr.contains("something broke"));
    assert!(stderr.contains("error(s)"));
    assert!(!stdout.contains("All checks passed"));
}

#[test]
fn format_report_only_warnings() {
    let mut cat = CategoryReport::new(CheckCategory::ConfigValidation);
    cat.push("warn1", Severity::Warn, "be careful".into());
    cat.push("warn2", Severity::Warn, "also check".into());
    let report = DoctorReport {
        project_dir: PathBuf::from("/x"),
        categories: vec![cat],
    };
    let (stdout, stderr) = format_report(&report);
    assert!(stderr.contains("be careful"));
    assert!(stderr.contains("also check"));
    assert!(stderr.contains("warning(s)"));
    assert!(!stdout.contains("All checks passed"));
}

#[test]
fn format_report_json_empty_categories() {
    let report = DoctorReport {
        project_dir: PathBuf::from("/empty"),
        categories: vec![],
    };
    let json = format_report_json(&report);
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed["healthy"].as_bool().unwrap());
    assert_eq!(parsed["categories"].as_array().unwrap().len(), 0);
}

#[test]
fn format_report_json_error_severity() {
    let mut cat = CategoryReport::new(CheckCategory::Workspace);
    cat.push("err", Severity::Error, "bad".into());
    let report = DoctorReport {
        project_dir: PathBuf::from("/x"),
        categories: vec![cat],
    };
    let json = format_report_json(&report);
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    let sev = &parsed["categories"][0]["checks"][0]["severity"];
    assert_eq!(sev.as_str(), Some("error"));
}

#[test]
fn workspace_with_runtime_alive_pid_files() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    let runtime = dir.path().join(".vo/runtime");
    fs::create_dir_all(&runtime).unwrap();

    fs::write(
        runtime.join("self.pid"),
        format!("{}\n", std::process::id()),
    )
    .unwrap();

    let report = check_workspace(dir.path(), &dir.path().join(".vo"));
    assert!(report
        .checks
        .iter()
        .any(|c| c.check == "stale-pid-files" && c.severity == Severity::Info));
}

#[test]
fn lockfile_with_multiple_entries_verifies_all() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    let wf_dir = dir.path().join(".vo/workflows");
    fs::write(wf_dir.join("a"), b"a-content").unwrap();
    fs::write(wf_dir.join("b"), b"b-content").unwrap();
    fs::write(wf_dir.join("c"), b"c-content").unwrap();

    let lockmap = run_lock(&LockConfig {
        project_dir: dir.path().to_path_buf(),
    })
    .unwrap();
    assert_eq!(lockmap.len(), 3);

    let report = check_lock_state(dir.path(), &dir.path().join(".vo"));
    assert!(report
        .checks
        .iter()
        .any(|c| c.check == "lock-integrity" && c.severity == Severity::Info));
}

#[test]
fn cli_command_all_variants_cloneable() {
    let commands = vec![
        Command::Purge {
            instance: "x".into(),
        },
        Command::Check {
            path: PathBuf::from("/p"),
        },
        Command::Gc {
            engine_url: "http://x".into(),
            dry_run: false,
        },
        Command::Init {
            project_dir: PathBuf::from("."),
            engine_url: "http://x".into(),
            storage_path: PathBuf::from(".vo/s"),
        },
        Command::Lock {
            project_dir: PathBuf::from("."),
        },
        Command::Doctor {
            project_dir: PathBuf::from("."),
        },
        Command::Rebuild {
            project_dir: PathBuf::from("."),
            projection_id: None,
            list_projections: false,
            force: false,
        },
    ];
    for cmd in &commands {
        let _cloned = cmd.clone();
    }
}

#[test]
fn init_error_all_variants_display() {
    let e1 = InitError::DirNotFound {
        path: PathBuf::from("/d"),
    };
    assert!(e1.to_string().contains("/d"));

    let e2 = InitError::NotDirectory {
        path: PathBuf::from("/f"),
    };
    assert!(e2.to_string().contains("/f"));

    let e3 = InitError::AlreadyInitialized {
        path: PathBuf::from("/a"),
    };
    assert!(e3.to_string().contains("/a"));

    let e4 = InitError::PermissionDenied {
        path: PathBuf::from("/p"),
        reason: "denied".into(),
    };
    assert!(e4.to_string().contains("/p"));

    let e5 = InitError::SymlinkTarget {
        path: PathBuf::from("/s"),
    };
    assert!(e5.to_string().contains("symlink"));

    let e6 = InitError::Io {
        path: PathBuf::from("/io"),
        reason: "read".into(),
        source: std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe"),
    };
    assert!(e6.to_string().contains("/io"));
}

#[test]
fn rebuild_error_io_from_conversion() {
    let io_err = std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "truncated");
    let rb_err: RebuildError = io_err.into();
    assert!(rb_err.to_string().contains("truncated"));
}

#[test]
fn check_binary_symlink_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("target");
    fs::write(&target, [0x7F, 0x45, 0x4C, 0x46, 0x00]).unwrap();
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&target, &link).unwrap();

    let result = validate_binary_header(&link);
    assert!(matches!(result, Err(CheckError::NotRegularFile { .. })));
}

#[test]
fn doctor_full_report_five_categories() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());

    let report = run_doctor(&vo_cli::commands::doctor::DoctorConfig {
        project_dir: dir.path().to_path_buf(),
    })
    .unwrap();

    assert_eq!(report.categories.len(), 5);
    let categories: Vec<_> = report.categories.iter().map(|c| c.category).collect();
    assert!(categories.contains(&CheckCategory::Workspace));
    assert!(categories.contains(&CheckCategory::LockState));
    assert!(categories.contains(&CheckCategory::SubprocessLiveness));
    assert!(categories.contains(&CheckCategory::StorageIntegrity));
    assert!(categories.contains(&CheckCategory::ConfigValidation));
}

#[test]
fn gc_error_http_status_display() {
    let err = vo_cli::commands::gc::GcError::EngineHttpError {
        url: "http://engine/api".into(),
        status: 503,
    };
    let msg = err.to_string();
    assert!(msg.contains("503"));
    assert!(msg.contains("http://engine/api"));
}

#[test]
fn gc_summary_fields() {
    let summary = vo_cli::commands::gc::GcSummary {
        pinned_count: 5,
        scanned_count: 10,
        deleted_count: 3,
        deleted_hashes: vec!["abc".into(), "def".into()],
        failures: vec![(PathBuf::from("/x"), "perm denied".into())],
    };
    assert_eq!(summary.pinned_count, 5);
    assert_eq!(summary.scanned_count, 10);
    assert_eq!(summary.deleted_count, 3);
    assert_eq!(summary.deleted_hashes.len(), 2);
    assert_eq!(summary.failures.len(), 1);
}
