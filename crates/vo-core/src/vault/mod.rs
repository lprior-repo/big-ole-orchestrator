pub mod access;
pub mod error;
pub mod rotation;
pub mod tests;
pub mod types;
#[allow(clippy::module_inception)]
pub mod vault;

pub use error::{CredentialError, RotationFailureReason};
pub use types::CredentialSummary;
pub use vault::CredentialVault;
pub use vo_types::credentials::Permission;
