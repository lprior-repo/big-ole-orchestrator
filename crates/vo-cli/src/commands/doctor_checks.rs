use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::utils::file_hash;

/// Category of health check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckCategory {
    Workspace,
    LockState,
    SubprocessLiveness,
    StorageIntegrity,
    ConfigValidation,
}

impl std::fmt::Display for CheckCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Workspace => write!(f, "workspace"),
            Self::LockState => write!(f, "lock-state"),
            Self::SubprocessLiveness => write!(f, "subprocess-liveness"),
            Self::StorageIntegrity => write!(f, "storage-integrity"),
            Self::ConfigValidation => write!(f, "config-validation"),
        }
    }
}

/// Severity of a diagnostic finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warn,
    Error,
}

/// A single diagnostic check result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckResult {
    pub check: &'static str,
    pub severity: Severity,
    pub message: String,
}

/// Results for a single category.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoryReport {
    pub category: CheckCategory,
    pub checks: Vec<CheckResult>,
}

impl CategoryReport {
    pub fn new(category: CheckCategory) -> Self {
        Self {
            category,
            checks: Vec::new(),
        }
    }

    pub fn is_healthy(&self) -> bool {
        self.checks.iter().all(|c| c.severity <= Severity::Warn)
    }

    pub fn warnings(&self) -> impl Iterator<Item = &CheckResult> {
        self.checks.iter().filter(|c| c.severity == Severity::Warn)
    }

    pub fn push(&mut self, check: &'static str, severity: Severity, message: String) {
        self.checks.push(CheckResult {
            check,
            severity,
            message,
        });
    }
}

/// Aggregated comprehensive doctor report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorReport {
    pub project_dir: PathBuf,
    pub categories: Vec<CategoryReport>,
}

impl DoctorReport {
    pub fn is_healthy(&self) -> bool {
        self.categories.iter().all(|c| c.is_healthy())
    }

    pub fn errors(&self) -> impl Iterator<Item = &CheckResult> {
        self.categories
            .iter()
            .flat_map(|c| c.checks.iter())
            .filter(|c| c.severity == Severity::Error)
    }

