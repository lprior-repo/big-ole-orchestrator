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
    fn b010_overcommit_policy_has_exactly_two_variants() {
        fn _exhaustiveness(p: OvercommitPolicy) -> bool {
            match p {
                OvercommitPolicy::NoOvercommit => false,
                OvercommitPolicy::AllowOvercommit => true,
            }
        }
        assert!(!_exhaustiveness(OvercommitPolicy::NoOvercommit));
        assert!(_exhaustiveness(OvercommitPolicy::AllowOvercommit));
    }

    #[test]
    fn b011_overcommit_policy_default_is_no_overcommit() {
        assert_eq!(OvercommitPolicy::default(), OvercommitPolicy::NoOvercommit);
    }

    #[test]
    fn b012_overcommit_policy_allows_overcommit_returns_true_for_allow() {
        assert!(OvercommitPolicy::AllowOvercommit.allows_overcommit());
    }

    #[test]
    fn b012_overcommit_policy_allows_overcommit_returns_false_for_no_overcommit() {
        assert!(!OvercommitPolicy::NoOvercommit.allows_overcommit());
    }

    #[test]
    fn b013_overcommit_policy_implements_clone_copy_partial_eq_eq_hash() {
        let p = OvercommitPolicy::AllowOvercommit;
        let p2 = p;
        assert_eq!(p, p2);
        let p3 = p.clone();
        assert_eq!(p, p3);
        let mut h = std::collections::HashSet::new();
        h.insert(p);
        assert!(h.contains(&OvercommitPolicy::AllowOvercommit));
    }

    #[test]
    fn b013_overcommit_policy_serializes_and_deserializes() {
        let json = serde_json::to_string(&OvercommitPolicy::AllowOvercommit).unwrap();
        let p: OvercommitPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(p, OvercommitPolicy::AllowOvercommit);

        let json2 = serde_json::to_string(&OvercommitPolicy::NoOvercommit).unwrap();
        let p2: OvercommitPolicy = serde_json::from_str(&json2).unwrap();
        assert_eq!(p2, OvercommitPolicy::NoOvercommit);
    }
}
