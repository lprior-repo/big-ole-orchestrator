#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![forbid(unsafe_code)]

use serde::Deserialize;
use std::borrow::Cow;
use std::path::PathBuf;

// ── Domain Types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PolecatName(Cow<'static, str>);

impl PolecatName {
    pub const fn new(s: &'static str) -> Self {
        Self(Cow::Borrowed(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn tmux_session(&self) -> String {
        format!("ve-{}", self.0)
    }

    pub fn worktree_path(&self) -> PathBuf {
        PathBuf::from(format!(
            "/home/lewis/gt/veloxide/polecats/{}/veloxide",
            self.0
        ))
    }

    pub fn role(&self) -> String {
        format!("veloxide/polecats/{}", self.0)
    }

    pub fn agent_name(&self) -> String {
        format!("veloxide/{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
pub struct BeadId(pub String);

impl BeadId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeKind {
    OpenCode,
    Claude,
}

#[derive(Debug, Clone)]
pub struct RuntimeSpec {
    pub kind: RuntimeKind,
    pub model: &'static str,
    pub agent_flag: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolecatStatus {
    Working,
    Idle,
    Dead,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedOutcome {
    Fed,
    SkippedWorking,
    SkippedNoBeads,
    AssignFailed,
    LaunchFailed,
}

#[derive(Debug, Default)]
pub struct FeedSummary {
    pub fed: u32,
    pub skipped_working: u32,
    pub skipped_no_beads: u32,
    pub assign_failed: u32,
    pub launch_failed: u32,
}

impl FeedSummary {
    pub fn record(&mut self, outcome: FeedOutcome) {
        match outcome {
            FeedOutcome::Fed => self.fed += 1,
            FeedOutcome::SkippedWorking => self.skipped_working += 1,
            FeedOutcome::SkippedNoBeads => self.skipped_no_beads += 1,
            FeedOutcome::AssignFailed => self.assign_failed += 1,
            FeedOutcome::LaunchFailed => self.launch_failed += 1,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct BeadJson {
    pub id: String,
    pub assignee: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FleetEntry {
    pub name: PolecatName,
    pub runtime: RuntimeSpec,
}

pub struct Fleet;

impl Fleet {
    pub fn all() -> Vec<FleetEntry> {
        let mut entries = Vec::with_capacity(20);

        let minimax_spec = RuntimeSpec {
            kind: RuntimeKind::OpenCode,
            model: "minimax-coding-plan/MiniMax-M2.7-highspeed",
            agent_flag: "opencode-minimax",
        };
        let glm51_spec = RuntimeSpec {
            kind: RuntimeKind::OpenCode,
            model: "zai-coding-plan/glm-5.1",
            agent_flag: "opencode-glm51",
        };
        let glm5t_spec = RuntimeSpec {
            kind: RuntimeKind::OpenCode,
            model: "zai-coding-plan/glm-5-turbo",
            agent_flag: "opencode-glm5t",
        };
        let qwen5090_spec = RuntimeSpec {
            kind: RuntimeKind::OpenCode,
            model: "qwen36-5090/Qwen3.6-35B-A3B-UD-Q5_K_XL.gguf",
            agent_flag: "opencode-qwen5090",
        };
        let qwen3090_spec = RuntimeSpec {
            kind: RuntimeKind::OpenCode,
            model: "qwen36-3090/Qwen3.6-35B-A3B-UD-Q4_K_XL.gguf",
            agent_flag: "opencode-qwen3090",
        };
        let claude_opus_spec = RuntimeSpec {
            kind: RuntimeKind::Claude,
            model: "opus",
            agent_flag: "claude",
        };
        let claude_sonnet_spec = RuntimeSpec {
            kind: RuntimeKind::Claude,
            model: "sonnet",
            agent_flag: "claude-sonnet",
        };

        // 8 MiniMax
        let minimax_names: [&'static str; 8] = [
            "brahmin", "chrome", "dust", "fury", "ghoul", "guzzle", "mirelurk", "mutant",
        ];
        // 2 GLM-5.1
        let glm51_names: [&'static str; 2] = ["nuka", "pipboy"];
        // 2 GLM-5T
        let glm5t_names: [&'static str; 2] = ["radrat", "scavenger"];
        // 2 Qwen-5090
        let qwen5090_names: [&'static str; 2] = ["vault", "thunder"];
        // 2 Qwen-3090
        let qwen3090_names: [&'static str; 2] = ["gecko", "lancer"];
        // 2 Claude Opus
        let claude_opus_names: [&'static str; 2] = ["rust", "deathclaw"];
        // 2 Claude Sonnet
        let claude_sonnet_names: [&'static str; 2] = ["shiny", "synth"];

        let push = |entries: &mut Vec<FleetEntry>, name: &'static str, spec: &RuntimeSpec| {
            entries.push(FleetEntry {
                name: PolecatName::new(name),
                runtime: spec.clone(),
            });
        };

        minimax_names.iter().for_each(|&n| push(&mut entries, n, &minimax_spec));
        glm51_names.iter().for_each(|&n| push(&mut entries, n, &glm51_spec));
        glm5t_names.iter().for_each(|&n| push(&mut entries, n, &glm5t_spec));
        qwen5090_names.iter().for_each(|&n| push(&mut entries, n, &qwen5090_spec));
        qwen3090_names.iter().for_each(|&n| push(&mut entries, n, &qwen3090_spec));
        claude_opus_names.iter().for_each(|&n| push(&mut entries, n, &claude_opus_spec));
        claude_sonnet_names.iter().for_each(|&n| push(&mut entries, n, &claude_sonnet_spec));

        entries
    }
}

pub const SRC_DIR: &str = "/home/lewis/src/veloxide";
pub const GT_ROOT: &str = "/home/lewis/gt";
pub const RIG_NAME: &str = "veloxide";

#[derive(Debug, thiserror::Error)]
pub enum FleetError {
    #[error("tmux command failed: {0}")]
    Tmux(String),
    #[error("bd command failed: {0}")]
    Bd(String),
    #[error("git command failed: {0}")]
    Git(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("UTF-8 error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
}
