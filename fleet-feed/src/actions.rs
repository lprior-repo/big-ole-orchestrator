use crate::calculations::{
    build_claude_launch_cmd, build_env_vars, build_opencode_launch_cmd, build_pre_launch,
    build_prompt, classify_status,
};
use crate::data::{
    BeadCategory, BeadId, BeadJson, FeedOutcome, FeedSummary, Fleet, FleetEntry, FleetError,
    FleetMetrics, ModuleMetrics, PolecatName, PolecatStatus, Rig, RuntimeKind,
};
use std::collections::HashSet;
use std::path::PathBuf;
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

/// Release `in_progress` beads claimed by idle/dead polecats back to ready state.
pub async fn recover_stale_beads(rig: &Rig, stale_names: &[&PolecatName]) -> usize {
    let mut recovered = 0usize;

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

    let stdout = String::from_utf8_lossy(&output.stdout);
    let all_beads: Vec<BeadJson> = match serde_json::from_str(&stdout) {
        Ok(b) => b,
        Err(e) => {
            warn!("Failed to parse in-progress beads JSON: {}", e);
            return 0;
        }
    };

    let stale_roles: std::collections::HashSet<String> = stale_names
        .iter()
        .map(|n| n.role(rig))
        .collect();

    let stale_beads: Vec<&BeadJson> = all_beads
        .iter()
        .filter(|b| {
            b.assignee
                .as_ref()
                .is_some_and(|a| stale_roles.contains(a))
        })
        .collect();

    if stale_beads.is_empty() {
        return 0;
    }

    info!("Found {} stale beads to recover", stale_beads.len());

    for bead in stale_beads.iter().take(50) {
        let release = Command::new("bd")
            .args(["update", &bead.id, "--status", "open", "--assignee", ""])
            .current_dir(rig.src_dir)
            .env("BD_DOLT_AUTO_COMMIT", "off")
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .status()
            .await;

        match release {
            Ok(s) if s.success() => {
                info!("Recovered stale bead {} from idle/dead polecat", bead.id);
                recovered += 1;
            }
            Ok(_) | Err(_) => {
                debug!("Could not release bead {}", bead.id);
            }
        }
    }

    recovered
}

