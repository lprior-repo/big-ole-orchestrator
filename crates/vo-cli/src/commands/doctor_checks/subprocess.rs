use std::path::Path;

use super::{CategoryReport, CheckCategory, Severity};

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
