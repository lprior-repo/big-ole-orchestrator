use super::{CategoryReport, CheckCategory, Severity};

pub fn check_agents(_project_dir: &std::path::Path) -> CategoryReport {
    let report = CategoryReport::new(CheckCategory::Workspace);
    report
}
