//! GhostLifecycle tests

use vo_types::{BinaryHash, RegistrationStatus, TimestampMs, WorkflowName};

use crate::ghost_workflow::{
    GhostLifecycle, GhostWorkflowError, WorkflowReaped, WorkflowRegistration,
};

fn make_hash() -> BinaryHash {
    BinaryHash::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap()
}

fn make_name(s: &str) -> WorkflowName {
    WorkflowName::parse(s).unwrap()
}

fn make_ts(ms: u64) -> TimestampMs {
    TimestampMs::try_from(ms).unwrap()
}

fn make_registration(name: &str) -> WorkflowRegistration {
    WorkflowRegistration::new(make_name(name), make_hash(), make_ts(1000))
}

#[test]
fn deactivate_active_workflow() {
    let mut lc = GhostLifecycle::new();
    let name = make_name("test-wf");
    lc.register(make_registration("test-wf"));

    lc.deactivate(&name, make_ts(2000)).unwrap();

    let reg = lc.get(&name).unwrap();
    assert_eq!(reg.status(), RegistrationStatus::Deactivated);
    assert_eq!(reg.deactivated_at(), Some(make_ts(2000)));
    assert!(!reg.accepts_triggers());
}

#[test]
fn deactivate_quarantined_workflow() {
    let mut lc = GhostLifecycle::new();
    let name = make_name("test-wf");
    let mut reg = make_registration("test-wf");
    reg.set_status(RegistrationStatus::Quarantined);
    lc.register(reg);

    lc.deactivate(&name, make_ts(2000)).unwrap();

    let reg = lc.get(&name).unwrap();
    assert_eq!(reg.status(), RegistrationStatus::Deactivated);
}

#[test]
fn deactivate_already_deactivated_is_error() {
    let mut lc = GhostLifecycle::new();
    let name = make_name("test-wf");
    let mut reg = make_registration("test-wf");
    reg.set_status(RegistrationStatus::Deactivated);
    lc.register(reg);

    let result = lc.deactivate(&name, make_ts(3000));
    assert!(result.is_err());
}

#[test]
fn deactivate_deleted_is_error() {
    let mut lc = GhostLifecycle::new();
    let name = make_name("test-wf");
    let mut reg = make_registration("test-wf");
    reg.set_status(RegistrationStatus::Deleted);
    lc.register(reg);

    let result = lc.deactivate(&name, make_ts(3000));
    assert!(result.is_err());
}

#[test]
fn trigger_on_deactivated_returns_404() {
    let mut lc = GhostLifecycle::new();
    let name = make_name("test-wf");
    let mut reg = make_registration("test-wf");
    reg.set_status(RegistrationStatus::Deactivated);
    lc.register(reg);

    let result = lc.check_trigger(&name);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        GhostWorkflowError::TriggerRejected { .. }
    ));
}

#[test]
fn trigger_on_active_succeeds() {
    let mut lc = GhostLifecycle::new();
    let name = make_name("test-wf");
    lc.register(make_registration("test-wf"));

    assert!(lc.check_trigger(&name).is_ok());
}

#[test]
fn trigger_on_deleted_returns_404() {
    let mut lc = GhostLifecycle::new();
    let name = make_name("test-wf");
    let mut reg = make_registration("test-wf");
    reg.set_status(RegistrationStatus::Deleted);
    lc.register(reg);

    let result = lc.check_trigger(&name);
    assert!(result.is_err());
}

#[test]
fn trigger_on_unknown_workflow_returns_404() {
    let lc = GhostLifecycle::new();
    let name = make_name("nonexistent");

    let result = lc.check_trigger(&name);
    assert!(result.is_err());
}

#[test]
fn reap_deactivated_with_zero_instances() {
    let mut lc = GhostLifecycle::new();
    let name = make_name("test-wf");
    let mut reg = make_registration("test-wf");
    reg.set_status(RegistrationStatus::Deactivated);
    lc.register(reg);

    let reaped = lc.reap();

    assert_eq!(reaped.len(), 1);
    assert_eq!(reaped[0].workflow, name);
    assert_eq!(reaped[0].version_hash, make_hash());
    assert_eq!(lc.get(&name).unwrap().status(), RegistrationStatus::Deleted);
}

#[test]
fn transition_to_deleted_succeeds_from_deactivated() {
    let mut reg = make_registration("test-wf");
    reg.set_status(RegistrationStatus::Deactivated);

    let event = reg.transition_to_deleted().unwrap();
    assert_eq!(event.workflow, make_name("test-wf"));
    assert_eq!(event.version_hash, make_hash());
    assert_eq!(reg.status(), RegistrationStatus::Deleted);
}

