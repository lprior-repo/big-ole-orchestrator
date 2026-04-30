//! Consolidated unit-level tests for vo-cli.
//!
//! Replaces duplicated test files across the vo-cli test suite.
//! Covers: type properties, error displays, exit codes, numeric parsing,
//! default configs, utility functions, handler registry, and report structures.

use std::path::{Path, PathBuf};

use vo_cli::utils::{file_hash, sha256_hex};
use vo_cli::{
    interpret_cli_from, map_error_to_exit_code, parse_strict_numeric, BinaryFormat, CategoryReport,
    CheckCategory, CheckError, CheckResult, Cli, CliError, Command, CommandContext, DoctorError,
    DoctorReport, GcConfig, GcError, HandlerRegistry, HistoryConfig, HistoryError, InitConfig,
    InitError, LockError, RebuildError, Severity,
};

// ---------------------------------------------------------------------------
// 1. parse_strict_numeric rstest
// ---------------------------------------------------------------------------

#[rstest::rstest]
#[case::zero("0", Some(0))]
#[case::one("1", Some(1))]
#[case::large("999999", Some(999999))]
#[case::leading_zeros("007", Some(7))]
#[case::zeros_only("000", Some(0))]
#[case::max_u64("18446744073709551615", Some(u64::MAX))]
#[case::max_minus_one("18446744073709551614", Some(u64::MAX - 1))]
#[case::trillion("1000000000000", Some(1000000000000))]
#[case::negative("-1", None)]
#[case::minus_zero("-0", None)]
#[case::leading_plus("+42", None)]
#[case::plus_five("+5", None)]
#[case::empty("", None)]
#[case::letters("abc", None)]
#[case::overflow("18446744073709551616", None)]
#[case::float("3.14", None)]
#[case::binary("0b1010", None)]
#[case::octal("0o777", None)]
#[case::hex("0x10", None)]
#[case::alphanumeric("12abc34", None)]
#[case::space_prefix(" 42", None)]
#[case::space_suffix("42 ", None)]
#[case::tab_prefix("\t42", None)]
#[case::newline_suffix("42\n", None)]
fn parse_strict_numeric_cases(#[case] input: &str, #[case] expected: Option<u64>) {
    let result = parse_strict_numeric(input);
    match expected {
        Some(val) => assert_eq!(result.unwrap(), val, "input: {input:?}"),
        None => assert!(result.is_err(), "input: {input:?} should be rejected"),
    }
}

#[rstest::rstest]
#[case::empty("", "empty")]
#[case::plus("+5", "plus")]
#[case::negative("-1", "negative")]
#[case::overflow("18446744073709551616", "overflow")]
#[case::invalid("abc", "invalid")]
fn parse_strict_numeric_error_messages(#[case] input: &str, #[case] expected_substring: &str) {
    let err = parse_strict_numeric(input).unwrap_err();
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains(expected_substring),
        "error message {msg:?} should contain {expected_substring:?} for input {input:?}"
    );
}

// ---------------------------------------------------------------------------
// 2. Exit code mapping
// ---------------------------------------------------------------------------

fn make_cli_errors() -> Vec<(CliError, i32)> {
    vec![
        (CliError::Dispatch("fail".into()), 1),
        (CliError::InvalidNumeric("abc".into()), 2),
        (
            CliError::Gc(GcError::EngineUnreachable {
                url: "http://x".into(),
                reason: "timeout".into(),
            }),
            1,
        ),
        (
            CliError::Init(InitError::DirNotFound { path: "/x".into() }),
            1,
        ),
        (
            CliError::Lock(LockError::NotInitialized { path: "/x".into() }),
            1,
        ),
        (
            CliError::Doctor(DoctorError::NotInitialized { path: "/x".into() }),
            1,
        ),
        (
            CliError::Rebuild(RebuildError::NotInitialized { path: "/x".into() }),
            1,
        ),
        (
            CliError::Check(CheckError::FileNotFound { path: "/x".into() }),
            1,
        ),
    ]
}

