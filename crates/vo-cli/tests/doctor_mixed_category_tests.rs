use std::path::PathBuf;
use vo_cli::commands::doctor_checks::{
    format_report, format_report_json, CategoryReport, CheckCategory, CheckResult, DoctorReport,
    Severity,
};

#[test]
fn doctor_report_mixed_severity_across_categories() {
    let mut cat1 = CategoryReport::new(CheckCategory::Workspace);
    cat1.checks.push(CheckResult {
        check: "ok",
        severity: Severity::Info,
        message: "fine".into(),
    });
    let mut cat2 = CategoryReport::new(CheckCategory::LockState);
    cat2.checks.push(CheckResult {
        check: "bad",
        severity: Severity::Error,
        message: "broken".into(),
    });
    let mut cat3 = CategoryReport::new(CheckCategory::StorageIntegrity);
    cat3.checks.push(CheckResult {
        check: "warn",
        severity: Severity::Warn,
        message: "careful".into(),
    });

    let report = DoctorReport {
        project_dir: PathBuf::from("/proj"),
        categories: vec![cat1, cat2, cat3],
    };

    assert!(!report.is_healthy());
    let errors: Vec<_> = report.errors().collect();
    let warnings: Vec<_> = report.warnings().collect();
    assert_eq!(errors.len(), 1);
    assert_eq!(warnings.len(), 1);

    let (stdout, stderr) = format_report(&report);
    assert!(stdout.contains("Doctor Report"));
    assert!(stderr.contains("1 error(s)"));
    assert!(stderr.contains("1 warning(s)"));
}

#[test]
fn doctor_report_json_mixed_categories() {
    let mut cat1 = CategoryReport::new(CheckCategory::Workspace);
    cat1.checks.push(CheckResult {
        check: "e1",
        severity: Severity::Error,
        message: "err".into(),
    });
    let cat2 = CategoryReport::new(CheckCategory::ConfigValidation);

    let report = DoctorReport {
        project_dir: PathBuf::from("/proj"),
        categories: vec![cat1, cat2],
    };

    let json_str = format_report_json(&report);
    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert!(!parsed["healthy"].as_bool().unwrap());
    assert_eq!(parsed["error_count"].as_u64().unwrap(), 1);
    assert_eq!(parsed["categories"].as_array().unwrap().len(), 2);
    assert!(!parsed["categories"][0]["healthy"].as_bool().unwrap());
    assert!(parsed["categories"][1]["healthy"].as_bool().unwrap());
}
