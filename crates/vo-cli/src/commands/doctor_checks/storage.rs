use std::path::Path;

use super::{CategoryReport, CheckCategory, Severity};

pub fn check_storage_integrity(vo_dir: &Path, project_dir: &Path) -> CategoryReport {
    let mut report = CategoryReport::new(CheckCategory::StorageIntegrity);

    let storage_dir = vo_dir.join("storage");
    if !storage_dir.is_dir() {
        report.push(
            "storage-dir",
            Severity::Warn,
            "storage directory does not exist".into(),
        );
        return report;
    }
    report.push(
        "storage-dir",
        Severity::Info,
        "storage directory exists".into(),
    );

    let probe_file = storage_dir.join(".doctor-probe");
    match std::fs::write(&probe_file, b"probe") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe_file);
            report.push(
                "storage-rw",
                Severity::Info,
                "storage directory is read/writable".into(),
            );
        }
        Err(e) => {
            report.push(
                "storage-rw",
                Severity::Error,
                format!("cannot write to storage directory: {e}"),
            );
        }
    }

    match std::fs::read_dir(&storage_dir) {
        Ok(entries) => {
            let items: Vec<_> = entries.filter_map(|e| e.ok()).collect();
            if items.is_empty() {
                report.push(
                    "storage-contents",
                    Severity::Info,
                    "storage directory is empty (new project)".into(),
                );
            } else {
                let partition_names = [
                    "events",
                    "instances",
                    "timers",
                    "leases",
                    "blobs",
                    "signals",
                ];
                let found: Vec<String> = items
                    .iter()
                    .filter_map(|e| {
                        let n = e.file_name().to_string_lossy().to_string();
                        if partition_names.contains(&n.as_str()) {
                            Some(n)
                        } else {
                            None
                        }
                    })
                    .collect();
                if !found.is_empty() {
                    report.push(
                        "storage-partitions",
                        Severity::Info,
                        format!("found {} partition(s): {}", found.len(), found.join(", ")),
                    );
                }
                report.push(
                    "storage-contents",
                    Severity::Info,
                    format!("{} item(s) in storage", items.len()),
                );
            }
        }
        Err(e) => {
            report.push(
                "storage-contents",
                Severity::Error,
                format!("cannot read storage directory: {e}"),
            );
        }
    }

    let wal_patterns = [".wal", ".journal", "-wal", "-journal"];
    if let Ok(entries) = std::fs::read_dir(&storage_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().to_string();
            if wal_patterns.iter().any(|p| name.ends_with(p)) {
                report.push(
                    "storage-wal",
                    Severity::Warn,
                    format!("WAL/journal file found: {name} (possible unclean shutdown)"),
                );
            }
        }
    }

    let config_path = project_dir.join("config.toml");
    if config_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            if let Some(line) = content.lines().find(|l| l.starts_with("path")) {
                if let Some(path_str) = line.split('"').nth(1) {
                    let referenced = project_dir.join(path_str);
                    if !referenced.is_dir() {
                        report.push(
                            "storage-path-ref",
                            Severity::Warn,
                            format!("config.toml references non-existent storage path: {path_str}"),
                        );
                    } else {
                        report.push(
                            "storage-path-ref",
                            Severity::Info,
                            "config.toml storage path is valid".into(),
                        );
                    }
                }
            }
        }
    }

    report
}

#[cfg(test)]
mod tests {
    use super::check_storage_integrity;
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
    fn storage_warns_when_missing() {
        let dir = make_project_dir();
        fs::create_dir_all(dir.join(".vo")).unwrap();
        let r = check_storage_integrity(&dir.join(".vo"), &dir);
        assert!(r
            .checks
            .iter()
            .any(|c| c.check == "storage-dir" && c.severity == Severity::Warn));
    }

    #[test]
    fn storage_ok_when_valid() {
        let dir = make_project_dir();
        init_project(&dir);
        let r = check_storage_integrity(&dir.join(".vo"), &dir);
        assert!(r.is_healthy());
    }

    #[test]
    fn storage_detects_wal_files() {
        let dir = make_project_dir();
        init_project(&dir);
        fs::write(dir.join(".vo/storage/events.wal"), b"wal").unwrap();
        let r = check_storage_integrity(&dir.join(".vo"), &dir);
        assert!(r.warnings().any(|c| c.check == "storage-wal"));
    }
}