#[test]
fn exit_code_all_command_errors_are_1() {
    for (err, expected) in make_cli_errors() {
        assert_eq!(
            map_error_to_exit_code(&err),
            expected,
            "exit code for {err:?}"
        );
    }
}

#[rstest::rstest]
#[case::dispatch_error(CliError::Dispatch("fail".into()), 1)]
#[case::invalid_numeric(CliError::InvalidNumeric("abc".into()), 2)]
fn exit_code_individual_cases(#[case] err: CliError, #[case] expected: i32) {
    assert_eq!(map_error_to_exit_code(&err), expected);
}

#[test]
fn exit_code_clap_help_is_0() {
    let result = interpret_cli_from(&["vo", "--version"]);
    let err = result.unwrap_err();
    assert_eq!(map_error_to_exit_code(&CliError::Clap(err)), 0);
}

#[test]
fn exit_code_clap_unknown_flag_is_2() {
    let result = interpret_cli_from(&["vo", "--unknown-flag"]);
    let err = result.unwrap_err();
    assert_eq!(map_error_to_exit_code(&CliError::Clap(err)), 2);
}

// ---------------------------------------------------------------------------
// 3. Error Display tests -- parameterized by error type
// ---------------------------------------------------------------------------

#[test]
fn rebuild_error_display_all_variants() {
    let cases: Vec<(RebuildError, &str)> = vec![
        (RebuildError::NotInitialized { path: "/x".into() }, "/x"),
        (RebuildError::ProjectionNotFound("my-agg".into()), "my-agg"),
        (RebuildError::RebuildFailed("disk full".into()), "disk full"),
        (RebuildError::UnsupportedSchemaVersion(255), "255"),
        (RebuildError::RebuildInProgress("orders".into()), "orders"),
        (
            RebuildError::IdempotencyMismatch {
                expected: "k1".into(),
                actual: "k2".into(),
            },
            "mismatch",
        ),
        (RebuildError::Engine("conn refused".into()), "conn refused"),
        (
            RebuildError::Io {
                path: "/data".into(),
                reason: "read failed".into(),
                source: std::io::Error::new(std::io::ErrorKind::Other, "io"),
            },
            "read failed",
        ),
    ];
    for (err, expected_substring) in cases {
        let msg = err.to_string();
        assert!(
            msg.contains(expected_substring),
            "RebuildError::{err:?} display {msg:?} should contain {expected_substring:?}"
        );
    }
}

#[test]
fn init_error_display_all_variants() {
    let cases: Vec<(InitError, &str)> = vec![
        (InitError::DirNotFound { path: "/x".into() }, "/x"),
        (InitError::NotDirectory { path: "/y".into() }, "/y"),
        (InitError::AlreadyInitialized { path: "/z".into() }, "/z"),
        (
            InitError::PermissionDenied {
                path: "/p".into(),
                reason: "denied".into(),
            },
            "denied",
        ),
        (
            InitError::Io {
                path: "/io".into(),
                reason: "read err".into(),
                source: std::io::Error::new(std::io::ErrorKind::Other, "io"),
            },
            "read err",
        ),
        (
            InitError::SymlinkTarget {
                path: "/sym".into(),
            },
            "symlink",
        ),
    ];
    for (err, expected_substring) in cases {
        let msg = err.to_string().to_lowercase();
        assert!(
            msg.contains(expected_substring),
            "InitError::{err:?} display {msg:?} should contain {expected_substring:?}"
        );
    }
}

#[test]
fn lock_error_display_all_variants() {
    let cases: Vec<(LockError, &str)> = vec![
        (LockError::NotInitialized { path: "/x".into() }, "/x"),
        (LockError::NoWorkflowsDir { path: "/wf".into() }, "/wf"),
        (LockError::Empty { path: "/e".into() }, "/e"),
        (
            LockError::Io {
                path: "/io".into(),
                reason: "read err".into(),
                source: std::io::Error::new(std::io::ErrorKind::Other, "io"),
            },
            "read err",
        ),
        (
            LockError::LockWrite {
                reason: "disk full".into(),
            },
            "disk full",
        ),
    ];
    for (err, expected_substring) in cases {
        let msg = err.to_string().to_lowercase();
        assert!(
            msg.contains(expected_substring),
            "LockError::{err:?} display {msg:?} should contain {expected_substring:?}"
        );
    }
}

