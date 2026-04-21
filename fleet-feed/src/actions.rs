use crate::calculations::{
    build_claude_launch_cmd, build_env_vars, build_opencode_launch_cmd, build_pre_launch,
    build_prompt, classify_status,
};
use crate::data::{
    BeadId, BeadJson, FeedOutcome, FeedSummary, Fleet, FleetEntry, FleetError, PolecatName,
    PolecatStatus, RuntimeKind, SRC_DIR,
};
use std::collections::HashSet;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, info, warn};

/// Check if dolt server is responsive. If not, attempt restart.
pub async fn ensure_dolt_alive() -> Result<(), FleetError> {
    let check = Command::new("gt")
        .args(["dolt", "status"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| FleetError::Io(e))?;

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
        .map_err(|e| FleetError::Io(e))?;

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

/// Release in_progress beads claimed by dead polecats back to ready state.
pub async fn recover_stale_beads(dead_names: &[&PolecatName]) -> usize {
    let mut recovered = 0usize;

    for name in dead_names {
        let assignee = name.role();
        let output = match Command::new("bd")
            .args(["list", "--status", "in_progress", "--assignee", &assignee, "--plain"])
            .current_dir(SRC_DIR)
            .env("BD_DOLT_AUTO_COMMIT", "off")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
        {
            Ok(o) => o,
            Err(e) => {
                warn!("Failed to list beads for {}: {}", name.as_str(), e);
                continue;
            }
        };

        let combined = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        let bead_ids: Vec<String> = combined
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                trimmed
                    .split_once("] ")
                    .and_then(|(_, rest)| {
                        rest.split_once(": ").map(|(before, _)| before)
                    })
                    .and_then(|before| {
                        before.rsplit(' ').next()
                    })
                    .filter(|id| id.starts_with("ve-"))
                    .map(|s| s.to_string())
            })
            .collect();

        for bead_id in bead_ids {
            let release = Command::new("bd")
                .args(["update", &bead_id, "--status", "open", "--assignee", ""])
                .current_dir(SRC_DIR)
                .env("BD_DOLT_AUTO_COMMIT", "off")
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .status()
                .await;

            match release {
                Ok(s) if s.success() => {
                    info!("Recovered stale bead {} from dead polecat {}", bead_id, name.as_str());
                    recovered += 1;
                }
                Ok(_) | Err(_) => {
                    debug!("Could not release bead {} for {}", bead_id, name.as_str());
                }
            }
        }
    }

    recovered
}

pub async fn check_polecat_status(name: &PolecatName) -> Result<PolecatStatus, FleetError> {
    let session = name.tmux_session();

    let exists_output = Command::new("tmux")
        .args(["has-session", "-t", &session])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map_err(|e| FleetError::Tmux(e.to_string()))?;

    if !exists_output.success() {
        return Ok(classify_status(false, false));
    }

    let pid_output = Command::new("tmux")
        .args(["list-panes", "-t", &session, "-F", "#{pane_pid}"])
        .output()
        .await
        .map_err(|e| FleetError::Tmux(e.to_string()))?;

    let pid_str = String::from_utf8_lossy(&pid_output.stdout).trim().to_string();
    let has_children = if pid_str.is_empty() {
        false
    } else {
        let pgrep_output = Command::new("pgrep")
            .args(["-P", &pid_str])
            .output()
            .await
            .map_err(|e| FleetError::Tmux(e.to_string()))?;

        !String::from_utf8_lossy(&pgrep_output.stdout).trim().is_empty()
    };

    Ok(classify_status(true, has_children))
}

/// Fetch 50 ready beads from `bd ready --json`, falling back to plain text parsing.
pub async fn fetch_ready_beads() -> Result<Vec<BeadJson>, FleetError> {
    let output = Command::new("bd")
        .args(["ready", "-n", "50", "--json"])
        .current_dir(SRC_DIR)
        .env("BD_DOLT_AUTO_COMMIT", "off")
        .output()
        .await
        .map_err(|e| FleetError::Bd(e.to_string()))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Try JSON parse from stdout first
    if let Ok(beads) = serde_json::from_str::<Vec<BeadJson>>(&stdout) {
        if !beads.is_empty() {
            return Ok(beads);
        }
    }

    // Fallback: parse plain text from combined stdout + stderr
    let combined = format!("{stdout}\n{stderr}");
    let beads: Vec<BeadJson> = combined
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            trimmed.split_once("] ").and_then(|(_, rest)| {
                rest.split_once(": ").and_then(|(before_id, _)| {
                    before_id
                        .rsplit(' ')
                        .next()
                        .filter(|id| id.starts_with("ve-"))
                        .map(|id| BeadJson {
                            id: id.to_string(),
                            assignee: None,
                        })
                })
            })
        })
        .collect();

    Ok(beads)
}

pub async fn assign_bead(bead: &BeadId, name: &PolecatName) -> Result<(), FleetError> {
    let assignee = name.role();
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(8),
        Command::new("bd")
            .args([
                "update",
                bead.as_str(),
                "--claim",
                "--assignee",
                &assignee,
            ])
            .current_dir(SRC_DIR)
            .env("BD_DOLT_AUTO_COMMIT", "off")
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output(),
    )
    .await
    .map_err(|_| FleetError::Bd("bd update timed out".into()))?
    .map_err(|e| FleetError::Bd(e.to_string()))?;

    if output.status.success() {
        Ok(())
    } else {
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
}