pub async fn check_polecat_status(name: &PolecatName, rig: &Rig) -> Result<PolecatStatus, FleetError> {
    let session = name.tmux_session(rig);

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
pub async fn fetch_ready_beads(rig: &Rig) -> Result<Vec<BeadJson>, FleetError> {
    let output = Command::new("bd")
        .args(["ready", "-n", "50", "--json"])
        .current_dir(rig.src_dir)
        .env("BD_DOLT_AUTO_COMMIT", "off")
        .output()
        .await
        .map_err(|e| FleetError::Bd(e.to_string()))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if let Ok(beads) = serde_json::from_str::<Vec<BeadJson>>(&stdout)
        && !beads.is_empty()
    {
        return Ok(beads);
    }

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
                        .filter(|id| id.starts_with(rig.bead_prefix))
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

pub async fn assign_bead(rig: &Rig, bead: &BeadId, name: &PolecatName) -> Result<(), FleetError> {
    let assignee = name.role(rig);
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

pub async fn launch_session(
    entry: &FleetEntry,
    bead: &BeadId,
) -> Result<(), FleetError> {
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
        .map_err(|e| FleetError::Git(e.to_string()))?;

    let branch = String::from_utf8_lossy(&branch_output.stdout)
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

/// Feed one polecat: check status, find a unique unassigned bead, assign, launch.
pub async fn feed_polecat(
    entry: &FleetEntry,
    beads: &[BeadJson],
    claimed: &HashSet<String>,
) -> (FeedOutcome, Option<String>) {
    let name = &entry.name;
    let rig = entry.rig;

    match check_polecat_status(name, rig).await {
        Ok(PolecatStatus::Working) => {
            debug!("{}: working, skipping", name.as_str());
            (FeedOutcome::SkippedWorking, None)
        }
        Ok(PolecatStatus::Dead | PolecatStatus::Idle) => {
            let bead_opt = beads
                .iter()
                .find(|b| b.assignee.is_none() && !claimed.contains(&b.id))
                .map(|b| BeadId(b.id.clone()));

            let Some(bead) = bead_opt else {
                info!("{}: no unassigned beads available", name.as_str());
                return (FeedOutcome::SkippedNoBeads, None);
            };

            if let Err(e) = assign_bead(rig, &bead, name).await {
                warn!("{}: assign failed: {}", name.as_str(), e);
                return (FeedOutcome::AssignFailed, None);
            }

            if let Err(e) = prepare_worktree(rig, name).await {
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

// ── Bead Generation (Ralph Loop) ───────────────────────────────────────────────

fn metrics_path(rig: &Rig) -> PathBuf {
    PathBuf::from(format!("{}/{}/fleet-metrics.json", rig.gt_root, rig.name))
}

fn load_metrics(rig: &Rig) -> FleetMetrics {
    let path = metrics_path(rig);
    let Ok(data) = std::fs::read_to_string(&path) else {
        return FleetMetrics::default();
    };
    let Ok(metrics) = serde_json::from_str::<FleetMetrics>(&data) else {
        return FleetMetrics::default();
    };
    metrics
}

fn save_metrics(rig: &Rig, metrics: &FleetMetrics) {
    let path = metrics_path(rig);
    if let Ok(data) = serde_json::to_string_pretty(metrics) {
        let _ = std::fs::write(&path, data);
    }
}

async fn scan_modules(rig: &Rig) -> Result<Vec<String>, FleetError> {
    let output = Command::new("find")
        .args([
            rig.src_dir,
            "-name", "*.rs",
            "-not", "-path", "*/target/*",
            "-not", "-path", "*/.beads/*",
            "-not", "-path", "*/.git/*",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
        .map_err(FleetError::Io)?;

    let stdout = String::from_utf8(output.stdout).map_err(FleetError::Utf8)?;
    let prefix = format!("{}/", rig.src_dir);

    let modules: Vec<String> = stdout
        .lines()
        .filter_map(|line| {
            let path = line.trim();
            path.strip_prefix(&prefix).map(String::from)
        })
        .collect();

    Ok(modules)
}

fn select_target_modules(modules: &[String], metrics: &FleetMetrics, count: usize) -> Vec<String> {
    let mut scored: Vec<(u32, &String)> = modules
        .iter()
        .map(|m| {
            let beads = metrics.modules.iter()
                .find(|mm| mm.module == *m)
                .map_or(0, |mm| mm.beads_created);
            (beads, m)
        })
        .collect();

    scored.sort_by_key(|(count, _)| *count);

    scored.into_iter()
        .take(count)
        .map(|(_, m)| m.clone())
        .collect()
}

/// Generate improvement beads when the pool runs low.
/// Creates beads across all 4 categories (BLACKHAT, QA-MANUAL, REDQUEEN, ARCH-DRIFT).
pub async fn generate_beads(rig: &Rig) -> usize {
    let modules = match scan_modules(rig).await {
        Ok(m) if !m.is_empty() => m,
        _ => return 0,
    };

    let metrics = load_metrics(rig);
    let targets = select_target_modules(&modules, &metrics, 10);

    if targets.is_empty() {
        return 0;
    }

    let categories = BeadCategory::all();
    let mut created = 0usize;
    let mut metrics = metrics;

    for (i, module) in targets.iter().enumerate() {
        let category = categories[i % categories.len()];
        let title = format!("{}: {}", category.prefix(), module);

        let result = Command::new("bd")
            .args([
                "create", &title,
                "--description", category.description(),
                "--type", "task",
                "-p", "2",
            ])
            .current_dir(rig.src_dir)
            .env("BD_DOLT_AUTO_COMMIT", "off")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;

        match result {
            Ok(status) if status.success() => {
                created += 1;
                let entry = metrics.modules.iter_mut()
                    .find(|m| m.module == *module);
                match entry {
                    Some(m) => m.beads_created += 1,
                    None => metrics.modules.push(ModuleMetrics {
                        module: module.clone(),
                        beads_created: 1,
                        beads_closed: 0,
                    }),
                }
            }
            _ => {
                debug!("Failed to create bead for {}", module);
            }
        }
    }

    if created > 0 {
        save_metrics(rig, &metrics);
        info!("{}: generated {} beads for {} modules", rig.name, created, targets.len());
    }

    created
}

// ── Main Fleet Feed Loop ───────────────────────────────────────────────────────

/// Run the full fleet feed cycle across all rigs.
pub async fn run_fleet_feed() -> FeedSummary {
    info!("=== Fleet feed start ===");
    let mut summary = FeedSummary::default();

    if let Err(e) = ensure_dolt_alive().await {
        warn!("Dolt health check failed: {}", e);
    }

    for rig in Rig::all() {
        let fleet = Fleet::for_rig(rig);
        info!("{}: processing {} polecats", rig.name, fleet.len());

        // Find idle/dead polecats for stale recovery
        let mut stale_polecats: Vec<&PolecatName> = Vec::new();
        for entry in &fleet {
            if let Ok(PolecatStatus::Dead | PolecatStatus::Idle) = check_polecat_status(&entry.name, rig).await {
                stale_polecats.push(&entry.name);
            }
        }

        if !stale_polecats.is_empty() {
            info!(
                "{}: found {} idle/dead polecats for stale recovery",
                rig.name,
                stale_polecats.len()
            );
            let recovered = recover_stale_beads(rig, &stale_polecats).await;
            if recovered > 0 {
                info!("{}: recovered {} stale beads", rig.name, recovered);
            }
        }

        // Fetch ready beads
        let beads = match fetch_ready_beads(rig).await {
            Ok(b) => b,
            Err(e) => {
                warn!("{}: failed to fetch beads: {}", rig.name, e);
                continue;
            }
        };

        info!("{}: fetched {} ready beads", rig.name, beads.len());

        // Generate beads if pool is low
        if beads.len() < 5 {
            let generated = generate_beads(rig).await;
            if generated > 0 {
                info!("{}: generated {} improvement beads", rig.name, generated);
            }
        }

        // Feed each polecat
        let mut claimed: HashSet<String> = HashSet::new();
        for entry in fleet {
            let (outcome, bead_id) = feed_polecat(&entry, &beads, &claimed).await;
            let was_fed = matches!(outcome, FeedOutcome::Fed);
            if let Some(id) = bead_id {
                claimed.insert(id);
            }
            summary.record(outcome);

            if was_fed {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
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
