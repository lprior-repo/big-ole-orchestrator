use super::{CategoryReport, CheckCategory, Severity};

pub fn check_dolt_health(_project_dir: &std::path::Path) -> CategoryReport {
    let report = CategoryReport::new(CheckCategory::Workspace);
    report
}
