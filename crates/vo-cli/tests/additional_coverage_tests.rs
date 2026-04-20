use std::path::PathBuf;
use vo_cli::commands::check::{
    CheckError, ELF_MAGIC, KNOWN_MAGICS, MACHO_MAGIC_32_BE, MACHO_MAGIC_32_LE, MACHO_MAGIC_64_BE,
    MACHO_MAGIC_64_LE,
};
use vo_cli::commands::doctor::{DoctorConfig, DoctorError};
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
    CommandContext, HandlerRegistry,
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
        workflow: false,
        path: PathBuf::from("/tmp"),
    };
    let c2 = Command::Check {
        workflow: false,
        path: PathBuf::from("/tmp"),
    };
    assert_eq!(c1, c2);

    let c3 = Command::Check {
        workflow: false,
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
    let registry = vo_cli::HandlerRegistry::default();
    let mut dispatcher = CommandDispatcher::new(registry);
    dispatcher.add_middleware(LoggingMiddleware::new());
    dispatcher.add_middleware(MetricsMiddleware::new());
}

#[test]
fn dispatcher_new_with_registry() {
    let registry = vo_cli::HandlerRegistry::default();
    let _dispatcher = CommandDispatcher::new(registry);
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
            workflow: false,
            path: PathBuf::from("/tmp"),
        },
    };
    let debug = format!("{cli:?}");
    assert!(debug.contains("Check"));
}

#[test]
fn command_context_stores_command_names() {
    let names = vec!["purge", "check", "gc", "init", "lock", "doctor", "rebuild"];
    for name in names {
        let ctx = CommandContext::new(name);
        assert_eq!(ctx.command_name, name);
    }
}

#[test]
fn command_context_from_string() {
    let ctx = CommandContext::new(String::from("check"));
    assert_eq!(ctx.command_name, "check");
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

#[test]
fn registry_get_gc_handler() {
    let registry = vo_cli::HandlerRegistry::default();
    let cli = vo_cli::Cli {
        command: Command::Gc {
            engine_url: "http://test".into(),
            dry_run: true,
        },
    };
    let handler = registry.get(&cli).expect("handler found");
    assert_eq!(handler.name(), "gc");
}

#[test]
fn registry_get_init_handler() {
    let registry = vo_cli::HandlerRegistry::default();
    let cli = vo_cli::Cli {
        command: Command::Init {
            project_dir: PathBuf::from("."),
            engine_url: "http://test".into(),
            storage_path: PathBuf::from(".vo/s"),
        },
    };
    let handler = registry.get(&cli).expect("handler found");
    assert_eq!(handler.name(), "init");
}

#[test]
fn registry_get_lock_handler() {
    let registry = vo_cli::HandlerRegistry::default();
    let cli = vo_cli::Cli {
        command: Command::Lock {
            project_dir: PathBuf::from("."),
        },
    };
    let handler = registry.get(&cli).expect("handler found");
    assert_eq!(handler.name(), "lock");
}

#[test]
fn registry_get_doctor_handler() {
    let registry = vo_cli::HandlerRegistry::default();
    let cli = vo_cli::Cli {
        command: Command::Doctor {
            project_dir: PathBuf::from("."),
        },
    };
    let handler = registry.get(&cli).expect("handler found");
    assert_eq!(handler.name(), "doctor");
}

#[test]
fn registry_names_contains_all_eight() {
    let registry = vo_cli::HandlerRegistry::default();
    let names = registry.names();
    assert_eq!(names.len(), 8);
    assert!(names.contains(&"purge"));
    assert!(names.contains(&"check"));
    assert!(names.contains(&"compensate"));
    assert!(names.contains(&"gc"));
    assert!(names.contains(&"init"));
    assert!(names.contains(&"lock"));
    assert!(names.contains(&"doctor"));
    assert!(names.contains(&"rebuild"));
    assert!(names.contains(&"status"));
    assert!(names.contains(&"workspace"));
}

#[test]
fn registry_new_is_same_as_default() {
    let r1 = vo_cli::HandlerRegistry::default();
    let r2 = vo_cli::HandlerRegistry::new();
    assert_eq!(r1.names().len(), r2.names().len());
}

#[test]
fn registry_register_custom_handler() {
    struct CustomHandler;
    impl vo_cli::CommandHandler for CustomHandler {
        fn name(&self) -> &'static str {
            "custom"
        }
        fn execute(
            &self,
            _cli: &vo_cli::Cli,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), CliError>> + Send + '_>>
        {
            Box::pin(async { Ok(()) })
        }
    }
    let mut registry = vo_cli::HandlerRegistry::default();
    registry.register(Box::new(CustomHandler));
    assert!(registry.names().contains(&"custom"));
    assert_eq!(registry.names().len(), 9);
}

