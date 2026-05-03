use crate::data::{BeadId, FleetEntry, PolecatName, PolecatStatus, Rig};
use std::collections::{HashMap, HashSet};

pub fn build_prompt(rig: &Rig, name: &PolecatName, bead: &BeadId) -> String {
    format!(
        "[GAS TOWN] polecat {} (rig: {}). \
         Claim bead {}. Run bd update {} --claim. \
         Then gt prime --hook and begin work. \
         AFTER completing your bead: \
         (1) Write ALL findings/notes to .beads/{}/findings.md. \
         (2) Run bd close {} --reason Completed-by-{}. \
         (3) If you changed code: git add -A && \
         git commit -m polecat/{}-completed-{} && \
         git pull --rebase origin main && \
         git push origin HEAD:main --force-with-lease. \
         (4) If NO code changes (QA/audit only): skip git push, just exit. \
         NEVER run gt done. Exit cleanly after bd close.",
        name.as_str(),
        rig.name,
        bead.as_str(),
        bead.as_str(),
        bead.as_str(),
        bead.as_str(),
        name.as_str(),
        name.as_str(),
        bead.as_str(),
    )
}

pub fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        "''".to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\"'\"'"))
    }
}

pub fn build_env_vars(entry: &FleetEntry, branch: &str) -> String {
    let name = &entry.name;
    let rig = entry.rig;
    let clone = name.worktree_path(rig);
    format!(
        "GT_BRANCH={branch} \
         GT_POLECAT={p} \
         GT_POLECAT_PATH={clone} \
         GT_RIG={rig_name} \
         GT_ROLE={role} \
         GT_TOWN_ROOT={gt_root} \
         BD_ACTOR={role} \
         BD_DOLT_AUTO_COMMIT=off \
         BEADS_AGENT_NAME={agent} \
         BEADS_DOLT_PORT={dolt_port} \
         GT_DOLT_PORT={dolt_port} \
         GT_AGENT={agent_flag}",
        branch = branch,
        p = name.as_str(),
        clone = clone.display(),
        rig_name = rig.name,
        role = name.role(rig),
        gt_root = rig.gt_root,
        agent = name.agent_name(rig),
        agent_flag = entry.runtime.agent_flag,
        dolt_port = rig.dolt_port,
    )
}

pub fn build_pre_launch(clone_path: &str) -> String {
    let quoted_clone = shell_quote(clone_path);
    format!(
        "cd {quoted_clone} && \
         git checkout main && \
         git pull origin main && \
         gt agents fix -a 2>/dev/null; \
         rm -f .runtime/agent.lock"
    )
}

pub fn build_opencode_launch_cmd(
    entry: &FleetEntry,
    _bead: &BeadId,
    env: &str,
    pre: &str,
    prompt: &str,
) -> String {
    let quoted_prompt = shell_quote(prompt);
    format!(
        "export {env} GT_PROCESS_NAMES=opencode,node,bun \
         OPENCODE_PERMISSION='{{\"*\":\"allow\"}}' && \
         {pre} && \
         opencode -m {model} --prompt {quoted_prompt}",
        model = entry.runtime.model,
    )
}

pub fn build_claude_launch_cmd(
    entry: &FleetEntry,
    _bead: &BeadId,
    env: &str,
    pre: &str,
    prompt: &str,
) -> String {
    let quoted_prompt = shell_quote(prompt);
    format!(
        "export {env} GT_PROCESS_NAMES=claude && \
         {pre} && \
         claude --model {model} --dangerously-skip-permissions {quoted_prompt}",
        model = entry.runtime.model,
    )
}

pub const fn classify_status(session_exists: bool, has_children: bool) -> PolecatStatus {
    match (session_exists, has_children) {
        (true, true) => PolecatStatus::Working,
        (true, false) => PolecatStatus::Idle,
        (false, _) => PolecatStatus::Dead,
    }
}

pub fn parse_tmux_session_pids(stdout: &str) -> HashMap<String, String> {
    stdout
        .lines()
        .filter_map(|line| {
            line.trim()
                .split_once(':')
                .map(|(name, pid)| (name.to_string(), pid.trim().to_string()))
        })
        .collect()
}

pub fn parse_active_parent_pids(stdout: &str) -> HashSet<String> {
    stdout
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            match (parts.next(), parts.next()) {
                (Some(ppid), Some(_pid)) if !ppid.is_empty() && ppid != "0" => {
                    Some(ppid.to_string())
                }
                _ => None,
            }
        })
        .collect()
}

