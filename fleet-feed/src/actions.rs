use crate::calculations::{
    build_claude_launch_cmd, build_env_vars, build_opencode_launch_cmd, build_pre_launch,
    build_prompt, classify_batch_status, parse_active_parent_pids, parse_tmux_session_pids,
};
use crate::data::{
    BeadCategory, BeadId, BeadJson, FeedOutcome, FeedSummary, Fleet, FleetEntry, FleetError,
    FleetMetrics, ModuleMetrics, PolecatName, PolecatStatus, Rig, RuntimeKind,
};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, info, warn};

const DOLT_COOLDOWN: std::time::Duration = std::time::Duration::from_millis(500);
const DOLT_MUTATION_COOLDOWN: std::time::Duration = std::time::Duration::from_millis(250);
const STALE_RECOVERY_CAP: usize = 5;
const MAX_CONCURRENT_POLECATS: usize = 25;
const PER_RIG_QUOTA: usize = 5;

async fn collect_active_parent_pids(session_pids: &HashMap<String, String>) -> HashSet<String> {
    let tracked_pids: HashSet<String> = session_pids
        .values()
        .filter(|pid| !pid.is_empty())
        .cloned()
        .collect();

    if tracked_pids.is_empty() {
        return HashSet::new();
    }

    let output = match Command::new("ps")
        .args(["-o", "ppid=", "-o", "pid=", "-ax"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
    {
        Ok(output) => output,
        Err(_) => return HashSet::new(),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_active_parent_pids(&stdout)
        .into_iter()
        .filter(|ppid| tracked_pids.contains(ppid))
        .collect()
}

pub async fn ensure_dolt_alive() -> Result<(), FleetError> {
    let check = Command::new("gt")
        .args(["dolt", "status"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(FleetError::Io)?;

    let stdout = String::from_utf8_lossy(&check.stdout);
    if stdout.contains("running") || check.status.success() {
        return Ok(());
    }

    warn!("Dolt not running, attempting restart");
    let restart = Command::new("gt")
        .args(["dolt", "start"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(FleetError::Io)?;

    if restart.status.success() {
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        info!("Dolt restarted successfully");
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&restart.stderr);
        Err(FleetError::Bd(format!(
            "dolt restart failed: {}",
            stderr.trim()
        )))
    }
}

/// Batch-check all polecat statuses for a rig using a single `tmux list-sessions`.
pub async fn batch_check_polecat_status(rig: &Rig, fleet: &[FleetEntry]) -> Vec<PolecatStatus> {
    let output = match Command::new("tmux")
        .args(["list-sessions", "-F", "#{session_name}:#{pane_pid}"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
    {
        Ok(o) => o,
        Err(_) => return vec![PolecatStatus::Dead; fleet.len()],
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let session_pids = parse_tmux_session_pids(&stdout);
    let active_parent_pids = collect_active_parent_pids(&session_pids).await;

    fleet
        .iter()
        .map(|entry| {
            let session = entry.name.tmux_session(rig);
            classify_batch_status(&session, &session_pids, &active_parent_pids)
        })
        .collect()
}

/// Release `in_progress` beads claimed by idle/dead polecats back to ready state.
pub async fn recover_stale_beads(rig: &Rig, stale_names: &[&PolecatName]) -> usize {
    let output = match Command::new("bd")
        .args(["list", "--status", "in_progress", "--json"])
        .current_dir(rig.src_dir)
        .env("BD_DOLT_AUTO_COMMIT", "off")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
    {
        Ok(o) => o,
        Err(e) => {
            warn!("Failed to list in-progress beads: {}", e);
            return 0;
        }
    };

    let stale_roles: HashSet<String> = stale_names
        .iter()
        .map(|n| n.role(rig))
        .collect();

    let stale_beads: Vec<&BeadJson> = String::from_utf8_lossy(&output.stdout)
        .parse::<String>()
        .ok()
        .and_then(|s| serde_json::from_str::<Vec<BeadJson>>(&s).ok())
        .unwrap_or_default()
        .iter()
        .filter(|b| b.assignee.as_ref().is_some_and(|a| stale_roles.contains(a)))
        .collect();

    if stale_beads.is_empty() {
        return 0;
    }

    info!("Found {} stale beads to recover", stale_beads.len());

    let mut recovered = 0usize;

    for bead in stale_beads.iter().take(STALE_RECOVERY_CAP) {
        let result = Command::new("bd")
            .args(["update", &bead.id, "--status", "open", "--assignee", ""])
            .current_dir(rig.src_dir)
            .env("BD_DOLT_AUTO_COMMIT", "off")
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .status()
            .await;

        match result {
            Ok(s) if s.success() => {
                info!("Recovered stale bead {} from idle/dead polecat", bead.id);
                recovered += 1;
                tokio::time::sleep(DOLT_MUTATION_COOLDOWN).await;
            }
            _ => {
                debug!("Could not release bead {}", bead.id);
            }
        }
    }

    recovered
}

/// Fetch 50 ready beads from `bd ready --json`, falling back to plain text parsing.
pub async fn fetch_ready_beads(rig: &Rig) -> Result<Vec<BeadJson>, FleetError> {
    let output = Command::new("bd")
        .args(["ready", "-n", "50", "--json"])
        .current_dir(rig.src_dir)
        .env("BD_DOLT_AUTO_COMMIT", "off")
        .output()
        .await
        .map_err(|e| FleetError::Bd(e.to_string()))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    if let Ok(beads) = serde_json::from_str::<Vec<BeadJson>>(&stdout)
        && !beads.is_empty()
    {
        return Ok(beads);
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}\n{stderr}");
    let prefix = rig.bead_prefix;

    let beads: Vec<BeadJson> = combined
        .lines()
        .filter_map(|line| {
            line.trim()
                .split_once("] ")
                .and_then(|(_, rest)| rest.split_once(": "))
                .and_then(|(before_id, _)| {
                    before_id
                        .rsplit(' ')
                        .next()
                        .filter(|id| id.starts_with(prefix))
                        .map(|id| BeadJson {
                            id: id.to_string(),
                            assignee: None,
                        })
                })
        })
        .collect();

    Ok(beads)
}

pub async fn assign_bead(rig: &Rig, bead: &BeadId, name: &PolecatName) -> Result<(), FleetError> {
    let assignee = name.role(rig);
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(8),
        Command::new("bd")
            .args(["update", bead.as_str(), "--claim", "--assignee", &assignee])
            .current_dir(rig.src_dir)
            .env("BD_DOLT_AUTO_COMMIT", "off")
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output(),
    )
    .await
    .map_err(|_| FleetError::Bd("bd update timed out".into()))?
    .map_err(|e| FleetError::Bd(e.to_string()))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("already claimed") {
        debug!("Bead {} already claimed", bead.as_str());
        Ok(())
    } else {
        Err(FleetError::Bd(format!(
            "bd update failed for {}: {}",
            bead.as_str(),
            stderr.trim()
        )))
    }
}

pub async fn prepare_worktree(rig: &Rig, name: &PolecatName) -> Result<(), FleetError> {
    let clone = name.worktree_path(rig);
    let metadata_dst = clone.join(".beads/metadata.json");

    if let Some(parent) = metadata_dst.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(FleetError::Io)?;
    }

    let metadata_src = format!("{}/.beads/metadata.json", rig.src_dir);
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

    let branch = String::from_utf8_lossy(
        &Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(&clone)
            .output()
            .await
            .map_err(|e| FleetError::Git(e.to_string()))?
            .stdout,
    )
    .trim()
    .to_string();

    let env = build_env_vars(entry, &branch);
    let pre = build_pre_launch(&clone_str);
    let prompt = build_prompt(rig, name, bead);

    let cmd = match entry.runtime.kind {
        RuntimeKind::OpenCode => build_opencode_launch_cmd(entry, bead, &env, &pre, &prompt),
        RuntimeKind::Claude => build_claude_launch_cmd(entry, bead, &env, &pre, &prompt),
    };

    let result = Command::new("tmux")
        .args(["new-session", "-d", "-s", &session, "-c", &clone_str])
        .arg(&cmd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map_err(|e| FleetError::Tmux(e.to_string()))?;

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

async fn feed_polecat_with_status(
    entry: &FleetEntry,
    status: PolecatStatus,
    beads: &[BeadJson],
    claimed: &HashSet<String>,
) -> (FeedOutcome, Option<String>) {
    let name = &entry.name;
    let rig = entry.rig;

    match status {
        PolecatStatus::Working => {
            debug!("{}: working, skipping", name.as_str());
            (FeedOutcome::SkippedWorking, None)
        }
        PolecatStatus::Dead | PolecatStatus::Idle => {
            let Some(bead) = beads
                .iter()
                .find(|b| b.assignee.is_none() && !claimed.contains(&b.id))
                .map(|b| BeadId(b.id.clone()))
            else {
                info!("{}: no unassigned beads available", name.as_str());
                return (FeedOutcome::SkippedNoBeads, None);
            };

            let outcome = assign_bead(rig, &bead, name)
                .await
                .and_then(|_| prepare_worktree(rig, name).await)
                .and_then(|_| launch_session(entry, &bead).await);

            match outcome {
                Ok(()) => (FeedOutcome::Fed, Some(bead.as_str().to_string())),
                Err(e) => {
                    warn!("{}: feed failed: {}", name.as_str(), e);
                    (FeedOutcome::LaunchFailed, None)
                }
            }
        }
    }
}
