use std::path::PathBuf;
use vo_cli::commands::check::{
    CheckError, ELF_MAGIC, KNOWN_MAGICS, MACHO_MAGIC_32_BE, MACHO_MAGIC_32_LE, MACHO_MAGIC_64_BE,
    MACHO_MAGIC_64_LE,
};
use vo_cli::commands::doctor::DoctorError;
use vo_cli::commands::doctor_checks::{
    format_report, format_report_json, CategoryReport, CheckCategory, CheckResult, DoctorReport,
    Severity,
};
use vo_cli::commands::gc::{GcConfig, GcError, GcSummary};
use vo_cli::commands::init::{
    InitConfig, InitError, CONFIG_FILE_NAME, VO_DIR_NAME, WORKFLOWS_DIR_NAME,
};
use vo_cli::commands::lock::{LockConfig, LockError, LOCK_FILE_NAME};
use vo_cli::middleware::{CommandDispatcher, LoggingMiddleware, MetricsMiddleware, Middleware};
use vo_cli::{
    interpret_cli_from, map_error_to_exit_code, parse_strict_numeric, CliError, Command,
    CommandContext,
};

#[test]
fn check_error_permission_denied_display() {
    let err = CheckError::PermissionDenied {
        path: PathBuf::from("/secret/binary"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/secret/binary"));
    assert!(msg.contains("permission denied"));
}

#[test]
fn check_error_io_display() {
    let err = CheckError::Io {
        path: PathBuf::from("/bad/file"),
        source: std::io::Error::new(std::io::ErrorKind::BrokenPipe, "broken pipe"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/bad/file"));
    assert!(msg.contains("broken pipe"));
}

#[test]
fn init_error_io_display() {
    let err = InitError::Io {
        path: PathBuf::from("/some/path"),
        reason: "writing config".into(),
        source: std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe broke"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/some/path"));
    assert!(msg.contains("writing config"));
}

#[test]
fn doctor_error_io_display() {
    let err = DoctorError::Io {
        path: PathBuf::from("/proj/.vo"),
        reason: "reading metadata".into(),
        source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/proj/.vo"));
    assert!(msg.contains("reading metadata"));
}

#[test]
fn init_config_default_values() {
    let config = InitConfig::default();
    assert_eq!(config.project_dir, PathBuf::from("."));
    assert_eq!(config.engine_url, "http://localhost:3000");
    assert_eq!(config.storage_path, PathBuf::from(".vo/storage"));
}

#[test]
fn init_constants() {
    assert_eq!(VO_DIR_NAME, ".vo");
    assert_eq!(WORKFLOWS_DIR_NAME, "workflows");
    assert_eq!(CONFIG_FILE_NAME, "config.toml");
}

#[test]
fn lock_constant() {
    assert_eq!(LOCK_FILE_NAME, "vo.lock");
}

#[test]
fn gc_config_default_values() {
    let config = GcConfig::default();
    assert_eq!(config.engine_url, "http://localhost:3000");
    assert_eq!(config.versions_dir, PathBuf::from("/var/wtf/versions"));
    assert!(!config.dry_run);
}

#[test]
fn gc_summary_construction() {
    let summary = GcSummary {
        pinned_count: 5,
        scanned_count: 10,
        deleted_count: 3,
        deleted_hashes: vec!["abc".into(), "def".into()],
        failures: vec![(PathBuf::from("/x"), "err".into())],
    };
    assert_eq!(summary.pinned_count, 5);
    assert_eq!(summary.scanned_count, 10);
    assert_eq!(summary.deleted_count, 3);
    assert_eq!(summary.deleted_hashes.len(), 2);
    assert_eq!(summary.failures.len(), 1);
}

#[test]
fn command_equality() {
    let c1 = Command::Check {
        path: PathBuf::from("/tmp"),
    };
    let c2 = Command::Check {
        path: PathBuf::from("/tmp"),
    };
    assert_eq!(c1, c2);

    let c3 = Command::Check {
        path: PathBuf::from("/other"),
    };
    assert_ne!(c1, c3);
}

#[test]
fn command_clone() {
    let c1 = Command::Gc {
        engine_url: "http://test".into(),
        dry_run: true,
    };
    let c2 = c1.clone();
    assert_eq!(c1, c2);
}

#[test]
fn command_purge_equality() {
    let c1 = Command::Purge {
        instance: "abc".into(),
    };
    let c2 = Command::Purge {
        instance: "abc".into(),
    };
    assert_eq!(c1, c2);
}

#[test]
fn dispatcher_add_middleware_method() {
    let mut dispatcher = CommandDispatcher::new();
    dispatcher.add_middleware(LoggingMiddleware::new());
    dispatcher.add_middleware(MetricsMiddleware::new());
}

#[test]
fn dispatcher_default() {
    let _dispatcher = CommandDispatcher::default();
}

#[test]
fn logging_middleware_default() {
    let m = LoggingMiddleware::default();
    assert_eq!(m.name(), "logging");
}

#[test]
fn metrics_middleware_default() {
    let m = MetricsMiddleware::default();
    assert_eq!(m.name(), "metrics");
}

#[test]
fn doctor_report_is_healthy_all_categories_clean() {
    let report = DoctorReport {
        project_dir: PathBuf::from("/tmp"),
        categories: vec![
            CategoryReport::new(CheckCategory::Workspace),
            CategoryReport::new(CheckCategory::LockState),
            CategoryReport::new(CheckCategory::ConfigValidation),
        ],
    };
    assert!(report.is_healthy());
}

#[test]
fn doctor_report_is_not_healthy_with_error() {
    let mut cat = CategoryReport::new(CheckCategory::Workspace);
    cat.checks.push(CheckResult {
        check: "bad",
        severity: Severity::Error,
        message: "broken".into(),
    });
    let report = DoctorReport {
        project_dir: PathBuf::from("/tmp"),
        categories: vec![cat],
    };
    assert!(!report.is_healthy());
}

#[test]
fn doctor_report_errors_iterator() {
    let mut cat = CategoryReport::new(CheckCategory::Workspace);
    cat.checks.push(CheckResult {
        check: "ok",
        severity: Severity::Info,
        message: "fine".into(),
    });
    cat.checks.push(CheckResult {
        check: "bad",
        severity: Severity::Error,
        message: "broken".into(),
    });
    let report = DoctorReport {
        project_dir: PathBuf::from("/tmp"),
        categories: vec![cat],
    };
    let errors: Vec<_> = report.errors().collect();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].check, "bad");
}

#[test]
fn doctor_report_warnings_iterator() {
    let mut cat = CategoryReport::new(CheckCategory::Workspace);
    cat.checks.push(CheckResult {
        check: "w1",
        severity: Severity::Warn,
        message: "careful".into(),
    });
    cat.checks.push(CheckResult {
        check: "w2",
        severity: Severity::Warn,
        message: "watch out".into(),
    });
    cat.checks.push(CheckResult {
        check: "ok",
        severity: Severity::Info,
        message: "fine".into(),
    });
    let report = DoctorReport {
        project_dir: PathBuf::from("/tmp"),
        categories: vec![cat],
    };
    let warnings: Vec<_> = report.warnings().collect();
    assert_eq!(warnings.len(), 2);
}

#[test]
fn check_result_equality() {
    let r1 = CheckResult {
        check: "test",
        severity: Severity::Info,
        message: "msg".into(),
    };
    let r2 = CheckResult {
        check: "test",
        severity: Severity::Info,
        message: "msg".into(),
    };
    assert_eq!(r1, r2);
}

#[test]
fn category_report_equality() {
    let c1 = CategoryReport::new(CheckCategory::Workspace);
    let c2 = CategoryReport::new(CheckCategory::Workspace);
    assert_eq!(c1, c2);
}

#[test]
fn severity_ordering_comprehensive() {
    assert!(Severity::Info <= Severity::Info);
    assert!(Severity::Warn <= Severity::Warn);
    assert!(Severity::Error <= Severity::Error);
    assert!(Severity::Info < Severity::Warn);
    assert!(Severity::Warn < Severity::Error);
    assert!(Severity::Info < Severity::Error);
}

#[test]
fn parse_strict_numeric_rejects_negative() {
    let result = parse_strict_numeric("-5");
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("negative"));
}

#[test]
fn parse_strict_numeric_rejects_empty() {
    let result = parse_strict_numeric("");
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("empty"));
}

