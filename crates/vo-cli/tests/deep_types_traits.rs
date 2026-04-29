#![allow(clippy::redundant_pattern_matching)]
use std::path::PathBuf;
use vo_cli::{BinaryFormat, CategoryReport, CheckCategory, CheckResult, DoctorReport, Severity};

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
fn check_category_display() {
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
fn severity_ordering_chain() {
    assert!(Severity::Error > Severity::Warn);
    assert!(Severity::Warn > Severity::Info);
    assert!(Severity::Error > Severity::Info);
}

#[test]
fn category_report_warnings_filter() {
    let r = CategoryReport {
        category: CheckCategory::Workspace,
        checks: vec![
            CheckResult {
                check: "a",
                severity: Severity::Info,
                message: "info msg".into(),
            },
            CheckResult {
                check: "b",
                severity: Severity::Warn,
                message: "warn msg".into(),
            },
            CheckResult {
                check: "c",
                severity: Severity::Error,
                message: "err msg".into(),
            },
        ],
    };
    let warnings: Vec<_> = r.warnings().collect();
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].check, "b");
}

#[test]
fn doctor_report_errors_iterator() {
    let report = DoctorReport {
        project_dir: PathBuf::from("/test"),
        categories: vec![
            CategoryReport {
                category: CheckCategory::Workspace,
                checks: vec![
                    CheckResult {
                        check: "e1",
                        severity: Severity::Error,
                        message: "err".into(),
                    },
                    CheckResult {
                        check: "i1",
                        severity: Severity::Info,
                        message: "info".into(),
                    },
                ],
            },
            CategoryReport {
                category: CheckCategory::LockState,
                checks: vec![CheckResult {
                    check: "e2",
                    severity: Severity::Error,
                    message: "err2".into(),
                }],
            },
        ],
    };
    let errors: Vec<_> = report.errors().collect();
    assert_eq!(errors.len(), 2);
}

#[test]
fn doctor_report_warnings_cross_category() {
    let report = DoctorReport {
        project_dir: PathBuf::from("/test"),
        categories: vec![
            CategoryReport {
                category: CheckCategory::Workspace,
                checks: vec![CheckResult {
                    check: "w1",
                    severity: Severity::Warn,
                    message: "w".into(),
                }],
            },
            CategoryReport {
                category: CheckCategory::StorageIntegrity,
                checks: vec![CheckResult {
                    check: "w2",
                    severity: Severity::Warn,
                    message: "w".into(),
                }],
            },
        ],
    };
    let warnings: Vec<_> = report.warnings().collect();
    assert_eq!(warnings.len(), 2);
}