#[test]
fn gc_error_display_all_variants() {
    let cases: Vec<(GcError, &str)> = vec![
        (
            GcError::EngineUnreachable {
                url: "http://x".into(),
                reason: "timeout".into(),
            },
            "timeout",
        ),
        (
            GcError::EngineHttpError {
                url: "http://x".into(),
                status: 503,
            },
            "503",
        ),
        (
            GcError::InvalidApiResponse {
                reason: "bad json".into(),
            },
            "bad json",
        ),
        (
            GcError::VersionsDirNotFound {
                path: PathBuf::from("/v"),
            },
            "/v",
        ),
        (
            GcError::DeleteFailed {
                path: PathBuf::from("/d"),
                source: std::io::Error::new(std::io::ErrorKind::Other, "io"),
            },
            "/d",
        ),
    ];
    for (err, expected_substring) in cases {
        let msg = err.to_string();
        assert!(
            msg.contains(expected_substring),
            "GcError::{err:?} display {msg:?} should contain {expected_substring:?}"
        );
    }
}

#[test]
fn doctor_error_display_all_variants() {
    let cases: Vec<(DoctorError, &str)> = vec![
        (DoctorError::NotInitialized { path: "/x".into() }, "/x"),
        (
            DoctorError::Io {
                path: "/io".into(),
                reason: "read err".into(),
                source: std::io::Error::new(std::io::ErrorKind::Other, "io"),
            },
            "read err",
        ),
    ];
    for (err, expected_substring) in cases {
        let msg = err.to_string();
        assert!(
            msg.contains(expected_substring),
            "DoctorError::{err:?} display {msg:?} should contain {expected_substring:?}"
        );
    }
}

#[test]
fn check_error_display_all_variants() {
    let cases: Vec<(CheckError, &str)> = vec![
        (CheckError::FileNotFound { path: "/x".into() }, "/x"),
        (CheckError::NotRegularFile { path: "/y".into() }, "/y"),
        (CheckError::FileTooSmall { path: "/s".into() }, "/s"),
        (
            CheckError::InvalidMagic {
                path: "/m".into(),
                magic: [0xDE, 0xAD, 0xBE, 0xEF],
            },
            "/m",
        ),
        (CheckError::PermissionDenied { path: "/p".into() }, "/p"),
        (
            CheckError::Io {
                path: "/io".into(),
                source: std::io::Error::new(std::io::ErrorKind::Other, "io"),
            },
            "/io",
        ),
        (
            CheckError::WorkflowSpec {
                path: "/w".into(),
                message: "bad spec".into(),
            },
            "bad spec",
        ),
    ];
    for (err, expected_substring) in cases {
        let msg = err.to_string();
        assert!(
            msg.contains(expected_substring),
            "CheckError::{err:?} display {msg:?} should contain {expected_substring:?}"
        );
    }
}

#[test]
fn history_error_display_all_variants() {
    let cases: Vec<(HistoryError, &str)> = vec![
        (
            HistoryError::HistoryFileNotFound {
                path: PathBuf::from("/h"),
            },
            "/h",
        ),
        (
            HistoryError::ReadFailed {
                reason: "io err".into(),
            },
            "io err",
        ),
        (
            HistoryError::WriteFailed {
                reason: "disk full".into(),
            },
            "disk full",
        ),
        (
            HistoryError::InvalidFormat {
                reason: "bad json".into(),
            },
            "bad json",
        ),
    ];
    for (err, expected_substring) in cases {
        let msg = err.to_string();
        assert!(
            msg.contains(expected_substring),
            "HistoryError::{err:?} display {msg:?} should contain {expected_substring:?}"
        );
    }
}

