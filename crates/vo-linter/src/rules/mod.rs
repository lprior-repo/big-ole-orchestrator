//! Collection of linting rules for workflow validation.
//!
//! Each rule module is independently testable and focuses on a specific
//! category of workflow issues:
//!
//! - [`random`] — L002: Detects non-deterministic random calls

mod random;

pub use random::check_random_in_workflow;
