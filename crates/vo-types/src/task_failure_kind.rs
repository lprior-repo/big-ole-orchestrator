use serde::{Deserialize, Serialize};

/// Represents the different ways a task can fail.
///
/// This is a core domain type that belongs in vo-types alongside other
/// result/error types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskFailureKind {
    User,
    System,
    Timeout,
}

impl TaskFailureKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "User",
            Self::System => "System",
            Self::Timeout => "Timeout",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_failure_kind_as_str() {
        assert_eq!(TaskFailureKind::User.as_str(), "User");
        assert_eq!(TaskFailureKind::System.as_str(), "System");
        assert_eq!(TaskFailureKind::Timeout.as_str(), "Timeout");
    }

    #[test]
    fn task_failure_kind_debug() {
        assert_eq!(format!("{:?}", TaskFailureKind::User), "User");
        assert_eq!(format!("{:?}", TaskFailureKind::System), "System");
        assert_eq!(format!("{:?}", TaskFailureKind::Timeout), "Timeout");
    }

    #[test]
    fn task_failure_kind_equality() {
        assert_eq!(TaskFailureKind::User, TaskFailureKind::User);
        assert_ne!(TaskFailureKind::User, TaskFailureKind::System);
        assert_ne!(TaskFailureKind::System, TaskFailureKind::Timeout);
    }

    #[test]
    fn task_failure_kind_clone_copy() {
        let kind = TaskFailureKind::User;
        let cloned = kind;
        assert_eq!(kind, cloned);

        let kind = TaskFailureKind::System;
        let copied = kind;
        assert_eq!(kind, copied);
    }

    #[test]
    fn task_failure_kind_serialize_roundtrip() {
        for kind in [
            TaskFailureKind::User,
            TaskFailureKind::System,
            TaskFailureKind::Timeout,
        ] {
            let json = serde_json::to_string(&kind).unwrap();
            let deserialized: TaskFailureKind = serde_json::from_str(&json).unwrap();
            assert_eq!(kind, deserialized);
        }
    }
}
