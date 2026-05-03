use crate::data::{FleetError, PolecatName, Rig};
use std::process::Stdio;
use tokio::process::Command;
use tracing::info;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorktreeStatus {
    Healthy,
    Dirty,
    Missing,
}

/// Check if a worktree is healthy. Read-only, no mutations.
pub async fn verify_worktree(rig: &Rig, name: &PolecatName) -> WorktreeStatus {
    let path = name.worktree_path(rig);

    if !path.exists() || !path.join(".git").exists() {
        return WorktreeStatus::Missing;
    }

    let branch_output = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(&path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await;

    let branch_ok = matches!(branch_output, Ok(output)
        if String::from_utf8_lossy(&output.stdout).trim() == "main");

    if !branch_ok {
        return WorktreeStatus::Dirty;
    }

    let status_output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await;

    if matches!(status_output, Ok(output) if output.stdout.is_empty()) {
        WorktreeStatus::Healthy
    } else {
        WorktreeStatus::Dirty
    }
}

/// Repair a worktree by cloning from the source repo.
pub async fn repair_worktree(rig: &Rig, name: &PolecatName) -> Result<(), FleetError> {
    let path = name.worktree_path(rig);

    if path.exists() {
        tokio::fs::remove_dir_all(&path)
            .await
            .map_err(FleetError::Io)?;
    }

    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(FleetError::Io)?;
    }

    let clone_result = Command::new("git")
        .args(["clone", rig.src_dir, &path.display().to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .await
        .map_err(|e| FleetError::Git(e.to_string()))?;

    if !clone_result.success() {
        return Err(FleetError::Git(format!(
            "git clone failed for worktree {}",
            name.as_str()
        )));
    }

    let metadata_src = format!("{}/.beads/metadata.json", rig.src_dir);
    let metadata_dst = path.join(".beads/metadata.json");
    if let Some(parent) = metadata_dst.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(FleetError::Io)?;
    }
    tokio::fs::copy(&metadata_src, &metadata_dst)
        .await
        .map_err(FleetError::Io)?;

    info!("{}: repaired worktree for {}", rig.name, name.as_str());
    Ok(())
}

/// Remove orphaned worktrees for polecats not in the fleet config.
pub async fn cleanup_orphan_worktrees(
    rig: &Rig,
    active_polecats: &[PolecatName],
) -> u32 {
    let polecats_dir = std::path::PathBuf::from(format!("{}/polecats", rig.gt_root));
    let mut entries = match tokio::fs::read_dir(&polecats_dir).await {
        Ok(rd) => rd,
        Err(_) => return 0,
    };

    let active_names: std::collections::HashSet<&str> =
        active_polecats.iter().map(|p| p.as_str()).collect();

    let mut removed = 0u32;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if active_names.contains(name.as_str()) {
            continue;
        }
        let worktree =
            std::path::PathBuf::from(format!("{}/polecats/{}/{}", rig.gt_root, name, rig.name));
        if worktree.exists() && tokio::fs::remove_dir_all(&worktree).await.is_ok() {
            info!("{}: removed orphan worktree for {}", rig.name, name);
            removed += 1;
        }
    }

    removed
}
