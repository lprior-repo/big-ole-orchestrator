use super::*;
use crate::data::{
    BeadCategory, BeadJson, Fleet, HARDLINE_RIG, Rig, RuntimeKind, TWERK_RIG, VELOXIDE_RIG,
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
fn fleet_has_31_entries() {
    let fleet = Fleet::for_rig(&VELOXIDE_RIG);
    assert_eq!(fleet.len(), 31);
}

#[test]
fn hardline_fleet_has_31_entries() {
    let fleet = Fleet::for_rig(&HARDLINE_RIG);
    assert_eq!(fleet.len(), 31);
}

#[test]
fn twerk_fleet_has_31_entries() {
    let fleet = Fleet::for_rig(&TWERK_RIG);
    assert_eq!(fleet.len(), 31);
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
    assert_eq!(opencode_count, 23);
    assert_eq!(claude_count, 8);
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
fn bead_category_prefixes() {
    assert_eq!(BeadCategory::Blackhat.prefix(), "BLACKHAT");
    assert_eq!(BeadCategory::QaManual.prefix(), "QA-MANUAL");
    assert_eq!(BeadCategory::RedQueen.prefix(), "REDQUEEN");
    assert_eq!(BeadCategory::ArchDrift.prefix(), "ARCH-DRIFT");
}

#[test]
fn tmux_prefix_correct() {
    let name = PolecatName::new("brahmin");
    assert_eq!(name.tmux_session(&VELOXIDE_RIG), "ve-brahmin");
    assert_eq!(name.tmux_session(&HARDLINE_RIG), "hl-brahmin");
    assert_eq!(name.tmux_session(&TWERK_RIG), "tw-brahmin");
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