#[test]
fn command_context_metadata_overwrite() {
    let ctx = CommandContext::new("cmd");
    ctx.set_metadata("key", "v1");
    assert_eq!(ctx.get_metadata("key"), Some("v1".to_string()));
    ctx.set_metadata("key", "v2");
    assert_eq!(ctx.get_metadata("key"), Some("v2".to_string()));
}

#[test]
fn command_context_missing_metadata_returns_none() {
    let ctx = CommandContext::new("cmd");
    assert_eq!(ctx.get_metadata("nonexistent"), None);
}

#[test]
fn command_context_multiple_metadata_keys() {
    let ctx = CommandContext::new("cmd");
    ctx.set_metadata("a", "1");
    ctx.set_metadata("b", "2");
    assert_eq!(ctx.get_metadata("a"), Some("1".to_string()));
    assert_eq!(ctx.get_metadata("b"), Some("2".to_string()));
}

#[test]
fn gc_config_clone_preserves_values() {
    let config = GcConfig::default();
    let cloned = config.clone();
    assert_eq!(config.engine_url, cloned.engine_url);
    assert_eq!(config.versions_dir, cloned.versions_dir);
    assert_eq!(config.dry_run, cloned.dry_run);
}

#[test]
fn gc_error_engine_unreachable_display_has_url_and_reason() {
    let err = GcError::EngineUnreachable {
        url: "http://host:9999".into(),
        reason: "timeout after 30s".into(),
    };
    let msg = err.to_string();
    assert!(msg.contains("http://host:9999"));
    assert!(msg.contains("timeout after 30s"));
    assert!(msg.contains("503"));
}

#[test]
fn gc_error_delete_failed_preserves_path() {
    let err = GcError::DeleteFailed {
        path: PathBuf::from("/var/wtf/versions/deadbeef"),
        source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "no access"),
    };
    let msg = err.to_string();
    assert!(msg.contains("deadbeef"));
    assert!(msg.contains("failed to delete"));
}

#[test]
fn check_error_partial_eq_same_path_file_not_found() {
    let e1 = CheckError::FileNotFound {
        path: PathBuf::from("/a"),
    };
    let e2 = CheckError::FileNotFound {
        path: PathBuf::from("/a"),
    };
    assert_eq!(e1, e2);
}

#[test]
fn check_error_partial_eq_different_variants() {
    let e1 = CheckError::FileNotFound {
        path: PathBuf::from("/a"),
    };
    let e2 = CheckError::NotRegularFile {
        path: PathBuf::from("/a"),
    };
    assert_ne!(e1, e2);
}

#[test]
fn check_error_partial_eq_invalid_magic() {
    let e1 = CheckError::InvalidMagic {
        path: PathBuf::from("/a"),
        magic: [0xCA, 0xFE, 0xBA, 0xBE],
    };
    let e2 = CheckError::InvalidMagic {
        path: PathBuf::from("/a"),
        magic: [0xCA, 0xFE, 0xBA, 0xBE],
    };
    assert_eq!(e1, e2);
}

#[test]
fn check_error_partial_eq_io_never_equal() {
    let e1 = CheckError::Io {
        path: PathBuf::from("/a"),
        source: std::io::Error::new(std::io::ErrorKind::BrokenPipe, "x"),
    };
    let e2 = CheckError::Io {
        path: PathBuf::from("/a"),
        source: std::io::Error::new(std::io::ErrorKind::BrokenPipe, "x"),
    };
    assert_ne!(e1, e2);
}

#[test]
fn cli_error_from_check_error() {
    let err = CliError::Check(CheckError::FileNotFound {
        path: PathBuf::from("/missing"),
    });
    assert!(err.to_string().contains("/missing"));
}

#[test]
fn cli_error_from_gc_error() {
    let err = CliError::Gc(GcError::VersionsDirNotFound {
        path: PathBuf::from("/v"),
    });
    assert!(err.to_string().contains("/v"));
}

