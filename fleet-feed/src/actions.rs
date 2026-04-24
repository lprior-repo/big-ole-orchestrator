use crate::bead_store::{assign_bead, fetch_ready_beads, recover_stale_beads};
use crate::data::{
    BeadId, BeadJson, FeedOutcome, FeedSummary, Fleet, FleetEntry, FleetError, PolecatName,
    PolecatStatus, Rig,
};
use crate::dolt_health::{ensure_dolt_alive, guard_dolt, validate_rig_route};
use crate::generation::generate_beads;
use crate::launcher::{launch_session, prepare_worktree};
use crate::polecat_status::batch_check_polecat_status;
use std::collections::HashSet;
use tracing::{debug, info, warn};

const DOLT_COOLDOWN: std::time::Duration = std::time::Duration::from_millis(500);
const MAX_CONCURRENT_POLECATS: usize = 25;
const PER_RIG_QUOTA: usize = 5;

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
            feed_available_polecat(entry, beads, claimed, name, rig).await
        }
    }
}

async fn feed_available_polecat(
    entry: &FleetEntry,
    beads: &[BeadJson],
    claimed: &HashSet<String>,
    name: &PolecatName,
    rig: &Rig,
) -> (FeedOutcome, Option<String>) {
    let bead_opt = beads
        .iter()
        .find(|bead| bead.assignee.is_none() && !claimed.contains(&bead.id))
        .map(|bead| BeadId(bead.id.clone()));

    let Some(bead) = bead_opt else {
        info!("{}: no unassigned beads available", name.as_str());
        return (FeedOutcome::SkippedNoBeads, None);
    };

    if let Err(error) = assign_bead(rig, &bead, name).await {
        warn!("{}: assign failed: {}", name.as_str(), error);
        let outcome = match error {
            FleetError::AlreadyClaimed(_) => FeedOutcome::SkippedAlreadyClaimed,
            _ => FeedOutcome::AssignFailed,
        };
        return (outcome, None);
    }

    if let Err(error) = prepare_worktree(rig, name).await {
        warn!("{}: worktree prep failed: {}", name.as_str(), error);
        return (FeedOutcome::LaunchFailed, None);
    }

    if let Err(error) = launch_session(entry, &bead).await {
        warn!("{}: launch failed: {}", name.as_str(), error);
        return (FeedOutcome::LaunchFailed, None);
    }

    (FeedOutcome::Fed, Some(bead.as_str().to_string()))
}

async fn recover_stale_for_rig(
    rig: &Rig,
    fleet: &[FleetEntry],
    statuses: &[PolecatStatus],
) -> bool {
    let stale_polecats: Vec<&PolecatName> = fleet
        .iter()
        .zip(statuses)
        .filter(|(_, status)| matches!(status, PolecatStatus::Dead | PolecatStatus::Idle))
        .map(|(entry, _)| &entry.name)
        .collect();

    if stale_polecats.is_empty() {
        return true;
    }

    if !guard_dolt(rig, "stale recovery").await {
        return false;
    }

    info!(
        "{}: found {} idle/dead polecats for stale recovery",
        rig.name,
        stale_polecats.len()
    );
    let recovered = recover_stale_beads(rig, &stale_polecats).await;
    if recovered > 0 {
        info!("{}: recovered {} stale beads", rig.name, recovered);
    }

    tokio::time::sleep(DOLT_COOLDOWN).await;
    true
}

async fn ready_beads_for_rig(rig: &Rig) -> Option<Vec<BeadJson>> {
    if !guard_dolt(rig, "ready bead fetch").await {
        return None;
    }

    match fetch_ready_beads(rig).await {
        Ok(beads) => {
            info!("{}: fetched {} ready beads", rig.name, beads.len());
            Some(beads)
        }
        Err(error) => {
            warn!("{}: failed to fetch beads: {}", rig.name, error);
            None
        }
    }
}

async fn generate_if_needed(rig: &Rig, beads: &[BeadJson]) {
    if beads.len() >= 5 || !guard_dolt(rig, "bead generation").await {
        return;
    }

    let generated = generate_beads(rig).await;
    if generated > 0 {
        info!("{}: generated {} improvement beads", rig.name, generated);
    }
}

async fn process_rig(rig: &'static Rig, summary: &mut FeedSummary, total_fed: &mut usize) {
    if let Err(error) = validate_rig_route(rig).await {
        warn!(
            "{}: skipping rig because route validation failed: {}",
            rig.name, error
        );
        return;
    }

    let fleet = Fleet::for_rig(rig);
    info!("{}: processing {} polecats", rig.name, fleet.len());

    let statuses = batch_check_polecat_status(rig, &fleet).await;
    if !recover_stale_for_rig(rig, &fleet, &statuses).await {
        return;
    }

    let Some(beads) = ready_beads_for_rig(rig).await else {
        return;
    };

    generate_if_needed(rig, &beads).await;

    let mut claimed: HashSet<String> = HashSet::new();
    let mut rig_fed = 0usize;

    for (entry, status) in fleet.into_iter().zip(statuses) {
        if *total_fed >= MAX_CONCURRENT_POLECATS || rig_fed >= PER_RIG_QUOTA {
            break;
        }

        let (outcome, bead_id) = feed_polecat_with_status(&entry, status, &beads, &claimed).await;
        let was_fed = matches!(outcome, FeedOutcome::Fed);
        if let Some(id) = bead_id {
            claimed.insert(id);
        }
        summary.record(outcome);

        if was_fed {
            rig_fed += 1;
            *total_fed += 1;
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        }
    }
}

/// Run the full fleet feed cycle across all rigs with round-robin rotation.
pub async fn run_fleet_feed() -> FeedSummary {
    info!("=== Fleet feed start ===");
    let mut summary = FeedSummary::default();

    if let Err(error) = ensure_dolt_alive().await {
        warn!("Dolt health check failed: {}", error);
    }

    let mut total_fed = 0usize;

    for rig in Rig::all() {
        if total_fed >= MAX_CONCURRENT_POLECATS {
            info!(
                "Global cap of {} reached, stopping",
                MAX_CONCURRENT_POLECATS
            );
            break;
        }

        process_rig(rig, &mut summary, &mut total_fed).await;
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }

    info!(
        "=== Fleet feed done: fed={} working={} no_beads={} already_claimed={} assign_fail={} launch_fail={} ===",
        summary.fed,
        summary.skipped_working,
        summary.skipped_no_beads,
        summary.skipped_already_claimed,
        summary.assign_failed,
        summary.launch_failed,
    );

    summary
}
