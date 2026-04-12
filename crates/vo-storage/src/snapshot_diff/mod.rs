use serde::{Deserialize, Serialize};
use vo_types::state::InstanceState;
use vo_types::InstanceId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DiffOperation<T> {
    Unchanged,
    Added(T),
    Removed(T),
    Modified(T, T),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateDiff {
    pub counter: DiffOperation<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotDiff {
    pub from_sequence: u64,
    pub to_sequence: u64,
    pub instance_id: InstanceId,
    pub state_diff: StateDiff,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DiffResult {
    Identical,
    HasDiff(SnapshotDiff),
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum DiffError {
    #[error("Snapshot bytes fail deserialization")]
    CorruptSnapshot,
    #[error("Schema version incompatibility")]
    VersionMismatch,
    #[error("Snapshots not consecutive")]
    SequenceGap,
    #[error("Cannot serialize diff")]
    SerializationFailed,
    #[error("Cannot deserialize diff")]
    DeserializationFailed,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ApplyError {
    #[error("Base state doesn't match expected")]
    BaseStateMismatch,
    #[error("Diff cannot apply to base")]
    DiffTargetInvalid,
    #[error("Target sequence < base sequence")]
    SequenceRegress,
}

pub fn diff(
    instance_id: InstanceId,
    from: &(u64, InstanceState),
    to: &(u64, InstanceState),
) -> DiffResult {
    let (from_seq, from_state) = from;
    let (to_seq, to_state) = to;

    if from_seq == to_seq {
        return DiffResult::Identical;
    }

    if to_seq < from_seq {
        return DiffResult::Identical;
    }

    let counter_diff = if from_state.counter == to_state.counter {
        DiffOperation::Unchanged
    } else if from_state.counter == 0 && to_state.counter > 0 {
        DiffOperation::Added(to_state.counter)
    } else if from_state.counter > 0 && to_state.counter == 0 {
        DiffOperation::Removed(from_state.counter)
    } else {
        DiffOperation::Modified(from_state.counter, to_state.counter)
    };

    let state_diff = StateDiff {
        counter: counter_diff,
    };

    DiffResult::HasDiff(SnapshotDiff {
        from_sequence: *from_seq,
        to_sequence: *to_seq,
        instance_id,
        state_diff,
    })
}

pub fn apply_diff(
    base: &(u64, InstanceState),
    diff: &SnapshotDiff,
) -> Result<InstanceState, ApplyError> {
    let (base_seq, base_state) = base;

    if diff.from_sequence != *base_seq {
        return Err(ApplyError::BaseStateMismatch);
    }

    let new_counter = match diff.state_diff.counter {
        DiffOperation::Unchanged => base_state.counter,
        DiffOperation::Added(val) => {
            if base_state.counter == 0 {
                val
            } else {
                return Err(ApplyError::DiffTargetInvalid);
            }
        }
        DiffOperation::Removed(val) => {
            if base_state.counter == val {
                0
            } else {
                return Err(ApplyError::DiffTargetInvalid);
            }
        }
        DiffOperation::Modified(old_val, new_val) => {
            if base_state.counter == old_val {
                new_val
            } else {
                return Err(ApplyError::DiffTargetInvalid);
            }
        }
    };

    Ok(InstanceState {
        counter: new_counter,
    })
}

pub fn invert_diff(diff: &SnapshotDiff) -> SnapshotDiff {
    let inverted_counter = match diff.state_diff.counter {
        DiffOperation::Unchanged => DiffOperation::Unchanged,
        DiffOperation::Added(val) => DiffOperation::Removed(val),
        DiffOperation::Removed(val) => DiffOperation::Added(val),
        DiffOperation::Modified(old_val, new_val) => DiffOperation::Modified(new_val, old_val),
    };

    SnapshotDiff {
        from_sequence: diff.to_sequence,
        to_sequence: diff.from_sequence,
        instance_id: diff.instance_id.clone(),
        state_diff: StateDiff {
            counter: inverted_counter,
        },
    }
}

impl SnapshotDiff {
    pub fn compose(&self, other: &SnapshotDiff) -> Result<SnapshotDiff, DiffError> {
        if self.to_sequence != other.from_sequence {
            return Err(DiffError::SequenceGap);
        }

        if self.instance_id != other.instance_id {
            return Err(DiffError::SequenceGap);
        }

        let composed_counter = match (&self.state_diff.counter, &other.state_diff.counter) {
            (DiffOperation::Unchanged, op) => *op,
            (op, DiffOperation::Unchanged) => *op,
            (DiffOperation::Added(_), DiffOperation::Added(_)) => {
                return Err(DiffError::SequenceGap);
            }
            (DiffOperation::Removed(_), DiffOperation::Removed(_)) => {
                return Err(DiffError::SequenceGap);
            }
            (DiffOperation::Modified(init_a, new_a), DiffOperation::Modified(old_b, final_b))
                if *new_a == *old_b =>
            {
                DiffOperation::Modified(*init_a, *final_b)
            }
            _ => return Err(DiffError::SequenceGap),
        };

        Ok(Self {
            from_sequence: self.from_sequence,
            to_sequence: other.to_sequence,
            instance_id: self.instance_id.clone(),
            state_diff: StateDiff {
                counter: composed_counter,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_idempotence() {
        let instance_id = vo_types::InstanceId::from_bytes([0u8; 16]);
        let state = InstanceState { counter: 42 };
        let diff_result = diff(instance_id, &(0, state.clone()), &(0, state.clone()));
        assert!(matches!(diff_result, DiffResult::Identical));
    }

    #[test]
    fn test_diff_added() {
        let instance_id = vo_types::InstanceId::from_bytes([0u8; 16]);
        let from_state = InstanceState { counter: 0 };
        let to_state = InstanceState { counter: 100 };
        let result = diff(instance_id, &(0, from_state), &(5, to_state));
        match result {
            DiffResult::HasDiff(diff) => {
                assert!(matches!(diff.state_diff.counter, DiffOperation::Added(100)));
            }
            _ => panic!("Expected HasDiff"),
        }
    }

    #[test]
    fn test_diff_modified() {
        let instance_id = vo_types::InstanceId::from_bytes([0u8; 16]);
        let from_state = InstanceState { counter: 10 };
        let to_state = InstanceState { counter: 20 };
        let result = diff(instance_id, &(0, from_state), &(5, to_state));
        match result {
            DiffResult::HasDiff(diff) => {
                assert!(matches!(
                    diff.state_diff.counter,
                    DiffOperation::Modified(10, 20)
                ));
            }
            _ => panic!("Expected HasDiff"),
        }
    }

    #[test]
    fn test_invert_added() {
        let instance_id = vo_types::InstanceId::from_bytes([0u8; 16]);
        let diff = SnapshotDiff {
            from_sequence: 0,
            to_sequence: 5,
            instance_id,
            state_diff: StateDiff {
                counter: DiffOperation::Added(100),
            },
        };
        let inverted = invert_diff(&diff);
        assert!(matches!(
            inverted.state_diff.counter,
            DiffOperation::Removed(100)
        ));
        assert_eq!(inverted.from_sequence, 5);
        assert_eq!(inverted.to_sequence, 0);
    }

    #[test]
    fn test_invert_modified() {
        let instance_id = vo_types::InstanceId::from_bytes([0u8; 16]);
        let diff = SnapshotDiff {
            from_sequence: 0,
            to_sequence: 5,
            instance_id,
            state_diff: StateDiff {
                counter: DiffOperation::Modified(10, 20),
            },
        };
        let inverted = invert_diff(&diff);
        assert!(matches!(
            inverted.state_diff.counter,
            DiffOperation::Modified(20, 10)
        ));
    }

    #[test]
    fn test_apply_added() {
        let instance_id = vo_types::InstanceId::from_bytes([0u8; 16]);
        let base = InstanceState { counter: 0 };
        let diff = SnapshotDiff {
            from_sequence: 0,
            to_sequence: 5,
            instance_id,
            state_diff: StateDiff {
                counter: DiffOperation::Added(100),
            },
        };
        let result = apply_diff(&(0, base), &diff);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().counter, 100);
    }

    #[test]
    fn test_apply_modified() {
        let instance_id = vo_types::InstanceId::from_bytes([0u8; 16]);
        let base = InstanceState { counter: 10 };
        let diff = SnapshotDiff {
            from_sequence: 0,
            to_sequence: 5,
            instance_id,
            state_diff: StateDiff {
                counter: DiffOperation::Modified(10, 20),
            },
        };
        let result = apply_diff(&(0, base), &diff);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().counter, 20);
    }

    #[test]
    fn test_compose_forward() {
        let instance_id = vo_types::InstanceId::from_bytes([0u8; 16]);
        let diff_ab = SnapshotDiff {
            from_sequence: 0,
            to_sequence: 5,
            instance_id: instance_id.clone(),
            state_diff: StateDiff {
                counter: DiffOperation::Modified(10, 20),
            },
        };
        let diff_bc = SnapshotDiff {
            from_sequence: 5,
            to_sequence: 10,
            instance_id,
            state_diff: StateDiff {
                counter: DiffOperation::Modified(20, 30),
            },
        };
        let composed = diff_ab.compose(&diff_bc);
        assert!(composed.is_ok());
        let composed = composed.unwrap();
        assert_eq!(composed.from_sequence, 0);
        assert_eq!(composed.to_sequence, 10);
        assert!(matches!(
            composed.state_diff.counter,
            DiffOperation::Modified(10, 30)
        ));
    }

    #[test]
    fn test_invert_produces_correct_inverse() {
        let instance_id = vo_types::InstanceId::from_bytes([0u8; 16]);
        let from_state = InstanceState { counter: 10 };
        let to_state = InstanceState { counter: 20 };
        let diff_result = diff(instance_id, &(0, from_state), &(5, to_state));
        match diff_result {
            DiffResult::HasDiff(diff) => {
                let inverted = invert_diff(&diff);
                assert_eq!(inverted.from_sequence, 5);
                assert_eq!(inverted.to_sequence, 0);
                assert!(matches!(
                    inverted.state_diff.counter,
                    DiffOperation::Modified(20, 10)
                ));
                let applied = apply_diff(&(5, InstanceState { counter: 20 }), &inverted);
                assert!(applied.is_ok());
                assert_eq!(applied.unwrap().counter, 10);
            }
            _ => panic!("Expected HasDiff"),
        }
    }
}
