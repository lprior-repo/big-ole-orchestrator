use crate::data::{FleetError, PolecatName, Rig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, SystemTime};
use tokio::process::Command;
use tracing::info;

const STALL_THRESHOLD: Duration = Duration::from_secs(600);

/// Persistent state for idle timestamp tracking.
/// File: {gt_root}/fleet-state.json
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FleetState {
    /// Map from "rig_name/polecat_name" -> unix epoch seconds when went idle
    idle_since: HashMap<String, u64>,
}

fn state_path(gt_root: &str) -> PathBuf {
    PathBuf::from(format!("{gt_root}/fleet-state.json"))
}

/// Load fleet state from disk. Never panics on corrupt JSON.
pub fn load_state(gt_root: &str) -> FleetState {
    let path = state_path(gt_root);
    let Ok(data) = std::fs::read_to_string(&path) else {
        return FleetState::default();
    };
    serde_json::from_str(&data).unwrap_or_default()
}

/// Save fleet state to disk. Invariant: file is always valid JSON after write.
pub fn save_state(gt_root: &str, state: &FleetState) {
    let path = state_path(gt_root);
    if let Ok(data) = serde_json::to_string_pretty(state) {
        let _ = std::fs::write(&path, data);
    }
}

fn state_key(rig_name: &str, name: &PolecatName) -> String {
    format!("{rig_name}/{}", name.as_str())
}

/// Record that a polecat is now idle.
pub fn mark_idle(rig_name: &str, name: &PolecatName, state: &mut FleetState) {
    let key = state_key(rig_name, name);
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    state.idle_since.insert(key, secs);
}

/// Clear idle timestamp for a polecat (now working or dead).
pub fn clear_idle(rig_name: &str, name: &PolecatName, state: &mut FleetState) {
    let key = state_key(rig_name, name);
    state.idle_since.remove(&key);
}

/// Check if a polecat has been idle longer than the stall threshold.
pub fn is_stalled(rig_name: &str, name: &PolecatName, state: &FleetState) -> bool {
    let key = state_key(rig_name, name);
    let Some(&timestamp) = state.idle_since.get(&key) else {
        return false;
    };
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    now.saturating_sub(timestamp) > STALL_THRESHOLD.as_secs()
}

/// Kill a stalled polecat's tmux session.
pub async fn restart_stalled_polecat(
    rig: &Rig,
    name: &PolecatName,
    state: &mut FleetState,
) -> Result<bool, FleetError> {
    let session = name.tmux_session(rig);

    let result = Command::new("tmux")
        .args(["kill-session", "-t", &session])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map_err(|e| FleetError::Tmux(e.to_string()))?;

    clear_idle(rig.name, name, state);

    if result.success() {
        info!("{}: killed stalled session for {}", rig.name, name.as_str());
        Ok(true)
    } else {
        Ok(false)
    }
}
