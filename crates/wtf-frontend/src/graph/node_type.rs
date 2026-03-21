#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NodeType {
    FsmEntry,
    FsmTransition,
    FsmState,
    FsmFinal,
    DagTask,
    DagSplit,
    DagJoin,
    ProceduralStep,
    ProceduralScript,
}

impl fmt::Display for NodeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::FsmEntry => "fsm-entry",
            Self::FsmTransition => "fsm-transition",
            Self::FsmState => "fsm-state",
            Self::FsmFinal => "fsm-final",
            Self::DagTask => "dag-task",
            Self::DagSplit => "dag-split",
            Self::DagJoin => "dag-join",
            Self::ProceduralStep => "procedural-step",
            Self::ProceduralScript => "procedural-script",
        };
        write!(f, "{s}")
    }
}

impl FromStr for NodeType {
    type Err = ParseNodeTypeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "fsm-entry" | "fsm_entry" => Ok(Self::FsmEntry),
            "fsm-transition" | "fsm_transition" => Ok(Self::FsmTransition),
            "fsm-state" | "fsm_state" => Ok(Self::FsmState),
            "fsm-final" | "fsm_final" => Ok(Self::FsmFinal),
            "dag-task" | "dag_task" => Ok(Self::DagTask),
            "dag-split" | "dag_split" => Ok(Self::DagSplit),
            "dag-join" | "dag_join" => Ok(Self::DagJoin),
            "procedural-step" | "procedural_step" => Ok(Self::ProceduralStep),
            "procedural-script" | "procedural_script" => Ok(Self::ProceduralScript),
            _ => Err(ParseNodeTypeError(s.to_string())),
        }
    }
}

impl NodeType {
    #[must_use]
    pub const fn is_fsm_entry(self) -> bool {
        matches!(self, Self::FsmEntry)
    }

    #[must_use]
    pub const fn is_fsm_state(self) -> bool {
        matches!(self, Self::FsmState)
    }

    #[must_use]
    pub const fn is_fsm_transition(self) -> bool {
        matches!(self, Self::FsmTransition)
    }

    #[must_use]
    pub const fn is_fsm_final(self) -> bool {
        matches!(self, Self::FsmFinal)
    }

    #[must_use]
    pub const fn is_fsm(self) -> bool {
        matches!(
            self,
            Self::FsmEntry | Self::FsmState | Self::FsmTransition | Self::FsmFinal
        )
    }

    #[must_use]
    pub const fn is_dag_task(self) -> bool {
        matches!(self, Self::DagTask)
    }

    #[must_use]
    pub const fn is_dag_split(self) -> bool {
        matches!(self, Self::DagSplit)
    }

    #[must_use]
    pub const fn is_dag_join(self) -> bool {
        matches!(self, Self::DagJoin)
    }

    #[must_use]
    pub const fn is_dag(self) -> bool {
        matches!(self, Self::DagTask | Self::DagSplit | Self::DagJoin)
    }

    #[must_use]
    pub const fn is_procedural_step(self) -> bool {
        matches!(self, Self::ProceduralStep)
    }

    #[must_use]
    pub const fn is_procedural_script(self) -> bool {
        matches!(self, Self::ProceduralScript)
    }

    #[must_use]
    pub const fn is_procedural(self) -> bool {
        matches!(self, Self::ProceduralStep | Self::ProceduralScript)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseNodeTypeError(pub String);

impl fmt::Display for ParseNodeTypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invalid NodeType: {}", self.0)
    }
}

impl std::error::Error for ParseNodeTypeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_all_nine_variants() {
        assert!(NodeType::FsmEntry.is_fsm_entry());
        assert!(NodeType::FsmState.is_fsm_state());
        assert!(NodeType::FsmTransition.is_fsm_transition());
        assert!(NodeType::FsmFinal.is_fsm_final());
        assert!(NodeType::DagTask.is_dag_task());
        assert!(NodeType::DagSplit.is_dag_split());
        assert!(NodeType::DagJoin.is_dag_join());
        assert!(NodeType::ProceduralStep.is_procedural_step());
        assert!(NodeType::ProceduralScript.is_procedural_script());
    }

    #[test]
    fn display_shows_kebab_case() {
        assert_eq!(format!("{}", NodeType::FsmEntry), "fsm-entry");
        assert_eq!(format!("{}", NodeType::FsmState), "fsm-state");
        assert_eq!(format!("{}", NodeType::DagTask), "dag-task");
    }

    #[test]
    fn from_str_parses_valid_inputs() {
        assert_eq!("fsm-entry".parse(), Ok(NodeType::FsmEntry));
        assert_eq!("fsm_entry".parse(), Ok(NodeType::FsmEntry));
        assert_eq!("dag-task".parse(), Ok(NodeType::DagTask));
    }

    #[test]
    fn from_str_rejects_invalid_input() {
        assert!("invalid".parse::<NodeType>().is_err());
    }

    #[test]
    fn is_fsm_returns_correct_values() {
        assert!(NodeType::FsmEntry.is_fsm());
        assert!(NodeType::FsmState.is_fsm());
        assert!(!NodeType::DagTask.is_fsm());
    }

    #[test]
    fn is_dag_returns_correct_values() {
        assert!(NodeType::DagTask.is_dag());
        assert!(NodeType::DagSplit.is_dag());
        assert!(NodeType::DagJoin.is_dag());
        assert!(!NodeType::FsmEntry.is_dag());
    }

    #[test]
    fn is_procedural_returns_correct_values() {
        assert!(NodeType::ProceduralStep.is_procedural());
        assert!(NodeType::ProceduralScript.is_procedural());
        assert!(!NodeType::DagTask.is_procedural());
    }

    #[test]
    fn parse_error_display() {
        let err = ParseNodeTypeError("bad-value".to_string());
        assert_eq!(format!("{}", err), "Invalid NodeType: bad-value");
    }
}
