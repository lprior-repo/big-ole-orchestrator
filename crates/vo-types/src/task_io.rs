use serde_json::Value;

/// Classifies why a task subprocess failed.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum TaskFailureKind {
    User,
    System,
    Timeout,
}

impl TaskFailureKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "User",
            Self::System => "System",
            Self::Timeout => "Timeout",
        }
    }
}

/// Deserialization envelope for FD3 task input.
#[derive(serde::Deserialize)]
pub struct TaskInputEnvelope {
    pub idempotency_key: String,
    pub data: Value,
}
