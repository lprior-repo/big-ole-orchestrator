use crate::data::FleetEntry;
use std::collections::HashSet;
use std::process::Stdio;
use tokio::process::Command;
use tracing::info;

const RIG_PREFIXES: &[&str] = &["ve-", "hl-", "oy-", "se-", "cd-", "cl-"];

/// Kill orphaned tmux sessions matching rig prefixes but not in the fleet.
pub async fn cleanup_orphan_sessions(all_fleet: &[FleetEntry]) -> u32 {
    let all_sessions = enumerate_rig_sessions().await;
    if all_sessions.is_empty() {
        return 0;
    }

    let valid_sessions: HashSet<String> = all_fleet
        .iter()
        .map(|entry| entry.name.tmux_session(entry.rig))
        .collect();

    let orphans: Vec<&String> = all_sessions
        .iter()
        .filter(|session| !valid_sessions.contains(*session))
        .collect();

    let mut killed = 0u32;
    for orphan in orphans {
        let result = Command::new("tmux")
            .args(["kill-session", "-t", orphan])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;

        if matches!(result, Ok(status) if status.success()) {
            info!("killed orphan session: {}", orphan);
            killed += 1;
        }
    }

    killed
}

/// Remove stale agent.lock files across all polecat worktrees.
pub async fn cleanup_stale_locks(gt_root: &str) -> u32 {
    let polecats_dir = std::path::PathBuf::from(format!("{gt_root}/polecats"));
    let mut removed = 0u32;

    let Ok(mut rig_entries) = tokio::fs::read_dir(&polecats_dir).await else {
        return 0;
    };

    while let Ok(Some(polecat_entry)) = rig_entries.next_entry().await {
        let polecat_dir = polecat_entry.path();
        let Ok(mut worktree_entries) = tokio::fs::read_dir(&polecat_dir).await else {
            continue;
        };
        while let Ok(Some(wt_entry)) = worktree_entries.next_entry().await {
            let lock = wt_entry.path().join(".runtime").join("agent.lock");
            if lock.exists() && tokio::fs::remove_file(&lock).await.is_ok() {
                removed += 1;
            }
        }
    }

    removed
}

async fn enumerate_rig_sessions() -> HashSet<String> {
    let Ok(output) = Command::new("tmux")
        .args(["list-sessions", "-F", "#{session_name}"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
    else {
        return HashSet::new();
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .map(str::trim)
        .filter(|name| RIG_PREFIXES.iter().any(|p| name.starts_with(p)))
        .map(String::from)
        .collect()
}
