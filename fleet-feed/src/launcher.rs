use crate::calculations::{
    build_claude_launch_cmd, build_env_vars, build_opencode_launch_cmd, build_pre_launch,
    build_prompt,
};
use crate::data::{BeadId, FleetEntry, FleetError, PolecatName, Rig, RuntimeKind};
use std::process::Stdio;
use tokio::process::Command;
use tracing::info;

pub async fn prepare_worktree(rig: &Rig, name: &PolecatName) -> Result<(), FleetError> {
    let clone = name.worktree_path(rig);
    let metadata_src = format!("{}/.beads/metadata.json", rig.src_dir);
    let metadata_dst = clone.join(".beads/metadata.json");

    if let Some(parent) = metadata_dst.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(FleetError::Io)?;
    }

    tokio::fs::copy(&metadata_src, &metadata_dst)
        .await
        .map_err(FleetError::Io)?;

    let lock_path = clone.join(".runtime/agent.lock");
    if lock_path.exists() {
        tokio::fs::remove_file(&lock_path)
            .await
            .map_err(FleetError::Io)?;
    }

    Ok(())
}

pub async fn launch_session(entry: &FleetEntry, bead: &BeadId) -> Result<(), FleetError> {
    let name = &entry.name;
    let rig = entry.rig;
    let session = name.tmux_session(rig);
    let clone = name.worktree_path(rig);
    let clone_str = clone.display().to_string();

    let _ = Command::new("tmux")
        .args(["kill-session", "-t", &session])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;

    let branch_output = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(&clone)
        .output()
        .await
        .map_err(|error| FleetError::Git(error.to_string()))?;

    let branch = String::from_utf8_lossy(&branch_output.stdout)
        .trim()
        .to_string();

    let env = build_env_vars(entry, &branch);
    let pre = build_pre_launch(&clone_str);
    let prompt = build_prompt(rig, name, bead);
    let cmd = launch_command(entry, bead, &env, &pre, &prompt);

    let result = Command::new("tmux")
        .args(["new-session", "-d", "-s", &session, "-c", &clone_str])
        .arg(&cmd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map_err(|error| FleetError::Tmux(error.to_string()))?;

    if result.success() {
        info!("{}: launched with {}", name.as_str(), bead.as_str());
        Ok(())
    } else {
        Err(FleetError::Tmux(format!(
            "tmux new-session failed for {}",
            name.as_str()
        )))
    }
}

fn launch_command(entry: &FleetEntry, bead: &BeadId, env: &str, pre: &str, prompt: &str) -> String {
    match entry.runtime.kind {
        RuntimeKind::OpenCode => build_opencode_launch_cmd(entry, bead, env, pre, prompt),
        RuntimeKind::Claude => build_claude_launch_cmd(entry, bead, env, pre, prompt),
    }
}
