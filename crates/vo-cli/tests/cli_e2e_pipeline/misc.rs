use vo_cli::commands::check::BinaryFormat;
use vo_cli::commands::doctor::{CategoryReport, CheckCategory, DoctorReport, Severity};
use vo_cli::commands::gc::GcConfig;
use vo_cli::commands::init::InitConfig;
use vo_cli::{parse_strict_numeric, Command};

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
fn command_equality() {
    let c1 = Command::Check {
        workflow: false,
        path: std::path::PathBuf::from("/tmp"),
    };
    let c2 = Command::Check {
        workflow: false,
        path: std::path::PathBuf::from("/tmp"),
    };
    let c3 = Command::Check {
        workflow: false,
        path: std::path::PathBuf::from("/other"),
    };
    assert_eq!(c1, c2);
    assert_ne!(c1, c3);
}

#[test]
fn command_clone() {
    let c = Command::Purge {
        instance: "test".to_string(),
    };
    let cloned = c.clone();
    assert_eq!(c, cloned);
}

#[test]
fn severity_ord_total() {
    assert!(Severity::Error > Severity::Warn);
    assert!(Severity::Warn > Severity::Info);
    assert!(Severity::Error > Severity::Info);
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

#[test]
fn parse_strict_numeric_valid_numbers() {
    assert_eq!(parse_strict_numeric("0").unwrap(), 0);
    assert_eq!(parse_strict_numeric("1").unwrap(), 1);
    assert_eq!(parse_strict_numeric("999999").unwrap(), 999999);
    assert_eq!(
        parse_strict_numeric("18446744073709551615").unwrap(),
        u64::MAX
    );
}

#[test]
fn parse_strict_numeric_negative() {
    let result = parse_strict_numeric("-1");
    assert!(matches!(result, Err(vo_cli::CliError::InvalidNumeric(msg)) if msg.contains("negative")));
}

#[test]
fn parse_strict_numeric_leading_plus() {
    let result = parse_strict_numeric("+42");
    assert!(matches!(result, Err(vo_cli::CliError::InvalidNumeric(msg)) if msg.contains("plus")));
}

#[test]
fn parse_strict_numeric_empty() {
    let result = parse_strict_numeric("");
    assert!(matches!(result, Err(vo_cli::CliError::InvalidNumeric(msg)) if msg.contains("empty")));
}

#[test]
fn parse_strict_numeric_letters() {
    let result = parse_strict_numeric("abc");
    assert!(matches!(result, Err(vo_cli::CliError::InvalidNumeric(msg)) if msg.contains("invalid")));
}

#[test]
fn parse_strict_numeric_overflow() {
    let result = parse_strict_numeric("18446744073709551616");
    assert!(matches!(result, Err(vo_cli::CliError::InvalidNumeric(msg)) if msg.contains("overflow")));
}

#[test]
fn init_config_default() {
    let config = InitConfig::default();
    assert_eq!(config.project_dir, std::path::PathBuf::from("."));
    assert_eq!(config.engine_url, "http://localhost:3000");
    assert_eq!(config.storage_path, std::path::PathBuf::from(".vo/storage"));
}

#[test]
fn gc_config_default() {
    let config = GcConfig::default();
    assert_eq!(config.engine_url, "http://localhost:3000");
    assert_eq!(config.versions_dir, std::path::PathBuf::from("/var/wtf/versions"));
    assert!(!config.dry_run);
}

#[test]
fn category_report_is_healthy_with_only_warnings() {
    let mut r = CategoryReport::new(CheckCategory::Workspace);
    r.push("test", Severity::Warn, "warning".into());
    assert!(r.is_healthy());
}

#[test]
fn category_report_warnings_iterator() {
    let mut r = CategoryReport::new(CheckCategory::Workspace);
    r.push("a", Severity::Info, "info".into());
    r.push("b", Severity::Warn, "warn1".into());
    r.push("c", Severity::Warn, "warn2".into());
    r.push("d", Severity::Error, "err".into());
    let warnings: Vec<_> = r.warnings().collect();
    assert_eq!(warnings.len(), 2);
}

#[test]
fn doctor_report_errors_and_warnings() {
    let mut cat1 = CategoryReport::new(CheckCategory::Workspace);
    cat1.push("e1", Severity::Error, "err".into());
    cat1.push("w1", Severity::Warn, "warn".into());

    let mut cat2 = CategoryReport::new(CheckCategory::LockState);
    cat2.push("e2", Severity::Error, "err2".into());

    let report = DoctorReport {
        project_dir: std::path::PathBuf::from("/tmp"),
        categories: vec![cat1, cat2],
    };
    assert!(!report.is_healthy());
    assert_eq!(report.errors().count(), 2);
    assert_eq!(report.warnings().count(), 1);
}

#[test]
fn doctor_report_healthy_no_errors_no_warnings() {
    let mut cat = CategoryReport::new(CheckCategory::Workspace);
    cat.push("ok", Severity::Info, "fine".into());
    let report = DoctorReport {
        project_dir: std::path::PathBuf::from("/tmp"),
        categories: vec![cat],
    };
    assert!(report.is_healthy());
    assert_eq!(report.errors().count(), 0);
    assert_eq!(report.warnings().count(), 0);
}