#[test]
fn cli_error_display_all_variants() {
    let cases: Vec<(CliError, &str)> = vec![
        (CliError::Dispatch("dispatch fail".into()), "dispatch fail"),
        (CliError::InvalidNumeric("abc".into()), "abc"),
    ];
    for (err, expected_substring) in cases {
        let msg = err.to_string();
        assert!(
            msg.contains(expected_substring),
            "CliError::{err:?} display {msg:?} should contain {expected_substring:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// 4. Default config tests
// ---------------------------------------------------------------------------

#[test]
fn init_config_default_values() {
    let cfg = InitConfig::default();
    assert_eq!(cfg.project_dir, PathBuf::from("."));
    assert_eq!(cfg.engine_url, "http://localhost:3000");
    assert_eq!(cfg.storage_path, PathBuf::from(".vo/storage"));
}

#[test]
fn gc_config_default_values() {
    let cfg = GcConfig::default();
    assert_eq!(cfg.engine_url, "http://localhost:3000");
    assert_eq!(cfg.versions_dir, PathBuf::from("/var/wtf/versions"));
    assert!(!cfg.dry_run);
}

#[test]
fn history_config_default_values() {
    let cfg = HistoryConfig::default();
    assert_eq!(cfg.history_path, PathBuf::from(".vo/command_history.json"));
    assert_eq!(cfg.workflow_name, "default");
}

// ---------------------------------------------------------------------------
// 5. Type property tests
// ---------------------------------------------------------------------------

#[test]
fn command_equality_and_clone() {
    let c1 = Command::Check {
        workflow: false,
        path: PathBuf::from("/bin/test"),
    };
    let c2 = Command::Check {
        workflow: false,
        path: PathBuf::from("/bin/test"),
    };
    let c3 = Command::Check {
        workflow: false,
        path: PathBuf::from("/bin/other"),
    };
    assert_eq!(c1, c2);
    assert_ne!(c1, c3);
    assert_eq!(c1, c1.clone());

    let p1 = Command::Purge {
        instance: "i-1".into(),
        storage_path: PathBuf::from(".vo/storage"),
        dry_run: false,
    };
    assert_eq!(p1, p1.clone());

    let r1 = Command::Rebuild {
        project_dir: PathBuf::from("."),
        projection_id: None,
        list_projections: false,
        force: false,
    };
    assert_eq!(r1, r1.clone());
}

#[test]
fn command_debug_all_variants() {
    let commands = vec![
        Command::Purge {
            instance: "i".into(),
            storage_path: PathBuf::from("."),
            dry_run: false,
        },
        Command::Check {
            workflow: false,
            path: PathBuf::from("/bin"),
        },
        Command::Compensate {
            engine_url: "http://x".into(),
            workflow_id: "wf".into(),
            force: false,
        },
        Command::Gc {
            engine_url: "http://x".into(),
            dry_run: false,
        },
        Command::Init {
            project_dir: PathBuf::from("."),
            engine_url: "http://x".into(),
            storage_path: PathBuf::from("."),
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
        Command::Status {
            engine_url: "http://x".into(),
            workflow_id: "wf".into(),
        },
        Command::Hardline {
            target: "t".into(),
            engine_url: "http://x".into(),
            timeout: 60,
            force: false,
            dry_run: false,
        },
        Command::Serve {
            host: "0.0.0.0".into(),
            port: 3000,
            storage_path: PathBuf::from("."),
        },
        Command::History {
            instance_id: "id".into(),
            engine_url: "http://x".into(),
            json: false,
            canonical: false,
        },
    ];
    for cmd in &commands {
        let debug = format!("{cmd:?}");
        assert!(
            !debug.is_empty(),
            "Command {:?} should have Debug output",
            cmd
        );
    }
}

#[test]
fn binary_format_display_names() {
    assert_eq!(BinaryFormat::Elf.display_name(), "valid ELF binary");
    assert_eq!(
        BinaryFormat::MachO32BigEndian.display_name(),
        "valid Mach-O 32-bit binary"
    );
    assert_eq!(
        BinaryFormat::MachO32LittleEndian.display_name(),
        "valid Mach-O 32-bit binary"
    );
    assert_eq!(
        BinaryFormat::MachO64BigEndian.display_name(),
        "valid Mach-O 64-bit binary"
    );
    assert_eq!(
        BinaryFormat::MachO64LittleEndian.display_name(),
        "valid Mach-O 64-bit binary"
    );
}

#[test]
fn severity_ordering() {
    assert!(Severity::Error > Severity::Warn);
    assert!(Severity::Warn > Severity::Info);
    assert!(Severity::Info <= Severity::Info);
    assert!(Severity::Warn >= Severity::Warn);
}

#[test]
fn check_category_display_all() {
    assert_eq!(CheckCategory::Workspace.to_string(), "workspace");
    assert_eq!(CheckCategory::LockState.to_string(), "lock-state");
    assert_eq!(
        CheckCategory::SubprocessLiveness.to_string(),
        "subprocess-liveness"
    );
    assert_eq!(
        CheckCategory::StorageIntegrity.to_string(),
        "storage-integrity"
    );
    assert_eq!(
        CheckCategory::ConfigValidation.to_string(),
        "config-validation"
    );
}

// ---------------------------------------------------------------------------
// 6. HandlerRegistry tests
// ---------------------------------------------------------------------------

fn cli_for(name: &str) -> Cli {
    match name {
        "purge" => Cli {
            command: Command::Purge {
                instance: "test".into(),
                storage_path: PathBuf::from(".vo/storage"),
                dry_run: false,
            },
        },
        "check" => Cli {
            command: Command::Check {
                workflow: false,
                path: PathBuf::from("/bin/test"),
            },
        },
        "compensate" => Cli {
            command: Command::Compensate {
                engine_url: "http://localhost:3000".into(),
                workflow_id: "wf".into(),
                force: false,
            },
        },
        "gc" => Cli {
            command: Command::Gc {
                engine_url: "http://localhost:3000".into(),
                dry_run: false,
            },
        },
        "init" => Cli {
            command: Command::Init {
                project_dir: PathBuf::from("."),
                engine_url: "http://localhost:3000".into(),
                storage_path: PathBuf::from(".vo/storage"),
            },
        },
        "lock" => Cli {
            command: Command::Lock {
                project_dir: PathBuf::from("."),
            },
        },
        "doctor" => Cli {
            command: Command::Doctor {
                project_dir: PathBuf::from("."),
            },
        },
        "rebuild" => Cli {
            command: Command::Rebuild {
                project_dir: PathBuf::from("."),
                projection_id: None,
                list_projections: false,
                force: false,
            },
        },
        "status" => Cli {
            command: Command::Status {
                engine_url: "http://localhost:3000".into(),
                workflow_id: "wf".into(),
            },
        },
        "hardline" => Cli {
            command: Command::Hardline {
                target: "t".into(),
                engine_url: "http://localhost:3000".into(),
                timeout: 60,
                force: false,
                dry_run: false,
            },
        },
        "serve" => Cli {
            command: Command::Serve {
                host: "127.0.0.1".into(),
                port: 3000,
                storage_path: PathBuf::from(".vo/storage"),
            },
        },
        "history" => Cli {
            command: Command::History {
                instance_id: "id".into(),
                engine_url: "http://localhost:3000".into(),
                json: false,
                canonical: false,
            },
        },
        _ => panic!("unknown command name: {name}"),
    }
}

#[test]
fn registry_lookups_all_commands() {
    let registry = HandlerRegistry::default();
    for name in &[
        "purge",
        "check",
        "compensate",
        "gc",
        "init",
        "lock",
        "doctor",
        "rebuild",
        "status",
        "serve",
        "history",
    ] {
        let cli = cli_for(name);
        let handler = registry.get(&cli);
        assert!(handler.is_some(), "handler {name} should exist");
        assert_eq!(handler.unwrap().name(), *name);
    }
}

#[test]
fn registry_names_contain_all_commands() {
    let registry = HandlerRegistry::default();
    let names = registry.names();
    for expected in &[
        "check",
        "compensate",
        "doctor",
        "gc",
        "history",
        "init",
        "lock",
        "purge",
        "rebuild",
        "serve",
        "status",
    ] {
        assert!(
            names.contains(expected),
            "registry should contain handler {expected}"
        );
    }
}

// ---------------------------------------------------------------------------
// 7. CommandContext metadata test
// ---------------------------------------------------------------------------

#[test]
fn command_context_metadata() {
    let ctx = CommandContext::new("gc");
    ctx.set_metadata("key1", "value1");
    assert_eq!(ctx.get_metadata("key1"), Some("value1".to_string()));
    ctx.set_metadata("key1", "value2");
    assert_eq!(ctx.get_metadata("key1"), Some("value2".to_string()));
    assert_eq!(ctx.get_metadata("missing"), None);
}

#[test]
fn command_context_metadata_many_keys() {
    let ctx = CommandContext::new("test-cmd");
    for i in 0..10 {
        ctx.set_metadata(format!("k{i}"), format!("v{i}"));
    }
    for i in 0..10 {
        assert_eq!(
            ctx.get_metadata(&format!("k{i}")),
            Some(format!("v{i}")),
            "metadata key k{i}"
        );
    }
}

// ---------------------------------------------------------------------------
// 8. Utility function tests (file_hash, sha256_hex)
// ---------------------------------------------------------------------------

#[test]
fn sha256_hex_properties() {
    let result = sha256_hex("test");
    assert_eq!(result.len(), 64);

    let empty = sha256_hex("");
    assert_eq!(empty.len(), 64);
}

#[test]
fn file_hash_deterministic() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.txt");
    std::fs::write(&path, b"hello world").unwrap();
    let h1 = file_hash(&path).unwrap();
    let h2 = file_hash(&path).unwrap();
    assert_eq!(h1, h2);
    assert_eq!(h1.len(), 64);
}

#[test]
fn file_hash_different_content() {
    let dir = tempfile::tempdir().unwrap();
    let p1 = dir.path().join("a.txt");
    let p2 = dir.path().join("b.txt");
    std::fs::write(&p1, b"hello").unwrap();
    std::fs::write(&p2, b"world").unwrap();
    assert_ne!(file_hash(&p1).unwrap(), file_hash(&p2).unwrap());
}

#[test]
fn file_hash_nonexistent_file() {
    assert!(file_hash(Path::new("/nonexistent/file")).is_err());
}

// ---------------------------------------------------------------------------
// 9. DoctorReport / CategoryReport structure tests
// ---------------------------------------------------------------------------

#[test]
fn doctor_report_healthy_no_errors() {
    let report = DoctorReport {
        project_dir: PathBuf::from("."),
        categories: vec![CategoryReport {
            category: CheckCategory::Workspace,
            checks: vec![CheckResult {
                check: "x",
                severity: Severity::Info,
                message: "ok".into(),
            }],
        }],
    };
    assert!(report.is_healthy());
    assert_eq!(report.errors().count(), 0);
}

#[test]
fn doctor_report_with_errors() {
    let report = DoctorReport {
        project_dir: PathBuf::from("."),
        categories: vec![CategoryReport {
            category: CheckCategory::Workspace,
            checks: vec![
                CheckResult {
                    check: "a",
                    severity: Severity::Error,
                    message: "bad".into(),
                },
                CheckResult {
                    check: "b",
                    severity: Severity::Warn,
                    message: "meh".into(),
                },
            ],
        }],
    };
    assert!(!report.is_healthy());
    assert_eq!(report.errors().count(), 1);
    assert_eq!(report.warnings().count(), 1);
}

#[test]
fn category_report_warnings_iterator() {
    let report = CategoryReport {
        category: CheckCategory::Workspace,
        checks: vec![
            CheckResult {
                check: "a",
                severity: Severity::Info,
                message: "ok".into(),
            },
            CheckResult {
                check: "b",
                severity: Severity::Warn,
                message: "w1".into(),
            },
            CheckResult {
                check: "c",
                severity: Severity::Error,
                message: "e1".into(),
            },
            CheckResult {
                check: "d",
                severity: Severity::Warn,
                message: "w2".into(),
            },
        ],
    };
    assert_eq!(report.warnings().count(), 2);
}

#[test]
fn category_report_healthy_with_only_warnings() {
    let report = CategoryReport {
        category: CheckCategory::Workspace,
        checks: vec![CheckResult {
            check: "a",
            severity: Severity::Warn,
            message: "w".into(),
        }],
    };
    assert!(report.is_healthy());
}
