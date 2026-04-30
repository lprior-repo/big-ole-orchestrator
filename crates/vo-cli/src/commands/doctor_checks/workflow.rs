use std::path::Path;

use super::{CategoryReport, CheckCategory, Severity};

pub fn check_workflow_definitions(project_dir: &Path, vo_dir: &Path) -> CategoryReport {
    let mut report = CategoryReport::new(CheckCategory::WorkflowValidation);

    let wf_dir = vo_dir.join("workflows");
    if !wf_dir.is_dir() {
        report.push(
            "workflow-dir",
            Severity::Info,
            "workflows directory does not exist — no definitions to validate".into(),
        );
        return report;
    }

    let entries = match std::fs::read_dir(&wf_dir) {
        Ok(e) => e,
        Err(e) => {
            report.push(
                "workflow-dir-read",
                Severity::Error,
                format!("cannot read workflows directory: {e}"),
            );
            return report;
        }
    };

    let json_files: Vec<_> = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_str()
                .map(|n| n.ends_with(".json"))
                .unwrap_or(false)
        })
        .collect();

    if json_files.is_empty() {
        report.push(
            "workflow-definitions",
            Severity::Info,
            "no JSON workflow definition files found".into(),
        );
        return report;
    }

    report.push(
        "workflow-definitions",
        Severity::Info,
        format!(
            "found {} JSON workflow definition file(s)",
            json_files.len()
        ),
    );

    let mut valid_count = 0u32;
    let mut invalid_count = 0u32;

    for entry in &json_files {
        let path = entry.path();
        let contents = match std::fs::read(&path) {
            Ok(c) => c,
            Err(e) => {
                report.push(
                    "workflow-parse",
                    Severity::Error,
                    format!(
                        "{}: failed to read: {e}",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    ),
                );
                invalid_count += 1;
                continue;
            }
        };

        let mut de = serde_json::Deserializer::from_slice(&contents);
        match vo_types::WorkflowDefinition::from_deserializer(&mut de) {
            Ok(def) => {
                valid_count += 1;
                report.push(
                    "workflow-parse",
                    Severity::Info,
                    format!(
                        "{}: valid ({} node(s))",
                        path.file_name().unwrap_or_default().to_string_lossy(),
                        def.nodes.len()
                    ),
                );
            }
            Err(e) => {
                invalid_count += 1;
                report.push(
                    "workflow-parse",
                    Severity::Error,
                    format!(
                        "{}: parse error: {e}",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    ),
                );
            }
        }
    }

    if valid_count > 0 && invalid_count == 0 {
        report.push(
            "workflow-validation",
            Severity::Info,
            format!("all {valid_count} workflow definition(s) are valid"),
        );
    } else if invalid_count > 0 {
        report.push(
            "workflow-validation",
            Severity::Error,
            format!("{invalid_count} invalid workflow definition(s)"),
        );
    }

    report
}