    pub fn warnings(&self) -> impl Iterator<Item = &CheckResult> {
        self.categories
            .iter()
            .flat_map(|c| c.checks.iter())
            .filter(|c| c.severity == Severity::Warn)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Check: Workspace integrity
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Check: Lock state
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Check: Subprocess liveness
// ---------------------------------------------------------------------------

pub fn check_subprocess_liveness(vo_dir: &Path) -> CategoryReport {
    let mut report = CategoryReport::new(CheckCategory::SubprocessLiveness);

    let runtime_dir = vo_dir.join("runtime");
    if !runtime_dir.is_dir() {
        report.push(
            "subprocess-liveness",
            Severity::Info,
            "no runtime directory — no managed subprocesses".into(),
        );
        return report;
    }

    let entries = match std::fs::read_dir(&runtime_dir) {
        Ok(e) => e,
        Err(e) => {
            report.push(
                "runtime-dir",
                Severity::Warn,
                format!("cannot read runtime directory: {e}"),
            );
            return report;
        }
    };

    let mut alive = 0u32;
    let mut dead = 0u32;

    for entry in entries.filter_map(|e| e.ok()) {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.ends_with(".pid") {
            continue;
        }
        let content = match std::fs::read_to_string(entry.path()) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let pid = match content.trim().parse::<u32>() {
            Ok(p) => p,
            Err(_) => continue,
        };
        match pid_alive(pid) {
            Some(true) => {
                alive += 1;
                let proc_name = name.strip_suffix(".pid").unwrap_or(&name);
                report.push(
                    "process-alive",
                    Severity::Info,
                    format!("{proc_name} (pid {pid}) is running"),
                );
            }
            Some(false) => {
                dead += 1;
                let proc_name = name.strip_suffix(".pid").unwrap_or(&name);
                report.push(
                    "process-dead",
                    Severity::Error,
                    format!("{proc_name} (pid {pid}) is not running (stale PID file)"),
                );
            }
            None => {}
        }
    }

    if alive == 0 && dead == 0 {
        report.push(
            "subprocess-liveness",
            Severity::Info,
            "no PID files found — no managed subprocesses".into(),
        );
    }

    report
}

// ---------------------------------------------------------------------------
// Check: Storage integrity
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Check: Config validation
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Display
// ---------------------------------------------------------------------------

pub fn format_report(report: &DoctorReport) -> (String, String) {
    let mut stdout = String::new();
    let mut stderr = String::new();
    use std::fmt::Write;
    writeln!(stdout, "Doctor Report: {}", report.project_dir.display()).unwrap();
    writeln!(stdout).unwrap();
    for cat in &report.categories {
        writeln!(stdout, "[{}]", cat.category).unwrap();
        if cat.checks.is_empty() {
            writeln!(stdout, "  (no checks)").unwrap();
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
                Severity::Info => writeln!(stdout, "{line}").unwrap(),
                Severity::Warn | Severity::Error => writeln!(stderr, "{line}").unwrap(),
            }
        }
        writeln!(stdout).unwrap();
    }
    let ec = report.errors().count();
    let wc = report.warnings().count();
    if ec == 0 && wc == 0 {
        writeln!(stdout, "All checks passed. Project is healthy.").unwrap();
    } else {
        if ec > 0 {
            writeln!(stderr, "{} error(s) found.", ec).unwrap();
        }
        if wc > 0 {
            writeln!(stderr, "{} warning(s) found.", wc).unwrap();
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

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

    // --- Type tests ---

    #[test]
    fn check_category_has_five_variants() {
        let all = [
            CheckCategory::Workspace,
            CheckCategory::LockState,
            CheckCategory::SubprocessLiveness,
            CheckCategory::StorageIntegrity,
            CheckCategory::ConfigValidation,
        ];
        assert_eq!(all.len(), 5);
    }

    #[test]
    fn severity_ordering() {
        assert!(Severity::Error > Severity::Warn);
        assert!(Severity::Warn > Severity::Info);
    }

    #[test]
    fn category_report_empty_is_healthy() {
        assert!(CategoryReport::new(CheckCategory::Workspace).is_healthy());
    }

    #[test]
    fn category_report_with_error_is_unhealthy() {
        let mut r = CategoryReport::new(CheckCategory::Workspace);
        r.push("t", Severity::Error, "b".into());
        assert!(!r.is_healthy());
    }

    // --- Workspace ---

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

    // --- Lock state ---

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
    #[test]
    fn lock_state_detects_orphan_binaries() {
        let dir = make_project_dir();
        init_project(&dir);
        fs::write(dir.join(".vo/workflows/orphan"), b"content").unwrap();
        // Need a lockfile so the function does not return early
        let hash = format!("{:x}", Sha256::digest(b"other"));
        fs::write(dir.join("vo.lock"), format!("locked-wf {hash}\n")).unwrap();
        fs::write(dir.join(".vo/workflows/locked-wf"), b"other").unwrap();
        let r = check_lock_state(&dir, &dir.join(".vo"));
        assert!(r.warnings().any(|c| c.check == "orphan-binaries"));
    }

    // --- Storage ---

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

    // --- Config ---

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

    // --- Display ---

    #[test]
    fn format_report_json_is_valid() {
        let report = DoctorReport {
            project_dir: PathBuf::from("/tmp/test"),
            categories: vec![CategoryReport::new(CheckCategory::Workspace)],
        };
        let json = format_report_json(&report);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed["healthy"].as_bool().unwrap());
        assert_eq!(parsed["categories"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn format_report_produces_output() {
        let report = DoctorReport {
            project_dir: PathBuf::from("/tmp/test"),
            categories: vec![CategoryReport::new(CheckCategory::Workspace)],
        };
        let (stdout, _stderr) = format_report(&report);
        assert!(stdout.contains("Doctor Report"));
        assert!(stdout.contains("workspace"));
    }
}
