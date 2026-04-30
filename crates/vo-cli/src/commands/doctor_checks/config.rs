use std::path::Path;

use super::{CategoryReport, CheckCategory, Severity};

pub fn check_config_validation(project_dir: &Path) -> CategoryReport {
    let mut report = CategoryReport::new(CheckCategory::ConfigValidation);

    let config_path = project_dir.join("config.toml");
    if !config_path.exists() {
        report.push(
            "config-exists",
            Severity::Error,
            "config.toml missing".into(),
        );
        return report;
    }
    report.push("config-exists", Severity::Info, "config.toml exists".into());

    let content = match std::fs::read_to_string(&config_path) {
        Ok(c) => c,
        Err(e) => {
            report.push(
                "config-readable",
                Severity::Error,
                format!("cannot read config.toml: {e}"),
            );
            return report;
        }
    };

    if content.trim().is_empty() {
        report.push(
            "config-empty",
            Severity::Error,
            "config.toml is empty".into(),
        );
        return report;
    }
    report.push(
        "config-readable",
        Severity::Info,
        "config.toml is readable and non-empty".into(),
    );

    match content.parse::<toml::Table>() {
        Ok(table) => {
            report.push(
                "config-parseable",
                Severity::Info,
                "config.toml parses as valid TOML".into(),
            );
            if let Some(engine) = table.get("engine") {
                if let Some(url) = engine.get("url") {
                    let url_str = url.as_str().unwrap_or("<non-string>");
                    if url_str.is_empty() {
                        report.push(
                            "config-engine-url",
                            Severity::Warn,
                            "engine URL is empty".into(),
                        );
                    } else {
                        report.push(
                            "config-engine-url",
                            Severity::Info,
                            format!("engine URL: {url_str}"),
                        );
                    }
                } else {
                    report.push(
                        "config-engine-url",
                        Severity::Warn,
                        "[engine] section missing 'url' field".into(),
                    );
                }
            } else {
                report.push(
                    "config-engine",
                    Severity::Warn,
                    "missing [engine] section".into(),
                );
            }
            if let Some(storage) = table.get("storage") {
                if let Some(path) = storage.get("path") {
                    let path_str = path.as_str().unwrap_or("<non-string>");
                    if path_str.is_empty() {
                        report.push(
                            "config-storage-path",
                            Severity::Warn,
                            "storage path is empty".into(),
                        );
                    } else {
                        let full_path = project_dir.join(path_str);
                        if full_path.is_dir() {
                            report.push(
                                "config-storage-path",
                                Severity::Info,
                                format!("storage path: {path_str}"),
                            );
                        } else {
                            report.push(
                                "config-storage-path",
                                Severity::Warn,
                                format!("storage path does not exist: {path_str}"),
                            );
                        }
                    }
                } else {
                    report.push(
                        "config-storage-path",
                        Severity::Warn,
                        "[storage] section missing 'path' field".into(),
                    );
                }
            } else {
                report.push(
                    "config-storage",
                    Severity::Warn,
                    "missing [storage] section".into(),
                );
            }
        }
        Err(e) => {
            report.push(
                "config-parseable",
                Severity::Error,
                format!("config.toml is not valid TOML: {e}"),
            );
        }
    }

    match std::fs::metadata(&config_path) {
        Ok(meta) => {
            if meta.permissions().readonly() {
                report.push(
                    "config-perms",
                    Severity::Warn,
                    "config.toml is read-only".into(),
                );
            } else {
                report.push(
                    "config-perms",
                    Severity::Info,
                    "config.toml is writable".into(),
                );
            }
        }
        Err(e) => {
            report.push(
                "config-perms",
                Severity::Warn,
                format!("cannot check config.toml permissions: {e}"),
            );
        }
    }

    report
}

#[cfg(test)]
mod tests {
    use super::check_config_validation;
    use crate::commands::doctor_checks::{CheckCategory, Severity};
    use std::fs;
    use std::path::PathBuf;

    fn make_project_dir() -> PathBuf {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().to_path_buf();
        std::mem::forget(dir);
        p
    }

    fn init_project(project_dir: &Path) {
        let vo_dir = project_dir.join(".vo");
        fs::create_dir_all(vo_dir.join("workflows")).unwrap();
        fs::create_dir_all(vo_dir.join("storage")).unwrap();
        fs::write(
            project_dir.join("config.toml"),
            "[engine]\nurl = \"http://localhost:3000\"\n\n[storage]\npath = \".vo/storage\"\n",
        )
        .unwrap();
    }

    #[test]
    fn config_errors_when_missing() {
        let dir = make_project_dir();
        fs::create_dir_all(dir.join(".vo/workflows")).unwrap();
        let r = check_config_validation(&dir);
        assert!(!r.is_healthy());
        assert!(r.checks.iter().any(|c| c.check == "config-exists"));
    }

    #[test]
    fn config_errors_when_empty() {
        let dir = make_project_dir();
        fs::create_dir_all(dir.join(".vo/workflows")).unwrap();
        fs::write(dir.join("config.toml"), "").unwrap();
        let r = check_config_validation(&dir);
        assert!(!r.is_healthy());
        assert!(r.checks.iter().any(|c| c.check == "config-empty"));
    }

    #[test]
    fn config_errors_on_invalid_toml() {
        let dir = make_project_dir();
        fs::create_dir_all(dir.join(".vo/workflows")).unwrap();
        fs::write(dir.join("config.toml"), "{{{invalid").unwrap();
        let r = check_config_validation(&dir);
        assert!(!r.is_healthy());
        assert!(r.checks.iter().any(|c| c.check == "config-parseable"));
    }

    #[test]
    fn config_ok_when_valid() {
        let dir = make_project_dir();
        init_project(&dir);
        let r = check_config_validation(&dir);
        assert!(r.is_healthy());
    }

    #[test]
    fn config_warns_on_missing_storage_path() {
        let dir = make_project_dir();
        fs::create_dir_all(dir.join(".vo/workflows")).unwrap();
        fs::write(
            dir.join("config.toml"),
            "[engine]\nurl = \"http://localhost:3000\"\n\n[storage]\npath = \".vo/nonexistent\"\n",
        )
        .unwrap();
        let r = check_config_validation(&dir);
        assert!(r.warnings().any(|c| c.check == "config-storage-path"));
    }
}