#[test]
fn cli_error_from_init_error() {
    let err = CliError::Init(InitError::DirNotFound {
        path: PathBuf::from("/d"),
    });
    assert!(err.to_string().contains("/d"));
}

#[test]
fn cli_error_from_lock_error() {
    let err = CliError::Lock(LockError::NotInitialized {
        path: PathBuf::from("/p"),
    });
    assert!(err.to_string().contains("/p"));
}

#[test]
fn cli_error_from_doctor_error() {
    let err = CliError::Doctor(DoctorError::NotInitialized {
        path: PathBuf::from("/p"),
    });
    assert!(err.to_string().contains("/p"));
}

#[test]
fn cli_struct_clone_preserves_command() {
    let cli = vo_cli::Cli {
        command: Command::Check {
            workflow: false,
            path: PathBuf::from("/tmp"),
        },
    };
    let cloned = cli.clone();
    assert_eq!(cli, cloned);
}

#[test]
fn cli_struct_equality() {
    let c1 = vo_cli::Cli {
        command: Command::Purge {
            instance: "i1".into(),
        },
    };
    let c2 = vo_cli::Cli {
        command: Command::Purge {
            instance: "i1".into(),
        },
    };
    assert_eq!(c1, c2);
}

#[test]
fn dispatcher_with_middleware_chain() {
    let registry = vo_cli::HandlerRegistry::default();
    let dispatcher = CommandDispatcher::new(registry)
        .with_middleware(LoggingMiddleware::new())
        .with_middleware(MetricsMiddleware::new())
        .with_middleware(LoggingMiddleware::new());
    assert_eq!(dispatcher.middleware_count(), 3);
}

#[tokio::test]
async fn dispatcher_dispatch_unknown_command_returns_error() {
    let registry = vo_cli::HandlerRegistry::new();
    let dispatcher = CommandDispatcher::new(registry);
    let cli = vo_cli::Cli {
        command: Command::Check {
            workflow: false,
            path: PathBuf::from("/tmp"),
        },
    };
    let result = dispatcher.dispatch(cli).await;
    assert!(result.is_err());
}

#[test]
fn rebuild_error_io_display() {
    let err = vo_cli::commands::rebuild::RebuildError::Io {
        path: PathBuf::from("/proj"),
        reason: "reading events".into(),
        source: std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe broke"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/proj"));
    assert!(msg.contains("reading events"));
}

#[test]
fn rebuild_config_clone() {
    use vo_cli::commands::rebuild::RebuildConfig;
    let config = RebuildConfig {
        project_dir: PathBuf::from("/proj"),
        projection_id: Some("p1".into()),
        list_projections: false,
        force: true,
        schema_version: Some(3),
    };
    let cloned = config.clone();
    assert_eq!(config, cloned);
}

#[test]
fn rebuild_report_debug_format() {
    use vo_cli::commands::rebuild::{RebuildReport, RebuildStatus};
    let report = RebuildReport {
        projection_id: Some("p".into()),
        rebuild_id: Some("r".into()),
        status: RebuildStatus::Completed,
        events_applied: 100,
        duration_ms: 50,
    };
    let debug = format!("{report:?}");
    assert!(debug.contains("Completed"));
    assert!(debug.contains("100"));
}

#[test]
fn doctor_config_equality() {
    let c1 = DoctorConfig {
        project_dir: PathBuf::from("/proj"),
    };
    let c2 = DoctorConfig {
        project_dir: PathBuf::from("/proj"),
    };
    assert_eq!(c1, c2);
}

#[test]
fn doctor_config_clone() {
    let c = DoctorConfig {
        project_dir: PathBuf::from("/proj"),
    };
    let c2 = c.clone();
    assert_eq!(c, c2);
}

#[test]
fn rebuild_config_equality() {
    use vo_cli::commands::rebuild::RebuildConfig;
    let c1 = RebuildConfig {
        project_dir: PathBuf::from("/a"),
        projection_id: Some("p".into()),
        list_projections: false,
        force: false,
        schema_version: None,
    };
    let c2 = RebuildConfig {
        project_dir: PathBuf::from("/a"),
        projection_id: Some("p".into()),
        list_projections: false,
        force: false,
        schema_version: None,
    };
    assert_eq!(c1, c2);
}
