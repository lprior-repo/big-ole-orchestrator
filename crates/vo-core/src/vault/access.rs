use vo_types::credentials::{AccessPolicy, Principal};

pub struct AccessChecker<'a> {
    policy: &'a AccessPolicy,
    caller: &'a Principal,
}

impl<'a> AccessChecker<'a> {
    pub fn new(policy: &'a AccessPolicy, caller: &'a Principal) -> Self {
        Self { policy, caller }
    }

    pub fn can_read(&self) -> bool {
        self.caller_is_authorized()
    }

    pub fn can_write(&self) -> bool {
        self.caller_is_authorized()
    }

    pub fn can_delete(&self) -> bool {
        self.caller_is_authorized()
    }

    pub fn can_rotate(&self) -> bool {
        self.caller_is_authorized()
    }

    pub fn can_revoke(&self) -> bool {
        self.caller_is_authorized()
    }

    fn caller_is_authorized(&self) -> bool {
        match self.caller {
            Principal::System => true,
            _ => {
                if !self.policy.allowed_principals().contains(self.caller) {
                    return false;
                }
                if self.policy.require_approval()
                    && !self.policy.approvers().contains(self.caller)
                {
                    return false;
                }
                true
            }
        }
    }
}

pub fn is_authorized(policy: &AccessPolicy, principal: &Principal) -> bool {
    AccessChecker::new(policy, principal).can_read()
}

#[cfg(test)]
mod tests {
    use super::*;
    use vo_types::{InstanceId, SpawnId, WorkflowName};

    fn make_user(id: &str) -> Principal {
        Principal::User(InstanceId::parse(id).expect("valid ULID"))
    }

    #[test]
    fn system_principal_always_allowed() {
        let policy = AccessPolicy::new(vec![]);
        let system = Principal::System;
        let checker = AccessChecker::new(&policy, &system);
        assert!(checker.can_read());
        assert!(checker.can_write());
        assert!(checker.can_delete());
        assert!(checker.can_rotate());
        assert!(checker.can_revoke());
    }

    #[test]
    fn empty_policy_denies_non_system() {
        let policy = AccessPolicy::new(vec![]);
        let user = make_user("01H5JYV4XHGSR2F8KZ9BWNRFMA");
        let checker = AccessChecker::new(&policy, &user);
        assert!(!checker.can_read());
        assert!(!checker.can_write());
    }

    #[test]
    fn user_in_allowed_principals_can_read() {
        let user = make_user("01H5JYV4XHGSR2F8KZ9BWNRFMA");
        let other = make_user("01H5JYV4XHGSR2F8KZ9BWNRFMB");
        let policy = AccessPolicy::new(vec![user.clone()]);
        let checker = AccessChecker::new(&policy, &user);
        assert!(checker.can_read());

        let checker_other = AccessChecker::new(&policy, &other);
        assert!(!checker_other.can_read());
    }

    #[test]
    fn user_in_approvers_can_read_when_approval_required() {
        let user = make_user("01H5JYV4XHGSR2F8KZ9BWNRFMA");
        let mut policy = AccessPolicy::new(vec![user.clone()]);
        policy = AccessPolicy {
            allowed_principals: policy.allowed_principals,
            require_approval: true,
            approvers: vec![user.clone()],
            audit_enabled: true,
        };
        let checker = AccessChecker::new(&policy, &user);
        assert!(checker.can_read());
    }

    #[test]
    fn user_not_in_approvers_denied_when_approval_required() {
        let user = make_user("01H5JYV4XHGSR2F8KZ9BWNRFMA");
        let other = make_user("01H5JYV4XHGSR2F8KZ9BWNRFMB");
        let mut policy = AccessPolicy::new(vec![user.clone()]);
        policy = AccessPolicy {
            allowed_principals: policy.allowed_principals,
            require_approval: true,
            approvers: vec![other.clone()],
            audit_enabled: true,
        };
        let checker = AccessChecker::new(&policy, &user);
        assert!(!checker.can_read());
    }

    #[test]
    fn is_authorized_helper_system() {
        let policy = AccessPolicy::new(vec![]);
        assert!(is_authorized(&policy, &Principal::System));
    }

    #[test]
    fn is_authorized_helper_user() {
        let user = make_user("01H5JYV4XHGSR2F8KZ9BWNRFMA");
        let policy = AccessPolicy::new(vec![user.clone()]);
        assert!(is_authorized(&policy, &user));
    }

    #[test]
    fn is_authorized_helper_denies_unknown() {
        let policy = AccessPolicy::new(vec![]);
        let user = make_user("01H5JYV4XHGSR2F8KZ9BWNRFMA");
        assert!(!is_authorized(&policy, &user));
    }

    #[test]
    fn actor_principal_authorized() {
        let actor =
            Principal::Actor(SpawnId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid SpawnId"));
        let policy = AccessPolicy::new(vec![actor.clone()]);
        let checker = AccessChecker::new(&policy, &actor);
        assert!(checker.can_read());
    }

    #[test]
    fn workflow_principal_authorized() {
        let workflow = Principal::Workflow(
            WorkflowName::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid WorkflowName"),
        );
        let policy = AccessPolicy::new(vec![workflow.clone()]);
        let checker = AccessChecker::new(&policy, &workflow);
        assert!(checker.can_read());
    }
}