pub fn classify_batch_status(
    session: &str,
    session_pids: &HashMap<String, String>,
    active_parent_pids: &HashSet<String>,
) -> PolecatStatus {
    match session_pids.get(session) {
        Some(pid) if pid.is_empty() => PolecatStatus::Idle,
        Some(pid) => classify_status(true, active_parent_pids.contains(pid)),
        None => PolecatStatus::Dead,
    }
}

/// Compute a proportional quota for how many beads a pool can consume.
#[cfg(test)]
///
/// # Preconditions
/// - `pool_size` <= `total_ready` (pool count is a subset of total)
/// - `remaining` > 0 (caller checked there is capacity)
/// - `max_per_rig` > 0 (hard ceiling)
///
/// # Postconditions
/// - Returns 0 when `pool_size` is 0
/// - Returns <= `max_per_rig` always
/// - Returns <= `remaining` always
pub fn proportional_rig_quota(
    pool_size: usize,
    total_ready: usize,
    remaining: usize,
    max_per_rig: usize,
) -> usize {
    if pool_size == 0 || total_ready == 0 || remaining == 0 {
        return 0;
    }
    let raw = (pool_size * remaining) / total_ready;
    raw.min(max_per_rig).min(remaining)
}

/// Determine if a pool needs seeding.
#[cfg(test)]
pub const fn should_seed_pool(current_count: usize, threshold: usize) -> bool {
    current_count < threshold
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{
        BeadJson, Fleet, HARDLINE_RIG, OYA_RIG, Rig, RuntimeKind, VELOXIDE_RIG,
    };

    #[test]
    fn prompt_contains_bead_and_findings_path() {
        let name = PolecatName::new("brahmin");
        let bead = BeadId("ve-6v8i7".into());
        let prompt = build_prompt(&VELOXIDE_RIG, &name, &bead);
        assert!(prompt.contains("ve-6v8i7"));
        assert!(prompt.contains("brahmin"));
        assert!(prompt.contains("findings.md"));
        assert!(prompt.contains("bd close"));
        assert!(prompt.contains("NEVER run gt done"));
        assert!(!prompt.contains('\''));
    }

    #[test]
    fn prompt_uses_rig_name() {
        let name = PolecatName::new("rust");
        let bead = BeadId("ha-test".into());
        let prompt = build_prompt(&HARDLINE_RIG, &name, &bead);
        assert!(prompt.contains("rig: hardline"));
        assert!(!prompt.contains("rig: veloxide"));
    }

    #[test]
    fn classify_status_working() {
        assert_eq!(classify_status(true, true), PolecatStatus::Working);
    }

    #[test]
    fn classify_status_idle() {
        assert_eq!(classify_status(true, false), PolecatStatus::Idle);
    }

    #[test]
    fn classify_status_dead() {
        assert_eq!(classify_status(false, false), PolecatStatus::Dead);
        assert_eq!(classify_status(false, true), PolecatStatus::Dead);
    }

    #[test]
    fn parse_tmux_session_pids_collects_named_sessions() {
        let sessions = parse_tmux_session_pids("ve-brahmin:101\nve-rust:202\n");
        assert_eq!(sessions.get("ve-brahmin"), Some(&"101".to_string()));
        assert_eq!(sessions.get("ve-rust"), Some(&"202".to_string()));
    }

    #[test]
    fn parse_active_parent_pids_collects_parent_processes() {
        let parents = parse_active_parent_pids("101 9001\n202 9002\n202 9003\n0 1\n");
        assert!(parents.contains("101"));
        assert!(parents.contains("202"));
        assert!(!parents.contains("0"));
    }

    #[test]
    fn classify_batch_status_distinguishes_dead_idle_and_working() {
        let sessions = parse_tmux_session_pids("ve-brahmin:101\nve-rust:\n");
        let active = parse_active_parent_pids("101 9001\n");

        assert_eq!(
            classify_batch_status("ve-brahmin", &sessions, &active),
            PolecatStatus::Working
        );
        assert_eq!(
            classify_batch_status("ve-rust", &sessions, &active),
            PolecatStatus::Idle
        );
        assert_eq!(
            classify_batch_status("ve-missing", &sessions, &active),
            PolecatStatus::Dead
        );
    }

    #[test]
    fn fleet_has_23_entries() {
        let fleet = Fleet::for_rig(&VELOXIDE_RIG);
        assert_eq!(fleet.len(), 23);
    }

    #[test]
    fn hardline_fleet_has_23_entries() {
        let fleet = Fleet::for_rig(&HARDLINE_RIG);
        assert_eq!(fleet.len(), 23);
    }

    #[test]
    fn oya_fleet_has_23_entries() {
        let fleet = Fleet::for_rig(&OYA_RIG);
        assert_eq!(fleet.len(), 23);
    }

    #[test]
    fn fleet_runtimes_are_correct() {
        let fleet = Fleet::for_rig(&VELOXIDE_RIG);
        let opencode_count = fleet
            .iter()
            .filter(|e| e.runtime.kind == RuntimeKind::OpenCode)
            .count();
        let claude_count = fleet
            .iter()
            .filter(|e| e.runtime.kind == RuntimeKind::Claude)
            .count();
        assert_eq!(opencode_count, 19);
        assert_eq!(claude_count, 4);
    }

    #[test]
    fn env_vars_contain_required_fields() {
        let entry = Fleet::for_rig(&VELOXIDE_RIG)
            .into_iter()
            .find(|e| e.name.as_str() == "brahmin")
            .unwrap();
        let env = build_env_vars(&entry, "main");
        assert!(env.contains("GT_POLECAT=brahmin"));
        assert!(env.contains("BD_DOLT_AUTO_COMMIT=off"));
        assert!(env.contains("GT_DOLT_PORT=3307"));
        assert!(env.contains("GT_AGENT=opencode-minimax"));
    }

    #[test]
    fn opencode_uses_prompt_flag() {
        let entry = Fleet::for_rig(&VELOXIDE_RIG)
            .into_iter()
            .find(|e| e.name.as_str() == "brahmin")
            .unwrap();
        let cmd = build_opencode_launch_cmd(
            &entry,
            &BeadId("ve-test".into()),
            "GT_TEST=1",
            "echo prep",
            "do work",
        );
        assert!(cmd.contains("--prompt"));
    }

    #[test]
    fn claude_does_not_use_prompt_flag() {
        let entry = Fleet::for_rig(&VELOXIDE_RIG)
            .into_iter()
            .find(|e| e.name.as_str() == "rust")
            .unwrap();
        let cmd = build_claude_launch_cmd(
            &entry,
            &BeadId("ve-test".into()),
            "GT_TEST=1",
            "echo prep",
            "do work",
        );
        assert!(!cmd.contains("--prompt"));
        assert!(cmd.contains("--dangerously-skip-permissions"));
    }

    #[test]
    fn find_unassigned_skips_claimed_in_batch() {
        let beads = vec![
            BeadJson {
                id: "ve-abc".into(),
                assignee: Some("a".into()),
            },
            BeadJson {
                id: "ve-def".into(),
                assignee: None,
            },
            BeadJson {
                id: "ve-ghi".into(),
                assignee: None,
            },
        ];
        let mut claimed: Vec<String> = Vec::new();

        let first = beads
            .iter()
            .find(|b| b.assignee.is_none() && !claimed.contains(&b.id))
            .map(|b| {
                claimed.push(b.id.clone());
                BeadId(b.id.clone())
            });
        assert_eq!(first.map(|b| b.as_str().to_string()), Some("ve-def".into()));

        let second = beads
            .iter()
            .find(|b| b.assignee.is_none() && !claimed.contains(&b.id))
            .map(|b| {
                claimed.push(b.id.clone());
                BeadId(b.id.clone())
            });
        assert_eq!(
            second.map(|b| b.as_str().to_string()),
            Some("ve-ghi".into())
        );
    }

    #[test]
    fn rig_all_returns_six() {
        assert_eq!(Rig::all().len(), 6);
    }

    #[test]
    fn tmux_prefix_correct() {
        let name = PolecatName::new("brahmin");
        assert_eq!(name.tmux_session(&VELOXIDE_RIG), "ve-brahmin");
        assert_eq!(name.tmux_session(&HARDLINE_RIG), "hl-brahmin");
        assert_eq!(name.tmux_session(&OYA_RIG), "oy-brahmin");
    }

    #[test]
    fn proportional_quota_gives_deeper_pool_more_capacity() {
        let shallow = proportional_rig_quota(10, 110, 22, 12);
        let deep = proportional_rig_quota(100, 110, 22, 12);
        assert_eq!(shallow, 2);
        assert_eq!(deep, 12);
    }

    #[test]
    fn proportional_quota_returns_zero_when_pool_empty() {
        assert_eq!(proportional_rig_quota(0, 100, 25, 12), 0);
    }

    #[test]
    fn seed_threshold_triggers_below_floor_only() {
        assert!(should_seed_pool(19, 20));
        assert!(!should_seed_pool(20, 20));
    }
}