pub async fn prepare_worktree(name: &PolecatName) -> Result<(), FleetError> {
    let clone = name.worktree_path();
    let metadata_src = format!("{}/.beads/metadata.json", SRC_DIR);
    let metadata_dst = clone.join(".beads/metadata.json");

    // Ensure .beads directory exists
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

pub async fn launch_session(
    entry: &FleetEntry,
    bead: &BeadId,
) -> Result<(), FleetError> {
    let name = &entry.name;
    let session = name.tmux_session();
    let clone = name.worktree_path();
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
        .map_err(|e| FleetError::Git(e.to_string()))?;

    let branch = String::from_utf8_lossy(&branch_output.stdout)
        .trim()
        .to_string();

    let env = build_env_vars(entry, &branch);
    let pre = build_pre_launch(&clone_str);
    let prompt = build_prompt(name, bead);

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

/// Feed one polecat: check status, find a unique unassigned bead, assign, launch.
/// FIXED: tracks claimed bead IDs so each polecat gets a different bead.
pub async fn feed_polecat(
    entry: &FleetEntry,
    beads: &[BeadJson],
    claimed: &HashSet<String>,
) -> (FeedOutcome, Option<String>) {
    let name = &entry.name;

    match check_polecat_status(name).await {
        Ok(PolecatStatus::Working) => {
            debug!("{}: working, skipping", name.as_str());
            (FeedOutcome::SkippedWorking, None)
        }
        Ok(_status) => {
            // Find first unassigned bead NOT already claimed in this cycle
            let bead_opt = beads
                .iter()
                .find(|b| b.assignee.is_none() && !claimed.contains(&b.id))
                .map(|b| BeadId(b.id.clone()));

            let bead = match bead_opt {
                Some(b) => b,
                None => {
                    info!("{}: no unassigned beads available", name.as_str());
                    return (FeedOutcome::SkippedNoBeads, None);
                }
            };

            if let Err(e) = assign_bead(&bead, name).await {
                warn!("{}: assign failed: {}", name.as_str(), e);
                return (FeedOutcome::AssignFailed, None);
            }

            if let Err(e) = prepare_worktree(name).await {
                warn!("{}: worktree prep failed: {}", name.as_str(), e);
                return (FeedOutcome::LaunchFailed, None);
            }

            if let Err(e) = launch_session(entry, &bead).await {
                warn!("{}: launch failed: {}", name.as_str(), e);
                return (FeedOutcome::LaunchFailed, None);
            }

            (FeedOutcome::Fed, Some(bead.as_str().to_string()))
        }
        Err(e) => {
            warn!("{}: status check failed: {}", name.as_str(), e);
            (FeedOutcome::LaunchFailed, None)
        }
    }
}

/// Run the full fleet feed cycle.
/// Fetches beads ONCE, tracks claimed IDs across all polecats, recovers stale beads from dead polecats.
pub async fn run_fleet_feed() -> FeedSummary {
    info!("=== Fleet feed start ===");
    let fleet = Fleet::all();
    let mut summary = FeedSummary::default();

    // Ensure dolt is alive before doing anything
    if let Err(e) = ensure_dolt_alive().await {
        warn!("Dolt health check failed: {}", e);
    }

    // Check statuses first to find dead polecats for stale recovery
    let mut dead_polecats: Vec<&PolecatName> = Vec::new();

    for entry in &fleet {
        if let Ok(PolecatStatus::Dead) = check_polecat_status(&entry.name).await {
            dead_polecats.push(&entry.name);
        }
    }

    // Recover stale beads from dead polecats
    if !dead_polecats.is_empty() {
        info!(
            "Found {} dead polecats, attempting stale bead recovery",
            dead_polecats.len()
        );
        let recovered = recover_stale_beads(&dead_polecats).await;
        if recovered > 0 {
            info!("Recovered {} stale beads", recovered);
        }
    }

    // Fetch beads ONCE for the whole cycle
    let beads = match fetch_ready_beads().await {
        Ok(b) => b,
        Err(e) => {
            warn!("Failed to fetch beads: {}", e);
            info!(
                "=== Fleet feed done: fed={} working={} no_beads={} assign_fail={} launch_fail={} ===",
                0, 0, 0, 0, 0
            );
            return summary;
        }
    };

    info!("Fetched {} ready beads", beads.len());

    // Track which bead IDs we've claimed in this cycle
    let mut claimed: HashSet<String> = HashSet::new();

    for entry in fleet {
        let (outcome, bead_id) = feed_polecat(&entry, &beads, &claimed).await;
        let was_fed = matches!(outcome, FeedOutcome::Fed);
        if let Some(id) = bead_id {
            claimed.insert(id);
        }
        summary.record(outcome);

        // Stagger: pause after each feed to let dolt recover
        if was_fed {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    }

    info!(
        "=== Fleet feed done: fed={} working={} no_beads={} assign_fail={} launch_fail={} ===",
        summary.fed,
        summary.skipped_working,
        summary.skipped_no_beads,
        summary.assign_failed,
        summary.launch_failed,
    );

    summary
}
