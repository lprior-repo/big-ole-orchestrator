//! Workflow identity types for continue-as-new (ADR-038).
//!
//! [`WorkflowId`] uniquely identifies a specific epoch of a workflow lineage.
//! The format is `lineage_id@epoch` where:
//! - `lineage_id` is a stable identifier for the logical workflow
//! - `epoch` is a monotonically increasing counter for continue-as-new rollovers

use std::fmt;
use std::str::FromStr;

use thiserror::Error;
use vo_types::Epoch;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkflowId {
    pub lineage_id: String,
    pub epoch: Epoch,
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum WorkflowIdError {
    #[error("WorkflowId must not be empty")]
    Empty,
    #[error("WorkflowId format must be lineage_id@epoch, got: {0}")]
    InvalidFormat(String),
    #[error("epoch must not be empty in WorkflowId")]
    MissingEpoch,
    #[error("invalid epoch value: {0}")]
    InvalidEpoch(String),
}

impl WorkflowId {
    pub fn new(lineage_id: impl Into<String>, epoch: Epoch) -> Self {
        Self {
            lineage_id: lineage_id.into(),
            epoch,
        }
    }

    pub fn lineage_id(&self) -> &str {
        &self.lineage_id
    }

    pub fn epoch(&self) -> Epoch {
        self.epoch
    }
}

impl fmt::Display for WorkflowId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.lineage_id, self.epoch)
    }
}

impl FromStr for WorkflowId {
    type Err = WorkflowIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(WorkflowIdError::Empty);
        }

        let (lineage_id, epoch_str) = s
            .rsplit_once('@')
            .ok_or_else(|| WorkflowIdError::InvalidFormat(s.to_string()))?;

        let lineage_id = lineage_id.to_string();
        if lineage_id.is_empty() {
            return Err(WorkflowIdError::Empty);
        }

        if epoch_str.is_empty() {
            return Err(WorkflowIdError::MissingEpoch);
        }

        let epoch_value: u64 = epoch_str
            .parse()
            .map_err(|_| WorkflowIdError::InvalidEpoch(epoch_str.to_string()))?;

        Ok(Self {
            lineage_id,
            epoch: Epoch::new(epoch_value),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_id_new_creates_with_lineage_and_epoch() {
        let wid = WorkflowId::new("lin-abc", Epoch::new(5));
        assert_eq!(wid.lineage_id, "lin-abc");
        assert_eq!(wid.epoch, Epoch::new(5));
    }

    #[test]
    fn workflow_id_display_format_is_lineage_at_epoch() {
        let wid = WorkflowId::new("lin-abc", Epoch::new(5));
        assert_eq!(wid.to_string(), "lin-abc@5");
    }

    #[test]
    fn workflow_id_display_format_epoch_zero() {
        let wid = WorkflowId::new("lin-xyz", Epoch::ZERO);
        assert_eq!(wid.to_string(), "lin-xyz@0");
    }

    #[test]
    fn workflow_id_parse_lineage_at_epoch_succeeds() {
        let s = "lin-abc@5";
        let wid: WorkflowId = s.parse().expect("should parse");
        assert_eq!(wid.lineage_id, "lin-abc");
        assert_eq!(wid.epoch, Epoch::new(5));
    }

    #[test]
    fn workflow_id_parse_roundtrip() {
        let original = WorkflowId::new("lin-roundtrip", Epoch::new(42));
        let formatted = original.to_string();
        let parsed: WorkflowId = formatted.parse().expect("should parse");
        assert_eq!(parsed, original);
    }

    #[test]
    fn workflow_id_parse_epoch_zero() {
        let s = "lin-root@0";
        let wid: WorkflowId = s.parse().expect("should parse");
        assert_eq!(wid.lineage_id, "lin-root");
        assert_eq!(wid.epoch, Epoch::ZERO);
    }

    #[test]
    fn workflow_id_parse_empty_string_fails() {
        let result: Result<WorkflowId, _> = "".parse();
        assert_eq!(result, Err(WorkflowIdError::Empty));
    }

    #[test]
    fn workflow_id_parse_missing_at_sign_fails() {
        let result: Result<WorkflowId, _> = "lin-abc".parse();
        assert!(matches!(result, Err(WorkflowIdError::InvalidFormat(_))));
    }

    #[test]
    fn workflow_id_parse_empty_lineage_fails() {
        let result: Result<WorkflowId, _> = "@5".parse();
        assert_eq!(result, Err(WorkflowIdError::Empty));
    }

    #[test]
    fn workflow_id_parse_missing_epoch_fails() {
        let result: Result<WorkflowId, _> = "lin-abc@".parse();
        assert_eq!(result, Err(WorkflowIdError::MissingEpoch));
    }

    #[test]
    fn workflow_id_parse_invalid_epoch_fails() {
        let result: Result<WorkflowId, _> = "lin-abc@notanumber".parse();
        assert!(matches!(result, Err(WorkflowIdError::InvalidEpoch(_))));
    }

    #[test]
    fn workflow_id_epoch_accessors() {
        let wid = WorkflowId::new("lin-access", Epoch::new(99));
        assert_eq!(wid.lineage_id(), "lin-access");
        assert_eq!(wid.epoch(), Epoch::new(99));
    }

    #[test]
    fn workflow_id_lineage_id_with_special_characters() {
        let wid = WorkflowId::new("lin-abc-123_xyz", Epoch::new(1));
        let formatted = wid.to_string();
        let parsed: WorkflowId = formatted.parse().expect("should parse");
        assert_eq!(parsed, wid);
    }

    #[test]
    fn workflow_id_at_sign_in_lineage_id() {
        let wid = WorkflowId::new("lin@abc", Epoch::new(1));
        let formatted = wid.to_string();
        assert_eq!(formatted, "lin@abc@1");
        let parsed: WorkflowId = formatted.parse().expect("should parse");
        assert_eq!(parsed, wid);
    }
}
