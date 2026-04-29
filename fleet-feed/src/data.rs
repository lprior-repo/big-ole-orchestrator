#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::path::PathBuf;

// ── Rig Types ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RigKind {
    Veloxide,
    Hardline,
    Twerk,
    Seshat,
    CentralizedDocs,
    Clarity,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Rig {
    pub kind: RigKind,
    pub src_dir: &'static str,
    pub gt_root: &'static str,
    pub name: &'static str,
    pub tmux_prefix: &'static str,
    pub bead_prefix: &'static str,
    pub dolt_database: &'static str,
    pub dolt_port: u16,
}

pub const VELOXIDE_RIG: Rig = Rig {
    kind: RigKind::Veloxide,
    src_dir: "/home/lewis/src/veloxide",
    gt_root: "/home/lewis/gt",
    name: "veloxide",
    tmux_prefix: "ve-",
    bead_prefix: "ve-",
    dolt_database: "veloxide",
    dolt_port: 3307,
};

pub const HARDLINE_RIG: Rig = Rig {
    kind: RigKind::Hardline,
    src_dir: "/home/lewis/src/hardline",
    gt_root: "/home/lewis/gt",
    name: "hardline",
    tmux_prefix: "hl-",
    bead_prefix: "ha-",
    dolt_database: "ha",
    dolt_port: 3307,
};

pub const TWERK_RIG: Rig = Rig {
    kind: RigKind::Twerk,
    src_dir: "/home/lewis/src/twerk",
    gt_root: "/home/lewis/gt",
    name: "twerk",
    tmux_prefix: "tw-",
    bead_prefix: "tw-",
    dolt_database: "twerk",
    dolt_port: 3307,
};

pub const SESHAT_RIG: Rig = Rig {
    kind: RigKind::Seshat,
    src_dir: "/home/lewis/src/Seshat",
    gt_root: "/home/lewis/gt",
    name: "seshat",
    tmux_prefix: "se-",
    bead_prefix: "se-",
    dolt_database: "Seshat",
    dolt_port: 3307,
};

pub const CDOCS_RIG: Rig = Rig {
    kind: RigKind::CentralizedDocs,
    src_dir: "/home/lewis/src/centralized-docs",
    gt_root: "/home/lewis/gt",
    name: "cdocs",
    tmux_prefix: "cd-",
    bead_prefix: "cd-",
    dolt_database: "cdocs",
    dolt_port: 3307,
};

pub const CLARITY_RIG: Rig = Rig {
    kind: RigKind::Clarity,
    src_dir: "/home/lewis/src/clarity",
    gt_root: "/home/lewis/gt",
    name: "clarity",
    tmux_prefix: "cl-",
    bead_prefix: "cl-",
    dolt_database: "clarity",
    dolt_port: 3307,
};

impl Rig {
    pub const fn all() -> &'static [Self] {
        &[
            VELOXIDE_RIG,
            HARDLINE_RIG,
            TWERK_RIG,
            SESHAT_RIG,
            CDOCS_RIG,
            CLARITY_RIG,
        ]
    }
}

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

    pub fn tmux_session(&self, rig: &Rig) -> String {
        format!("{}{}", rig.tmux_prefix, self.0)
    }

    pub fn worktree_path(&self, rig: &Rig) -> PathBuf {
        PathBuf::from(format!("{}/polecats/{}/{}", rig.gt_root, self.0, rig.name))
    }

    pub fn role(&self, rig: &Rig) -> String {
        format!("{}/polecats/{}", rig.name, self.0)
    }

    pub fn agent_name(&self, rig: &Rig) -> String {
        format!("{}/{}", rig.name, self.0)
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
    SkippedAlreadyClaimed,
    AssignFailed,
    LaunchFailed,
}

#[derive(Debug, Default)]
pub struct FeedSummary {
    pub fed: u32,
    pub skipped_working: u32,
    pub skipped_no_beads: u32,
    pub skipped_already_claimed: u32,
    pub assign_failed: u32,
    pub launch_failed: u32,
}

impl FeedSummary {
    pub const fn record(&mut self, outcome: FeedOutcome) {
        match outcome {
            FeedOutcome::Fed => self.fed += 1,
            FeedOutcome::SkippedWorking => self.skipped_working += 1,
            FeedOutcome::SkippedNoBeads => self.skipped_no_beads += 1,
            FeedOutcome::SkippedAlreadyClaimed => self.skipped_already_claimed += 1,
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
    pub rig: &'static Rig,
}

pub struct Fleet;

impl Fleet {
    #[allow(clippy::similar_names)]
    pub fn for_rig(rig: &'static Rig) -> Vec<FleetEntry> {
        let mut entries = Vec::with_capacity(31);

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
        let glm5_spec = RuntimeSpec {
            kind: RuntimeKind::OpenCode,
            model: "zai-coding-plan/glm-5-turbo",
            agent_flag: "opencode-glm5t",
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

        let minimax_names: [&'static str; 8] = [
            "brahmin", "chrome", "dust", "fury", "ghoul", "guzzle", "mirelurk", "mutant",
        ];
        let glm51_names: [&'static str; 3] = ["nuka", "pipboy", "nitro"];
        let glm5_names: [&'static str; 2] = ["lancer", "drifter"];
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
                rig,
            });
        };

        for &n in &minimax_names {
            push(&mut entries, n, &minimax_spec);
        }
        for &n in &glm51_names {
            push(&mut entries, n, &glm51_spec);
        }
        for &n in &glm5_names {
            push(&mut entries, n, &glm5_spec);
        }
        for &n in &glm5t_names {
            push(&mut entries, n, &glm5t_spec);
        }
        for &n in &qwen5090_names {
            push(&mut entries, n, &qwen5090_spec);
        }
        for &n in &qwen3090_names {
            push(&mut entries, n, &qwen3090_spec);
        }
        for &n in &claude_opus_names {
            push(&mut entries, n, &claude_opus_spec);
        }
        for &n in &claude_sonnet_names {
            push(&mut entries, n, &claude_sonnet_spec);
        }

        entries
    }
}

// ── Bead Generation Types ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeadCategory {
    Blackhat,
    QaManual,
    RedQueen,
    ArchDrift,
}

impl BeadCategory {
    pub const fn prefix(self) -> &'static str {
        match self {
            Self::Blackhat => "BLACKHAT",
            Self::QaManual => "QA-MANUAL",
            Self::RedQueen => "REDQUEEN",
            Self::ArchDrift => "ARCH-DRIFT",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::Blackhat => "adversarial security testing and attack surface analysis",
            Self::QaManual => "exploratory manual testing and edge case discovery",
            Self::RedQueen => "coevolutionary quality testing against implementation",
            Self::ArchDrift => "architectural drift detection and compliance verification",
        }
    }

    pub const fn all() -> &'static [Self] {
        &[
            Self::Blackhat,
            Self::QaManual,
            Self::RedQueen,
            Self::ArchDrift,
        ]
    }
}

// ── Metrics Types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModuleMetrics {
    pub module: String,
    pub beads_created: u32,
    pub beads_closed: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FleetMetrics {
    pub modules: Vec<ModuleMetrics>,
}

// ── Error Types ───────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum FleetError {
    #[error("tmux command failed: {0}")]
    Tmux(String),
    #[error("bd command failed: {0}")]
    Bd(String),
    #[error("git command failed: {0}")]
    Git(String),
    #[error("configuration error: {0}")]
    Config(String),
    #[error("bead already claimed: {0}")]
    AlreadyClaimed(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("UTF-8 error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
}
