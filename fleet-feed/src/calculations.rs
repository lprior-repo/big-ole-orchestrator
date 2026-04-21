use crate::data::{BeadId, FleetEntry, PolecatName, PolecatStatus, GT_ROOT, RIG_NAME};

pub fn build_prompt(name: &PolecatName, bead: &BeadId) -> String {
    format!(
        "[GAS TOWN] polecat {} (rig: veloxide). \
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
        bead.as_str(),
        bead.as_str(),
        bead.as_str(),
        bead.as_str(),
        name.as_str(),
        name.as_str(),
        bead.as_str(),
    )
}

pub fn build_env_vars(entry: &FleetEntry, branch: &str) -> String {
    let name = &entry.name;
    let clone = name.worktree_path();
    format!(
        "GT_BRANCH={branch} \
         GT_POLECAT={p} \
         GT_POLECAT_PATH={clone} \
         GT_RIG={rig} \
         GT_ROLE={role} \
         GT_TOWN_ROOT={gt_root} \
         BD_ACTOR={role} \
         BD_DOLT_AUTO_COMMIT=off \
         BEADS_AGENT_NAME={agent} \
         BEADS_DOLT_PORT=3307 \
         GT_DOLT_PORT=3307 \
         GT_AGENT={agent_flag}",
        branch = branch,
        p = name.as_str(),
        clone = clone.display(),
        rig = RIG_NAME,
        role = name.role(),
        gt_root = GT_ROOT,
        agent = name.agent_name(),
        agent_flag = entry.runtime.agent_flag,
    )
}

pub fn build_pre_launch(clone_path: &str) -> String {
    format!(
        "cd {clone_path} && \
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
    format!(
        "export {env} GT_PROCESS_NAMES=opencode,node,bun \
         OPENCODE_PERMISSION='{{\"*\":\"allow\"}}' && \
         {pre} && \
         opencode -m {model} --prompt \"{prompt}\"",
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
    format!(
        "export {env} GT_PROCESS_NAMES=claude && \
         {pre} && \
         claude --model {model} --dangerously-skip-permissions \"{prompt}\"",
        model = entry.runtime.model,
    )
}

pub fn classify_status(session_exists: bool, has_children: bool) -> PolecatStatus {
    match (session_exists, has_children) {
        (true, true) => PolecatStatus::Working,
        (true, false) => PolecatStatus::Idle,
        (false, _) => PolecatStatus::Dead,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{BeadJson, Fleet, RuntimeKind};

    #[test]
    fn prompt_contains_bead_and_findings_path() {
        let name = PolecatName::new("brahmin");
        let bead = BeadId("ve-6v8i7".into());
        let prompt = build_prompt(&name, &bead);
        assert!(prompt.contains("ve-6v8i7"));
        assert!(prompt.contains("brahmin"));
        assert!(prompt.contains("findings.md"));
        assert!(prompt.contains("bd close"));
        assert!(prompt.contains("NEVER run gt done"));
        assert!(!prompt.contains('\''));
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
    fn fleet_has_34_entries() {
        let fleet = Fleet::all();
        assert_eq!(fleet.len(), 34);
    }

    #[test]
    fn fleet_runtimes_are_correct() {
        let fleet = Fleet::all();
        let opencode_count = fleet
            .iter()
            .filter(|e| e.runtime.kind == RuntimeKind::OpenCode)
            .count();
        let claude_count = fleet
            .iter()
            .filter(|e| e.runtime.kind == RuntimeKind::Claude)
            .count();
        assert_eq!(opencode_count, 26);
        assert_eq!(claude_count, 8);
    }

    #[test]
    fn env_vars_contain_required_fields() {
        let entry = Fleet::all()
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
        let entry = Fleet::all()
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
        let entry = Fleet::all()
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
            BeadJson { id: "ve-abc".into(), assignee: Some("a".into()) },
            BeadJson { id: "ve-def".into(), assignee: None },
            BeadJson { id: "ve-ghi".into(), assignee: None },
        ];
        let mut claimed: Vec<String> = Vec::new();

        let first = beads.iter()
            .find(|b| b.assignee.is_none() && !claimed.contains(&b.id))
            .map(|b| {
                claimed.push(b.id.clone());
                BeadId(b.id.clone())
            });
        assert_eq!(first.map(|b| b.as_str().to_string()), Some("ve-def".into()));

        let second = beads.iter()
            .find(|b| b.assignee.is_none() && !claimed.contains(&b.id))
            .map(|b| {
                claimed.push(b.id.clone());
                BeadId(b.id.clone())
            });
        assert_eq!(second.map(|b| b.as_str().to_string()), Some("ve-ghi".into()));
    }
}
