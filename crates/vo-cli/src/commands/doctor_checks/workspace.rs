use std::path::Path;

use super::{CategoryReport, CheckCategory, Severity};
use crate::utils::file_hash;

fn pid_alive(pid: u32) -> Option<bool> {
    unsafe {
        let ret = libc::kill(pid as libc::pid_t, 0);
        if ret == 0 {
            Some(true)
        } else {
            let errno = *libc::__errno_location();
            match errno {
                libc::ESRCH => Some(false),
                libc::EPERM => Some(true),
                _ => None,
            }
        }
    }
}

pub fn check_workspace(project_dir: &Path, vo_dir: &Path) -> CategoryReport {
    let mut report = CategoryReport::new(CheckCategory::Workspace);

    if !vo_dir.is_dir() {
        report.push(
            "vo-dir",
            Severity::Error,
            format!(".vo/ directory missing in {}", project_dir.display()),
        );
        return report;
    }
    report.push("vo-dir", Severity::Info, ".vo/ directory exists".into());

    let wf_dir = vo_dir.join("workflows");
    if wf_dir.is_dir() {
        let bins: Vec<_> = std::fs::read_dir(&wf_dir)
            .ok()
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
                    .collect()
            })
            .unwrap_or_default();
        report.push(
            "workflows-dir",
            Severity::Info,
            format!("workflows directory exists ({} binaries)", bins.len()),
        );
    } else {
        report.push(
            "workflows-dir",
            Severity::Warn,
            "workflows directory missing".into(),
        );
    }

    let storage_dir = vo_dir.join("storage");
    if storage_dir.is_dir() {
        report.push(
            "storage-dir",
            Severity::Info,
            "storage directory exists".into(),
        );
    } else {
        report.push(
            "storage-dir",
            Severity::Warn,
            "storage directory missing".into(),
        );
    }

    match std::fs::metadata(vo_dir) {
        Ok(meta) => {
            if meta.permissions().readonly() {
                report.push(
                    "vo-dir-perms",
                    Severity::Error,
                    ".vo/ directory is read-only".into(),
                );
            } else {
                report.push(
                    "vo-dir-perms",
                    Severity::Info,
                    ".vo/ directory is writable".into(),
                );
            }
        }
        Err(e) => {
            report.push(
                "vo-dir-perms",
                Severity::Error,
                format!("cannot read .vo/ metadata: {e}"),
            );
        }
    }

    let runtime_dir = vo_dir.join("runtime");
    if runtime_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&runtime_dir) {
            let pid_files: Vec<_> = entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.file_name()
                        .to_str()
                        .map(|n| n.ends_with(".pid"))
                        .unwrap_or(false)
                })
                .collect();
            if !pid_files.is_empty() {
                let mut stale = 0u32;
                for pf in &pid_files {
                    if let Ok(content) = std::fs::read_to_string(pf.path()) {
                        if let Ok(pid) = content.trim().parse::<u32>() {
                            if pid_alive(pid) == Some(false) {
                                stale += 1;
                            }
                        }
                    }
                }
                if stale > 0 {
                    report.push(
                        "stale-pid-files",
                        Severity::Warn,
                        format!("{stale} stale PID file(s) in .vo/runtime/"),
                    );
                } else {
                    report.push(
                        "stale-pid-files",
                        Severity::Info,
                        format!("{} PID file(s), all alive", pid_files.len()),
                    );
                }
            }
        }
    }

    report
}

#[cfg(test)]
mod tests {
    use super::check_workspace;
    use crate::commands::doctor_checks::{CategoryReport, CheckCategory, Severity};
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
    fn workspace_errors_on_missing_vo_dir() {
        let dir = make_project_dir();
        let r = check_workspace(&dir, &dir.join(".vo"));
        assert!(!r.is_healthy());
        assert!(r
            .checks
            .iter()
            .any(|c| c.check == "vo-dir" && c.severity == Severity::Error));
    }

    #[test]
    fn workspace_ok_when_valid() {
        let dir = make_project_dir();
        init_project(&dir);
        let r = check_workspace(&dir, &dir.join(".vo"));
        assert!(r.is_healthy());
    }

    #[test]
    fn workspace_warns_on_missing_workflows() {
        let dir = make_project_dir();
        fs::create_dir_all(dir.join(".vo")).unwrap();
        let r = check_workspace(&dir, &dir.join(".vo"));
        assert!(r
            .checks
            .iter()
            .any(|c| c.check == "workflows-dir" && c.severity == Severity::Warn));
    }
}
