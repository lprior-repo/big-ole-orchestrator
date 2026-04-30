use crate::commands::doctor_checks::{CategoryReport, CheckResult, DoctorReport, Severity};

#[allow(clippy::unwrap_used)]
pub fn format_report(report: &DoctorReport) -> (String, String) {
    let mut stdout = String::new();
    let mut stderr = String::new();
    use std::fmt::Write;
    let _ = writeln!(stdout, "Doctor Report: {}", report.project_dir.display());
    let _ = writeln!(stdout);
    for cat in &report.categories {
        let _ = writeln!(stdout, "[{}]", cat.category);
        if cat.checks.is_empty() {
            let _ = writeln!(stdout, "  (no checks)");
            continue;
        }
        for check in &cat.checks {
            let icon = match check.severity {
                Severity::Info => "\u{2713}",
                Severity::Warn => "\u{26A0}",
                Severity::Error => "\u{2717}",
            };
            let line = format!("  {} {}: {}", icon, check.check, check.message);
            match check.severity {
                Severity::Info => {
                    let _ = writeln!(stdout, "{line}");
                }
                Severity::Warn | Severity::Error => {
                    let _ = writeln!(stderr, "{line}");
                }
            }
        }
        let _ = writeln!(stdout);
    }
    let ec = report.errors().count();
    let wc = report.warnings().count();
    if ec == 0 && wc == 0 {
        let _ = writeln!(stdout, "All checks passed. Project is healthy.");
    } else {
        if ec > 0 {
            let _ = writeln!(stderr, "{} error(s) found.", ec);
        }
        if wc > 0 {
            let _ = writeln!(stderr, "{} warning(s) found.", wc);
        }
    }
    (stdout, stderr)
}

pub fn format_report_json(report: &DoctorReport) -> String {
    let categories: Vec<serde_json::Value> = report.categories.iter().map(|cat| {
        let checks: Vec<serde_json::Value> = cat.checks.iter().map(|c| {
            serde_json::json!({
                "check": c.check,
                "severity": match c.severity {
                    Severity::Info => "info",
                    Severity::Warn => "warn",
                    Severity::Error => "error",
                },
                "message": c.message,
            })
        }).collect();
        serde_json::json!({ "category": cat.category.to_string(), "healthy": cat.is_healthy(), "checks": checks })
    }).collect();
    serde_json::json!({
        "project_dir": report.project_dir.to_string_lossy(),
        "healthy": report.is_healthy(),
        "error_count": report.errors().count(),
        "warn_count": report.warnings().count(),
        "categories": categories,
    })
    .to_string()
}
