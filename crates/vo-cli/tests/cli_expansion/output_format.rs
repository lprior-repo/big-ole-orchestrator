use std::path::PathBuf;
use vo_cli::commands::check::BinaryFormat;
use vo_cli::commands::doctor_checks::{
    format_report, format_report_json, CategoryReport, CheckCategory, CheckResult, DoctorReport,
    Severity,
};

#[test]
fn binary_format_display_name_elf() {
    assert_eq!(BinaryFormat::Elf.display_name(), "valid ELF binary");
}

#[test]
fn binary_format_display_name_macho_32() {
    assert_eq!(
        BinaryFormat::MachO32BigEndian.display_name(),
        "valid Mach-O 32-bit binary"
    );
    assert_eq!(
        BinaryFormat::MachO32LittleEndian.display_name(),
        "valid Mach-O 32-bit binary"
    );
}

#[test]
fn binary_format_display_name_macho_64() {
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
fn doctor_report_format_contains_header() {
    let report = DoctorReport {
        project_dir: PathBuf::from("/test/project"),
        categories: vec![CategoryReport::new(CheckCategory::Workspace)],
    };
    let (stdout, _stderr) = format_report(&report);
    assert!(stdout.contains("Doctor Report"));
    assert!(stdout.contains("/test/project"));
}

#[test]
fn doctor_report_format_contains_category_names() {
    let report = DoctorReport {
        project_dir: PathBuf::from("/tmp"),
        categories: vec![
            CategoryReport::new(CheckCategory::Workspace),
            CategoryReport::new(CheckCategory::LockState),
            CategoryReport::new(CheckCategory::SubprocessLiveness),
            CategoryReport::new(CheckCategory::StorageIntegrity),
            CategoryReport::new(CheckCategory::ConfigValidation),
        ],
    };
    let (stdout, _stderr) = format_report(&report);
    assert!(stdout.contains("[workspace]"));
    assert!(stdout.contains("[lock-state]"));
    assert!(stdout.contains("[subprocess-liveness]"));
    assert!(stdout.contains("[storage-integrity]"));
    assert!(stdout.contains("[config-validation]"));
}

fn make_cat(
    category: CheckCategory,
    checks: Vec<(&'static str, Severity, String)>,
) -> CategoryReport {
    CategoryReport {
        category,
        checks: checks
            .into_iter()
            .map(|(check, severity, message)| CheckResult {
                check,
                severity,
                message,
            })
            .collect(),
    }
}

#[test]
fn doctor_report_format_info_uses_checkmark() {
    let cat = make_cat(
        CheckCategory::Workspace,
        vec![("test-check", Severity::Info, "all good".into())],
    );
    let report = DoctorReport {
        project_dir: PathBuf::from("/tmp"),
        categories: vec![cat],
    };
    let (stdout, _stderr) = format_report(&report);
    assert!(stdout.contains("\u{2713}"));
    assert!(stdout.contains("test-check"));
    assert!(stdout.contains("all good"));
}

#[test]
fn doctor_report_format_error_uses_cross() {
    let cat = make_cat(
        CheckCategory::Workspace,
        vec![("bad-check", Severity::Error, "something broke".into())],
    );
    let report = DoctorReport {
        project_dir: PathBuf::from("/tmp"),
        categories: vec![cat],
    };
    let (_stdout, stderr) = format_report(&report);
    assert!(stderr.contains("\u{2717}"));
    assert!(stderr.contains("bad-check"));
    assert!(stderr.contains("something broke"));
}

#[test]
fn doctor_report_format_warn_uses_warning_icon() {
    let cat = make_cat(
        CheckCategory::Workspace,
        vec![("warn-check", Severity::Warn, "be careful".into())],
    );
    let report = DoctorReport {
        project_dir: PathBuf::from("/tmp"),
        categories: vec![cat],
    };
    let (_stdout, stderr) = format_report(&report);
    assert!(stderr.contains("\u{26A0}"));
    assert!(stderr.contains("warn-check"));
}

#[test]
fn doctor_report_format_healthy_summary() {
    let report = DoctorReport {
        project_dir: PathBuf::from("/tmp"),
        categories: vec![CategoryReport::new(CheckCategory::Workspace)],
    };
    let (stdout, _stderr) = format_report(&report);
    assert!(stdout.contains("All checks passed"));
}

#[test]
fn doctor_report_format_unhealthy_shows_error_count() {
    let cat = make_cat(
        CheckCategory::Workspace,
        vec![
            ("e1", Severity::Error, "err1".into()),
            ("e2", Severity::Error, "err2".into()),
        ],
    );
    let report = DoctorReport {
        project_dir: PathBuf::from("/tmp"),
        categories: vec![cat],
    };
    let (_stdout, stderr) = format_report(&report);
    assert!(stderr.contains("2 error(s)"));
}

#[test]
fn doctor_report_json_structure() {
    let cat = make_cat(
        CheckCategory::Workspace,
        vec![("check1", Severity::Info, "ok".into())],
    );
    let report = DoctorReport {
        project_dir: PathBuf::from("/tmp/json-test"),
        categories: vec![cat],
    };
    let json_str = format_report_json(&report);
    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

    assert_eq!(parsed["project_dir"].as_str().unwrap(), "/tmp/json-test");
    assert!(parsed["healthy"].as_bool().unwrap());
    assert_eq!(parsed["error_count"].as_u64().unwrap(), 0);
    assert_eq!(parsed["warn_count"].as_u64().unwrap(), 0);

    let categories = parsed["categories"].as_array().unwrap();
    assert_eq!(categories.len(), 1);
    assert_eq!(categories[0]["category"].as_str().unwrap(), "workspace");
    assert!(categories[0]["healthy"].as_bool().unwrap());

    let checks = categories[0]["checks"].as_array().unwrap();
    assert_eq!(checks.len(), 1);
    assert_eq!(checks[0]["check"].as_str().unwrap(), "check1");
    assert_eq!(checks[0]["severity"].as_str().unwrap(), "info");
    assert_eq!(checks[0]["message"].as_str().unwrap(), "ok");
}

#[test]
fn doctor_report_json_severity_mapping() {
    let cat = make_cat(
        CheckCategory::Workspace,
        vec![
            ("i", Severity::Info, "info msg".into()),
            ("w", Severity::Warn, "warn msg".into()),
            ("e", Severity::Error, "error msg".into()),
        ],
    );
    let report = DoctorReport {
        project_dir: PathBuf::from("/tmp"),
        categories: vec![cat],
    };
    let json_str = format_report_json(&report);
    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

    let checks = parsed["categories"][0]["checks"].as_array().unwrap();
    assert_eq!(checks[0]["severity"].as_str().unwrap(), "info");
    assert_eq!(checks[1]["severity"].as_str().unwrap(), "warn");
    assert_eq!(checks[2]["severity"].as_str().unwrap(), "error");
}

#[test]
fn check_category_display_format() {
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
fn severity_ordering_invariant() {
    assert!(Severity::Info < Severity::Warn);
    assert!(Severity::Warn < Severity::Error);
    assert!(Severity::Info < Severity::Error);
}

#[test]
fn category_report_is_healthy_only_info_and_warn() {
    let cat = make_cat(
        CheckCategory::Workspace,
        vec![
            ("i", Severity::Info, "ok".into()),
            ("w", Severity::Warn, "careful".into()),
        ],
    );
    assert!(cat.is_healthy());
}

#[test]
fn category_report_not_healthy_with_error() {
    let cat = make_cat(
        CheckCategory::Workspace,
        vec![("e", Severity::Error, "bad".into())],
    );
    assert!(!cat.is_healthy());
}

#[test]
fn category_report_warnings_iterator() {
    let cat = make_cat(
        CheckCategory::Workspace,
        vec![
            ("i", Severity::Info, "ok".into()),
            ("w1", Severity::Warn, "warn1".into()),
            ("w2", Severity::Warn, "warn2".into()),
        ],
    );
    let warns: Vec<_> = cat.warnings().collect();
    assert_eq!(warns.len(), 2);
}
