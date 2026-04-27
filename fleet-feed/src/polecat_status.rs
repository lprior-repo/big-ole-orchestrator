use crate::calculations::{
    classify_batch_status, parse_active_parent_pids, parse_tmux_session_pids,
};
use crate::data::{FleetEntry, PolecatStatus, Rig};
use std::collections::{HashMap, HashSet};
use std::process::Stdio;
use tokio::process::Command;

async fn collect_active_parent_pids(session_pids: &HashMap<String, String>) -> HashSet<String> {
    let tracked_pids: HashSet<String> = session_pids
        .values()
        .filter(|pid| !pid.is_empty())
        .cloned()
        .collect();

    if tracked_pids.is_empty() {
        return HashSet::new();
    }

    let Ok(output) = Command::new("ps")
        .args(["-o", "ppid=", "-o", "pid=", "-ax"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
    else {
        return HashSet::new();
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_active_parent_pids(&stdout)
        .into_iter()
        .filter(|ppid| tracked_pids.contains(ppid))
        .collect()
}

/// Batch-check all polecat statuses for a rig using one `tmux` call and one `ps` sweep.
pub async fn batch_check_polecat_status(rig: &Rig, fleet: &[FleetEntry]) -> Vec<PolecatStatus> {
    let Ok(output) = Command::new("tmux")
        .args(["list-sessions", "-F", "#{session_name}:#{pane_pid}"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
    else {
        return vec![PolecatStatus::Dead; fleet.len()];
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