#[test]
fn parse_strict_numeric_rejects_leading_plus() {
    let result = parse_strict_numeric("+5");
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("plus"));
}

#[test]
fn parse_strict_numeric_accepts_zero() {
    assert_eq!(parse_strict_numeric("0").unwrap(), 0);
}

#[test]
fn parse_strict_numeric_accepts_max_u64() {
    assert_eq!(
        parse_strict_numeric("18446744073709551615").unwrap(),
        u64::MAX
    );
}

#[test]
fn parse_strict_numeric_rejects_overflow() {
    let result = parse_strict_numeric("18446744073709551616");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("overflow"));
}

#[test]
fn cli_struct_debug_format() {
    let cli = vo_cli::Cli {
        command: Command::Check {
            path: PathBuf::from("/tmp"),
        },
    };
    let debug = format!("{cli:?}");
    assert!(debug.contains("Check"));
}

#[test]
fn command_context_stores_all_variants() {
    let cmds = vec![
        Command::Purge {
            instance: "i".into(),
        },
        Command::Check {
            path: PathBuf::from("/tmp"),
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
    for cmd in cmds {
        let ctx = CommandContext::new(cmd.clone());
        assert_eq!(ctx.command, cmd);
    }
}

#[test]
fn magic_constants_are_correct() {
    assert_eq!(ELF_MAGIC, [0x7F, 0x45, 0x4C, 0x46]);
    assert_eq!(MACHO_MAGIC_32_BE, [0xFE, 0xED, 0xFA, 0xCE]);
    assert_eq!(MACHO_MAGIC_32_LE, [0xCE, 0xFA, 0xED, 0xFE]);
    assert_eq!(MACHO_MAGIC_64_BE, [0xFE, 0xED, 0xFA, 0xCF]);
    assert_eq!(MACHO_MAGIC_64_LE, [0xCF, 0xFA, 0xED, 0xFE]);
    assert_eq!(KNOWN_MAGICS.len(), 5);
}

#[test]
fn lock_error_io_display() {
    let err = LockError::Io {
        path: PathBuf::from("/proj/.vo/workflows"),
        reason: "readdir".into(),
        source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/proj/.vo/workflows"));
    assert!(msg.contains("readdir"));
}

#[test]
fn gc_error_variants_are_descriptive() {
    let err = GcError::VersionsDirNotFound {
        path: PathBuf::from("/var/wtf/versions"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/var/wtf/versions"));
    assert!(msg.contains("does not exist"));
}

#[test]
fn gc_error_engine_http_error_display() {
    let err = GcError::EngineHttpError {
        url: "http://engine:3000/api/v1/registry/pinned-hashes".into(),
        status: 500,
    };
    let msg = err.to_string();
    assert!(msg.contains("500"));
    assert!(msg.contains("HTTP"));
    assert!(msg.contains("engine:3000"));
}

#[test]
fn format_report_empty_categories() {
    let report = DoctorReport {
        project_dir: PathBuf::from("/tmp"),
        categories: vec![],
    };
    let (stdout, stderr) = format_report(&report);
    assert!(stdout.contains("Doctor Report"));
    assert!(stdout.contains("All checks passed"));
    assert!(stderr.is_empty());
}

#[test]
fn format_report_json_empty_categories() {
    let report = DoctorReport {
        project_dir: PathBuf::from("/tmp/empty"),
        categories: vec![],
    };
    let json_str = format_report_json(&report);
    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert_eq!(parsed["project_dir"].as_str().unwrap(), "/tmp/empty");
    assert!(parsed["healthy"].as_bool().unwrap());
    assert_eq!(parsed["error_count"].as_u64().unwrap(), 0);
    assert_eq!(parsed["warn_count"].as_u64().unwrap(), 0);
    assert_eq!(parsed["categories"].as_array().unwrap().len(), 0);
}

#[test]
fn format_report_json_with_errors_and_warnings() {
    let mut cat = CategoryReport::new(CheckCategory::Workspace);
    cat.checks.push(CheckResult {
        check: "e1",
        severity: Severity::Error,
        message: "err".into(),
    });
    cat.checks.push(CheckResult {
        check: "w1",
        severity: Severity::Warn,
        message: "warn".into(),
    });
    let report = DoctorReport {
        project_dir: PathBuf::from("/tmp"),
        categories: vec![cat],
    };
    let json_str = format_report_json(&report);
    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert!(!parsed["healthy"].as_bool().unwrap());
    assert_eq!(parsed["error_count"].as_u64().unwrap(), 1);
    assert_eq!(parsed["warn_count"].as_u64().unwrap(), 1);
}

#[test]
fn init_error_display_consistency() {
    let e1 = InitError::DirNotFound {
        path: PathBuf::from("/a"),
    };
    let e2 = InitError::DirNotFound {
        path: PathBuf::from("/a"),
    };
    assert_eq!(e1.to_string(), e2.to_string());
}

#[test]
fn lock_config_equality() {
    let c1 = LockConfig {
        project_dir: PathBuf::from("/proj"),
    };
    let c2 = LockConfig {
        project_dir: PathBuf::from("/proj"),
    };
    assert_eq!(c1, c2);
}
