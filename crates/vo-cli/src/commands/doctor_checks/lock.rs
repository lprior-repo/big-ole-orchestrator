use std::collections::BTreeMap;
use std::path::Path;

use super::{CategoryReport, CheckCategory, Severity};
use crate::utils::file_hash;

fn parse_lockfile(content: &str) -> BTreeMap<String, String> {
    content
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|l| {
            let (name, hash) = l.split_once(' ')?;
            Some((name.to_string(), hash.to_string()))
        })
        .collect()
}

pub fn check_lock_state(project_dir: &Path, vo_dir: &Path) -> CategoryReport {
    let mut report = CategoryReport::new(CheckCategory::LockState);

    let lock_path = project_dir.join("vo.lock");
    if !lock_path.exists() {
        report.push(
            "lockfile",
            Severity::Info,
            "no lockfile (vo.lock) — run `vo lock` to create one".into(),
        );
        return report;
    }

    let lock_content = match std::fs::read_to_string(&lock_path) {
        Ok(c) => c,
        Err(e) => {
            report.push(
                "lockfile",
                Severity::Error,
                format!("cannot read vo.lock: {e}"),
            );
            return report;
        }
    };

    let lockmap = parse_lockfile(&lock_content);
    if lockmap.is_empty() {
        report.push("lockfile", Severity::Warn, "lockfile is empty".into());
        return report;
    }

    report.push(
        "lockfile",
        Severity::Info,
        format!("lockfile contains {} entries", lockmap.len()),
    );

    let wf_dir = vo_dir.join("workflows");
    let mut verified = 0u32;

    for (name, expected_hash) in &lockmap {
        let bin_path = wf_dir.join(name);
        if !bin_path.exists() {
            report.push(
                "lock-integrity",
                Severity::Error,
                format!("{name}: binary missing (referenced in lockfile)"),
            );
            continue;
        }
        match file_hash(&bin_path) {
            Ok(actual) if actual == *expected_hash => {
                verified += 1;
            }
            Ok(actual) => {
                report.push(
                    "lock-integrity",
                    Severity::Error,
                    format!("{name}: hash mismatch (expected {expected_hash}, got {actual})"),
                );
            }
            Err(e) => {
                report.push(
                    "lock-integrity",
                    Severity::Error,
                    format!("{name}: failed to read: {e}"),
                );
            }
        }
    }

    if verified == lockmap.len() as u32 {
        report.push(
            "lock-integrity",
            Severity::Info,
            format!("all {verified} lockfile entries verified"),
        );
    }

    if wf_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&wf_dir) {
            let orphans: Vec<String> = entries
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    if lockmap.contains_key(&name) {
                        None
                    } else {
                        Some(name)
                    }
                })
                .collect();
            if !orphans.is_empty() {
                report.push(
                    "orphan-binaries",
                    Severity::Warn,
                    format!(
                        "{} binary(ies) not in lockfile: {}",
                        orphans.len(),
                        orphans.join(", ")
                    ),
                );
            }
        }
    }

    report
}

#[cfg(test)]
mod tests {
    use super::check_lock_state;
    use crate::commands::doctor_checks::{CheckCategory, Severity};
    use sha2::{Digest, Sha256};
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
    fn lock_state_info_when_no_lockfile() {
        let dir = make_project_dir();
        init_project(&dir);
        let r = check_lock_state(&dir, &dir.join(".vo"));
        assert!(r.is_healthy());
        assert!(r
            .checks
            .iter()
            .any(|c| c.check == "lockfile" && c.severity == Severity::Info));
    }

    #[test]
    fn lock_state_detects_hash_mismatch() {
        let dir = make_project_dir();
        init_project(&dir);
        fs::write(dir.join(".vo/workflows/wf"), b"actual").unwrap();
        let bad_hash = format!("{:x}", Sha256::digest(b"wrong"));
        fs::write(dir.join("vo.lock"), format!("wf {bad_hash}\n")).unwrap();
        let r = check_lock_state(&dir, &dir.join(".vo"));
        assert!(!r.is_healthy());
        assert!(r.checks.iter().any(|c| c.check == "lock-integrity"));
    }

    #[test]
    fn lock_state_detects_missing_binary() {
        let dir = make_project_dir();
        init_project(&dir);
        let hash = format!("{:x}", Sha256::digest(b"x"));
        fs::write(dir.join("vo.lock"), format!("missing {hash}\n")).unwrap();
        let r = check_lock_state(&dir, &dir.join(".vo"));
        assert!(!r.is_healthy());
        assert!(r
            .checks
            .iter()
            .any(|c| c.check == "lock-integrity" && c.message.contains("missing")));
    }

    #[test]
    fn lock_state_verifies_valid_lockfile() {
        let dir = make_project_dir();
        init_project(&dir);
        let content = b"my-wf binary content";
        fs::write(dir.join(".vo/workflows/my-wf"), content).unwrap();
        let hash = format!("{:x}", Sha256::digest(content));
        fs::write(dir.join("vo.lock"), format!("my-wf {hash}\n")).unwrap();
        let r = check_lock_state(&dir, &dir.join(".vo"));
        assert!(r.is_healthy());
    }

    #[test]
    fn lock_state_detects_orphan_binaries() {
        let dir = make_project_dir();
        init_project(&dir);
        fs::write(dir.join(".vo/workflows/orphan"), b"content").unwrap();
        let hash = format!("{:x}", Sha256::digest(b"other"));
        fs::write(dir.join("vo.lock"), format!("locked-wf {hash}\n")).unwrap();
        fs::write(dir.join(".vo/workflows/locked-wf"), b"other").unwrap();
        let r = check_lock_state(&dir, &dir.join(".vo"));
        assert!(r.warnings().any(|c| c.check == "orphan-binaries"));
    }
}
