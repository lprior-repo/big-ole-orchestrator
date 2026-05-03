use crate::bead_store::{assign_bead, fetch_ready_beads, recover_stale_beads};
use crate::data::{
    BeadId, BeadJson, FeedOutcome, FeedSummary, Fleet, FleetEntry, FleetError, PolecatName,
    PolecatStatus, Rig,
};
use crate::dolt_health::{ensure_dolt_alive, guard_dolt, validate_rig_route};
use crate::generation::generate_beads;
use crate::launcher::{launch_session, prepare_worktree};
use crate::polecat_restart::{self as restart};
use crate::polecat_status::batch_check_polecat_status;
use crate::scheduling::{select_bead_for_polecat, ReadyPool, RigCycle};
use crate::session_cleanup;
use crate::worktree_health::{verify_worktree, repair_worktree, cleanup_orphan_worktrees, WorktreeStatus};
use tracing::{debug, info, warn};

const DOLT_COOLDOWN: std::time::Duration = std::time::Duration::from_millis(500);
const MAX_CONCURRENT_POLECATS: usize = 25;
const PER_RIG_QUOTA: usize = 5;

async fn feed_polecat_with_selected(
    entry: &FleetEntry,
    selected_bead: &BeadId,
    selected_rig: &Rig,
    summary: &mut FeedSummary,
) -> (FeedOutcome, Option<String>) {
    let name = &entry.name;

    // Worktree health check — repair if missing
    if verify_worktree(entry.rig, name).await == WorktreeStatus::Missing {
        info!("{}: worktree missing, attempting repair", name.as_str());
        match repair_worktree(entry.rig, name).await {
            Ok(()) => summary.worktree_repaired += 1,
            Err(error) => {
                warn!("{}: worktree repair failed: {}", name.as_str(), error);
                return (FeedOutcome::LaunchFailed, None);
            }
        }
    }

    if let Err(error) = assign_bead(selected_rig, selected_bead, name).await {
        warn!("{}: assign failed: {}", name.as_str(), error);
        let outcome = match error {
            FleetError::AlreadyClaimed(_) => FeedOutcome::SkippedAlreadyClaimed,
            _ => FeedOutcome::AssignFailed,
        };
        return (outcome, None);
    }

    if let Err(error) = prepare_worktree(entry.rig, name).await {
        warn!("{}: worktree prep failed: {}", name.as_str(), error);
        return (FeedOutcome::LaunchFailed, None);
    }

    if let Err(error) = launch_session(entry, selected_bead).await {
        warn!("{}: launch failed: {}", name.as_str(), error);
        return (FeedOutcome::LaunchFailed, None);
    }

    (FeedOutcome::Fed, Some(selected_bead.as_str().to_string()))
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

/// Run the full fleet feed cycle with cross-rig scheduling.
pub async fn run_fleet_feed() -> FeedSummary {
    info!("=== Fleet feed start ===");
    let mut summary = FeedSummary::default();

    if let Err(error) = ensure_dolt_alive().await {
        warn!("Dolt health check failed: {}", error);
    }

    // Branch landing
    let landing = crate::branch_landing::land_branches_for_all_rigs().await;
    if landing.branches_landed > 0
        || landing.branches_auto_resolved > 0
        || landing.branches_escalated > 0
        || landing.branches_failed > 0
    {
        info!(
            "branch landing: landed={} auto_resolved={} escalated={} failed={}",
            landing.branches_landed,
            landing.branches_auto_resolved,
            landing.branches_escalated,
            landing.branches_failed,
        );
    }

    // Session cleanup — kill orphans and stale locks
    let all_fleet: Vec<FleetEntry> = Rig::all().iter().flat_map(Fleet::for_rig).collect();
    let sessions_killed = session_cleanup::cleanup_orphan_sessions(&all_fleet).await;
    let locks_cleaned = session_cleanup::cleanup_stale_locks(Rig::all()[0].gt_root).await;
    if sessions_killed > 0 || locks_cleaned > 0 {
        info!(
            "cleanup: killed {} orphan sessions, {} stale locks",
            sessions_killed, locks_cleaned
        );
        summary.sessions_cleaned = sessions_killed + locks_cleaned;
    }

    // Load fleet state for stalled polecat tracking
    let gt_root = Rig::all()[0].gt_root;
    let mut fleet_state = restart::load_state(gt_root);

    // Phase 1: Gather all rig data
    let mut pools: Vec<ReadyPool> = Vec::new();
    let mut cycles: Vec<RigCycle> = Vec::new();

    for rig in Rig::all() {
        if let Err(error) = validate_rig_route(rig).await {
            warn!("{}: skipping rig: {}", rig.name, error);
            continue;
        }

        let fleet = Fleet::for_rig(rig);
        let active_names: Vec<_> = fleet.iter().map(|e| e.name.clone()).collect();
        let orphans = cleanup_orphan_worktrees(rig, &active_names).await;
        if orphans > 0 {
            info!("{}: cleaned up {} orphan worktrees", rig.name, orphans);
        }
        let statuses = batch_check_polecat_status(rig, &fleet).await;

        // Stalled polecat detection
        for (entry, status) in fleet.iter().zip(&statuses) {
            match status {
                PolecatStatus::Working => {
                    restart::clear_idle(rig.name, &entry.name, &mut fleet_state);
                }
                PolecatStatus::Idle => {
                    if restart::is_stalled(rig.name, &entry.name, &fleet_state) {
                        info!("{}: {} is stalled, killing for restart", rig.name, entry.name.as_str());
                        let _ = restart::restart_stalled_polecat(
                            rig,
                            &entry.name,
                            &mut fleet_state,
                        )
                        .await;
                        summary.stalled_restarted += 1;
                    } else {
                        restart::mark_idle(rig.name, &entry.name, &mut fleet_state);
                    }
                }
                PolecatStatus::Dead => {
                    restart::clear_idle(rig.name, &entry.name, &mut fleet_state);
                }
            }
        }

        // Recalculate statuses after stall restarts
        let statuses = batch_check_polecat_status(rig, &fleet).await;

        if !recover_stale_for_rig(rig, &fleet, &statuses).await {
            restart::save_state(gt_root, &fleet_state);
            continue;
        }

        let Some(beads) = ready_beads_for_rig(rig).await else {
            restart::save_state(gt_root, &fleet_state);
            continue;
        };

        generate_if_needed(rig, &beads).await;

        pools.push(ReadyPool::new(rig, beads));
        cycles.push(RigCycle {
            rig,
            fleet,
            statuses,
        });

        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }

    restart::save_state(gt_root, &fleet_state);

    // Phase 2: Feed all polecats using cross-rig scheduling
    let mut total_fed = 0usize;

    for cycle in &cycles {
        if total_fed >= MAX_CONCURRENT_POLECATS {
            info!("Global cap of {} reached", MAX_CONCURRENT_POLECATS);
            break;
        }

        let mut rig_fed = 0usize;

        for (entry, status) in cycle.fleet.iter().zip(&cycle.statuses) {
            if total_fed >= MAX_CONCURRENT_POLECATS || rig_fed >= PER_RIG_QUOTA {
                break;
            }

            if matches!(status, PolecatStatus::Working) {
                debug!("{}: working, skipping", entry.name.as_str());
                summary.record(FeedOutcome::SkippedWorking);
                continue;
            }

            let Some(selected) = select_bead_for_polecat(
                entry.rig,
                &entry.name,
                &mut pools,
                &cycles,
            ) else {
                summary.record(FeedOutcome::SkippedNoBeads);
                continue;
            };

            let (outcome, _bead_id) = feed_polecat_with_selected(
                entry,
                &selected.bead,
                selected.rig,
                &mut summary,
            )
            .await;

            let was_fed = matches!(outcome, FeedOutcome::Fed);
            summary.record(outcome);

            if was_fed {
                rig_fed += 1;
                total_fed += 1;
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            }
        }
    }

    info!(
        "=== Fleet feed done: fed={} working={} no_beads={} already_claimed={} assign_fail={} launch_fail={} repaired={} stalled={} cleaned={} ===",
        summary.fed,
        summary.skipped_working,
        summary.skipped_no_beads,
        summary.skipped_already_claimed,
        summary.assign_failed,
        summary.launch_failed,
        summary.worktree_repaired,
        summary.stalled_restarted,
        summary.sessions_cleaned,
    );

    summary
}
