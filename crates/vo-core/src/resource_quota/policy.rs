//! Overcommit policy for resource quotas.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum OvercommitPolicy {
    #[default]
    NoOvercommit,
    AllowOvercommit,
}

impl OvercommitPolicy {
    pub fn allows_overcommit(&self) -> bool {
        matches!(self, Self::AllowOvercommit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overcommit_policy_default_is_no_overcommit() {
        assert_eq!(OvercommitPolicy::default(), OvercommitPolicy::NoOvercommit);
    }

    #[test]
    fn overcommit_policy_allows_overcommit_returns_true_when_allow() {
        assert!(OvercommitPolicy::AllowOvercommit.allows_overcommit());
    }

    #[test]
    fn overcommit_policy_allows_overcommit_returns_false_when_no_overcommit() {
        assert!(!OvercommitPolicy::NoOvercommit.allows_overcommit());
    }
}