#[test]
fn transition_to_deleted_rejects_active() {
    let mut reg = make_registration("test-wf");
    let result = reg.transition_to_deleted();
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        GhostWorkflowError::InvalidTransition { .. }
    ));
    assert_eq!(reg.status(), RegistrationStatus::Active);
}

#[test]
fn transition_to_deleted_rejects_deactivated_with_instances() {
    let mut reg = make_registration("test-wf");
    reg.set_status(RegistrationStatus::Deactivated);
    reg.running_instance_count = 2;

    let result = reg.transition_to_deleted();
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        GhostWorkflowError::ReaperNotDeactivated { .. }
    ));
    assert_eq!(reg.status(), RegistrationStatus::Deactivated);
}

#[test]
fn transition_to_deleted_rejects_already_deleted() {
    let mut reg = make_registration("test-wf");
    reg.set_status(RegistrationStatus::Deleted);

    let result = reg.transition_to_deleted();
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        GhostWorkflowError::InvalidTransition { .. }
    ));
}

#[test]
fn reap_skips_deactivated_with_running_instances() {
    let mut lc = GhostLifecycle::new();
    let name = make_name("test-wf");
    let mut reg = make_registration("test-wf");
    reg.set_status(RegistrationStatus::Deactivated);
    reg.running_instance_count = 3;
    lc.register(reg);

    let reaped = lc.reap();

    assert!(reaped.is_empty());
    assert_eq!(
        lc.get(&name).unwrap().status(),
        RegistrationStatus::Deactivated
    );
}

#[test]
fn reap_skips_active_workflows() {
    let mut lc = GhostLifecycle::new();
    lc.register(make_registration("test-wf"));

    let reaped = lc.reap();

    assert!(reaped.is_empty());
    assert_eq!(
        lc.get(&make_name("test-wf")).unwrap().status(),
        RegistrationStatus::Active
    );
}

#[test]
fn in_flight_completes_then_reaper_cleans_up() {
    let mut lc = GhostLifecycle::new();
    let name = make_name("test-wf");

    let mut reg = make_registration("test-wf");
    reg.increment_instances();
    reg.increment_instances();
    lc.register(reg);

    lc.deactivate(&name, make_ts(2000)).unwrap();
    assert_eq!(lc.get(&name).unwrap().running_instance_count(), 2);

    let reaped = lc.reap();
    assert!(reaped.is_empty());

    lc.instance_completed(&name);
    lc.instance_completed(&name);
    assert_eq!(lc.get(&name).unwrap().running_instance_count(), 0);

    let reaped = lc.reap();
    assert_eq!(reaped.len(), 1);
    assert_eq!(lc.get(&name).unwrap().status(), RegistrationStatus::Deleted);
}

#[test]
fn full_lifecycle_active_deactivate_reap() {
    let mut lc = GhostLifecycle::new();
    let name = make_name("my-workflow");
    lc.register(make_registration("my-workflow"));

    assert!(lc.check_trigger(&name).is_ok());

    lc.instance_started(&name);
    lc.deactivate(&name, make_ts(2000)).unwrap();
    assert!(lc.check_trigger(&name).is_err());

    let reaped = lc.reap();
    assert!(reaped.is_empty());

    lc.instance_completed(&name);
    let reaped = lc.reap();
    assert_eq!(reaped.len(), 1);
    assert_eq!(lc.get(&name).unwrap().status(), RegistrationStatus::Deleted);
}

#[test]
fn reap_multiple_workflows() {
    let mut lc = GhostLifecycle::new();

    let mut reg1 = make_registration("wf-a");
    reg1.set_status(RegistrationStatus::Deactivated);
    lc.register(reg1);

    let mut reg2 = make_registration("wf-b");
    reg2.set_status(RegistrationStatus::Deactivated);
    reg2.running_instance_count = 1;
    lc.register(reg2);

    lc.register(make_registration("wf-c"));

    let reaped = lc.reap();
    assert_eq!(reaped.len(), 1);
    assert_eq!(
        lc.get(&make_name("wf-a")).unwrap().status(),
        RegistrationStatus::Deleted
    );
    assert_eq!(
        lc.get(&make_name("wf-b")).unwrap().status(),
        RegistrationStatus::Deactivated
    );
    assert_eq!(
        lc.get(&make_name("wf-c")).unwrap().status(),
        RegistrationStatus::Active
    );
}
