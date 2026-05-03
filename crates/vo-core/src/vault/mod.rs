pub mod error;
pub mod rotation;
pub mod types;
pub mod vault;

pub use error::{CredentialError, RotationFailureReason};
pub use rotation::{RotationStateMachine, RotationStateError};
pub use types::CredentialSummary;
pub use vault::CredentialVault;

/// Permission level for vault access control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
    Read,
    Write,
    Admin,
}

impl std::fmt::Display for Permission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Permission::Read => write!(f, "Read"),
            Permission::Write => write!(f, "Write"),
            Permission::Admin => write!(f, "Admin"),
        }
    }
}