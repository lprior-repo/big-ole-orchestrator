use crate::data::{BeadId, BeadJson, FleetError, PolecatName, Rig};
use std::collections::HashSet;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, info, warn};

const DOLT_MUTATION_COOLDOWN: std::time::Duration = std::time::Duration::from_millis(250);
const STALE_RECOVERY_CAP: usize = 5;

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
        Ok(output) => output,
        Err(error) => {
            warn!("Failed to list in-progress beads: {}", error);
            return 0;
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let all_beads: Vec<BeadJson> = match serde_json::from_str(&stdout) {
        Ok(beads) => beads,
        Err(error) => {
            warn!("Failed to parse in-progress beads JSON: {}", error);
            return 0;
        }
    };

    let stale_roles: HashSet<String> = stale_names.iter().map(|name| name.role(rig)).collect();
    let stale_beads: Vec<&BeadJson> = all_beads
        .iter()
        .filter(|bead| {
            bead.assignee
                .as_ref()
                .is_some_and(|assignee| stale_roles.contains(assignee))
        })
        .collect();

    if stale_beads.is_empty() {
        return 0;
    }

    info!("Found {} stale beads to recover", stale_beads.len());
    release_stale_beads(rig, &stale_beads).await
}

async fn release_stale_beads(rig: &Rig, stale_beads: &[&BeadJson]) -> usize {
    let mut recovered = 0usize;

    for bead in stale_beads.iter().take(STALE_RECOVERY_CAP) {
        let release = Command::new("bd")
            .args(["update", &bead.id, "--status", "open", "--assignee", ""])
            .current_dir(rig.src_dir)
            .env("BD_DOLT_AUTO_COMMIT", "off")
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .status()
            .await;

        match release {
            Ok(status) if status.success() => {
                info!("Recovered stale bead {} from idle/dead polecat", bead.id);
                recovered += 1;
                tokio::time::sleep(DOLT_MUTATION_COOLDOWN).await;
            }
            Ok(_) | Err(_) => {
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
        .map_err(|error| FleetError::Bd(error.to_string()))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if let Ok(beads) = serde_json::from_str::<Vec<BeadJson>>(&stdout)
        && !beads.is_empty()
    {
        return Ok(beads);
    }

    Ok(parse_plain_ready_output(rig, &stdout, &stderr))
}

fn parse_plain_ready_output(rig: &Rig, stdout: &str, stderr: &str) -> Vec<BeadJson> {
    let combined = format!("{stdout}\n{stderr}");
    combined
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
        .collect()
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
    .map_err(|error| FleetError::Bd(error.to_string()))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("already claimed") {
        Err(FleetError::AlreadyClaimed(bead.as_str().to_string()))
    } else {
        Err(FleetError::Bd(format!(
            "bd update failed for {}: {}",
            bead.as_str(),
            stderr.trim()
        )))
    }
}
